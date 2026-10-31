//! The software center's words, in the person's language.
//!
//! # Why a map per enum, and why every one of them is a `match`
//!
//! Because every enum here crosses into the screen as TEXT, and the crate that
//! owns them (`ronoctua-software`) names a word per variant for its wire and
//! its CLI, never a sentence. A key built at run time from that word —
//! `format!("software-refusal-{}", r.word())` — compiles whatever the `.ftl`
//! holds and shows `[software-refusal-new-thing]` on the day a variant is added.
//! A `match` onto `crate::key::*` fails to COMPILE on that day instead,
//! in exactly one file, which is the property the audio and network views'
//! `_words` files already have for their own daemons.
//!
//! # And the words about the words
//!
//! A refusal is a SENTENCE here, never a word: the one thing a person reading
//! a refused row needs is what to do next, and `unfree-needs-consent` is a
//! word for a machine. The program's own trouble, when there is one, travels
//! beside the sentence as data (`Job::trouble`), and is never looked up.

use ronoctua_software::declaration::{DeclineState, Standing};
use ronoctua_software::found::{Badge, Prefer, Status};
use ronoctua_software::order::{AndroidImage, Display, Grant, Reach, Traces};
use ronoctua_software::person::reports::SkipWhy;
use ronoctua_software::progress::Phase;
use ronoctua_software::state::Origin;
use ronoctua_software::{Refusal, Source};
use ronoctua_tokens as tokens;

/// A source's name.
#[must_use]
pub const fn source_key(source: Source) -> &'static str {
    use crate::key;
    match source {
        Source::Nix => key::SOFTWARE_SOURCE_NIX,
        Source::Flatpak => key::SOFTWARE_SOURCE_FLATPAK,
        Source::AppImage => key::SOFTWARE_SOURCE_APPIMAGE,
        Source::Android => key::SOFTWARE_SOURCE_ANDROID,
        Source::Web => key::SOFTWARE_SOURCE_WEB,
    }
}

/// A source's glyph, in front of a program's name.
#[must_use]
pub const fn source_glyph(source: Source) -> char {
    match source {
        Source::Nix => tokens::icon::SOFTWARE_SOURCE_NIX,
        Source::Flatpak => tokens::icon::SOFTWARE_SOURCE_FLATPAK,
        Source::AppImage => tokens::icon::SOFTWARE_SOURCE_APPIMAGE,
        Source::Android => tokens::icon::SOFTWARE_SOURCE_ANDROID,
        Source::Web => tokens::icon::SOFTWARE_SOURCE_WEB,
    }
}

/// The glyph of a key: its source's, or the system's for what the flake
/// installed.
#[must_use]
pub const fn key_glyph(key: &ronoctua_software::AppKey) -> char {
    match key.source() {
        Some(s) => source_glyph(s),
        None => tokens::icon::SOFTWARE_SOURCE_SYSTEM,
    }
}

/// Why this source was recommended, as a sentence.
#[must_use]
pub const fn why_key(source: Source) -> &'static str {
    use crate::key;
    match source {
        Source::Nix => key::SOFTWARE_NOTE_WHY_NIX,
        Source::Flatpak => key::SOFTWARE_NOTE_WHY_FLATPAK,
        Source::AppImage => key::SOFTWARE_NOTE_WHY_APPIMAGE,
        Source::Android => key::SOFTWARE_NOTE_WHY_ANDROID,
        Source::Web => key::SOFTWARE_NOTE_WHY_WEB,
    }
}

/// What one badge says.
#[must_use]
pub const fn badge_key(badge: Badge) -> &'static str {
    use crate::key;
    match badge {
        Badge::Verified => key::SOFTWARE_BADGE_VERIFIED,
        Badge::Unfree => key::SOFTWARE_BADGE_UNFREE,
        Badge::Sandboxed => key::SOFTWARE_BADGE_SANDBOXED,
        Badge::CommandLine => key::SOFTWARE_BADGE_COMMAND_LINE,
        Badge::ArmOnly => key::SOFTWARE_BADGE_ARM_ONLY,
        Badge::Broken => key::SOFTWARE_BADGE_BROKEN,
        Badge::Insecure => key::SOFTWARE_BADGE_INSECURE,
        Badge::EndOfLife => key::SOFTWARE_BADGE_END_OF_LIFE,
        Badge::AntiFeatures => key::SOFTWARE_BADGE_ANTI_FEATURES,
        Badge::Library => key::SOFTWARE_BADGE_LIBRARY,
        Badge::Installed => key::SOFTWARE_BADGE_INSTALLED,
    }
}

/// One badge's glyph.
#[must_use]
pub const fn badge_glyph(badge: Badge) -> char {
    match badge {
        Badge::Verified => tokens::icon::SOFTWARE_BADGE_VERIFIED,
        Badge::Unfree => tokens::icon::SOFTWARE_BADGE_UNFREE,
        Badge::Sandboxed => tokens::icon::SOFTWARE_BADGE_SANDBOXED,
        Badge::CommandLine => tokens::icon::SOFTWARE_BADGE_COMMAND_LINE,
        Badge::ArmOnly => tokens::icon::SOFTWARE_BADGE_ARM_ONLY,
        Badge::Broken => tokens::icon::SOFTWARE_BADGE_BROKEN,
        Badge::Insecure => tokens::icon::SOFTWARE_BADGE_INSECURE,
        Badge::EndOfLife => tokens::icon::SOFTWARE_BADGE_END_OF_LIFE,
        Badge::AntiFeatures => tokens::icon::SOFTWARE_BADGE_ANTI_FEATURES,
        Badge::Library => tokens::icon::SOFTWARE_BADGE_LIBRARY,
        Badge::Installed => tokens::icon::SOFTWARE_BADGE_INSTALLED,
    }
}

/// Whether a badge is a WARNING: drawn in the danger tone where it is a line
/// of its own. Danger is a colour and never another drawing — the glyph stays the badge's.
#[must_use]
pub const fn badge_warns(badge: Badge) -> bool {
    match badge {
        Badge::Broken | Badge::Insecure | Badge::EndOfLife | Badge::ArmOnly => true,
        Badge::Verified
        | Badge::Unfree
        | Badge::Sandboxed
        | Badge::CommandLine
        | Badge::AntiFeatures
        | Badge::Library
        | Badge::Installed => false,
    }
}

/// The sentence one refusal gets.
#[must_use]
pub const fn refusal_key(why: Refusal) -> &'static str {
    use crate::key;
    match why {
        Refusal::UnfreeNeedsConsent => key::SOFTWARE_REFUSAL_UNFREE_NEEDS_CONSENT,
        Refusal::UnverifiedNeedsConsent => key::SOFTWARE_REFUSAL_UNVERIFIED_NEEDS_CONSENT,
        Refusal::BigNeedsConsent => key::SOFTWARE_REFUSAL_BIG_NEEDS_CONSENT,
        Refusal::Insecure => key::SOFTWARE_REFUSAL_INSECURE,
        Refusal::Broken => key::SOFTWARE_REFUSAL_BROKEN,
        Refusal::Unavailable => key::SOFTWARE_REFUSAL_UNAVAILABLE,
        Refusal::SourceAbsent => key::SOFTWARE_REFUSAL_SOURCE_ABSENT,
        Refusal::SourceUnprepared => key::SOFTWARE_REFUSAL_SOURCE_UNPREPARED,
        Refusal::DeclaredInTheFlake => key::SOFTWARE_REFUSAL_DECLARED_IN_THE_FLAKE,
        Refusal::ArmOnly => key::SOFTWARE_REFUSAL_ARM_ONLY,
        Refusal::Offline => key::SOFTWARE_REFUSAL_OFFLINE,
        Refusal::RateLimited => key::SOFTWARE_REFUSAL_RATE_LIMITED,
        Refusal::Integrity => key::SOFTWARE_REFUSAL_INTEGRITY,
        Refusal::TooLarge => key::SOFTWARE_REFUSAL_TOO_LARGE,
        Refusal::NotAnAppImage => key::SOFTWARE_REFUSAL_NOT_AN_APPIMAGE,
        Refusal::LegacyAppImage => key::SOFTWARE_REFUSAL_LEGACY_APPIMAGE,
        Refusal::DwarfsAppImage => key::SOFTWARE_REFUSAL_DWARFS_APPIMAGE,
        Refusal::NotAnApk => key::SOFTWARE_REFUSAL_NOT_AN_APK,
        Refusal::UrlScheme => key::SOFTWARE_REFUSAL_URL_SCHEME,
        Refusal::UrlCredentials => key::SOFTWARE_REFUSAL_URL_CREDENTIALS,
        Refusal::UrlInvalid => key::SOFTWARE_REFUSAL_URL_INVALID,
        Refusal::DuplicateWindow => key::SOFTWARE_REFUSAL_DUPLICATE_WINDOW,
        Refusal::NoBrowser => key::SOFTWARE_REFUSAL_NO_BROWSER,
        Refusal::ProfileUnknown => key::SOFTWARE_REFUSAL_PROFILE_UNKNOWN,
        Refusal::IconUnusable => key::SOFTWARE_REFUSAL_ICON_UNUSABLE,
        Refusal::ProfileIsNixEnv => key::SOFTWARE_REFUSAL_PROFILE_IS_NIX_ENV,
        Refusal::Conflict => key::SOFTWARE_REFUSAL_CONFLICT,
        Refusal::NotCancellableNow => key::SOFTWARE_REFUSAL_NOT_CANCELLABLE_NOW,
        Refusal::UnknownItem => key::SOFTWARE_REFUSAL_UNKNOWN_ITEM,
        Refusal::ToolFailed => key::SOFTWARE_REFUSAL_TOOL_FAILED,
        Refusal::NoUpdateInfo => key::SOFTWARE_REFUSAL_NO_UPDATE_INFO,
        Refusal::CannotTry => key::SOFTWARE_REFUSAL_CANNOT_TRY,
        Refusal::SessionDown => key::SOFTWARE_REFUSAL_SESSION_DOWN,
        Refusal::NotInsideAndroid => key::SOFTWARE_REFUSAL_NOT_INSIDE_ANDROID,
        Refusal::WillNotParse => key::SOFTWARE_REFUSAL_WILL_NOT_PARSE,
        Refusal::WrongUser => key::SOFTWARE_REFUSAL_WRONG_USER,
        Refusal::NotLet => key::SOFTWARE_REFUSAL_NOT_LET,
        Refusal::NothingToPin => key::SOFTWARE_DECLARED_REFUSAL_NOTHING_TO_PIN,
        Refusal::DeclarationRefused => key::SOFTWARE_DECLARED_REFUSAL_DECLARATION_REFUSED,
        Refusal::DeclarationUnreadable => key::SOFTWARE_DECLARED_REFUSAL_DECLARATION_UNREADABLE,
        Refusal::Essential => key::SOFTWARE_DECLARED_REFUSAL_ESSENTIAL,
        Refusal::ApplyFailed => key::SOFTWARE_DECLARED_REFUSAL_APPLY_FAILED,
        Refusal::MachineUnnamed => key::SOFTWARE_DECLARED_REFUSAL_MACHINE_UNNAMED,
        Refusal::LeavesAtNextApply => key::SOFTWARE_DECLARED_REFUSAL_LEAVES_AT_NEXT_APPLY,
        Refusal::LeavesAtNextRestart => key::SOFTWARE_DECLARED_REFUSAL_LEAVES_AT_NEXT_RESTART,
        Refusal::DeclinedElsewhere => key::SOFTWARE_DECLARED_REFUSAL_DECLINED_ELSEWHERE,
        Refusal::ProvidedByAPart => key::SOFTWARE_REFUSAL_PROVIDED_BY_A_PART,
    }
}

/// What a job of the software engine is doing, as a phrase — for the queue's
/// row and for a sheet's "working" value.
#[must_use]
pub const fn phase_key(phase: Phase) -> &'static str {
    use crate::key;
    match phase {
        Phase::Resolving => key::SOFTWARE_PHASE_RESOLVING,
        Phase::Waiting => key::SOFTWARE_PHASE_WAITING,
        Phase::Downloading => key::SOFTWARE_PHASE_DOWNLOADING,
        Phase::Verifying => key::SOFTWARE_PHASE_VERIFYING,
        Phase::Unpacking => key::SOFTWARE_PHASE_UNPACKING,
        Phase::Building => key::SOFTWARE_PHASE_BUILDING,
        Phase::Installing => key::SOFTWARE_PHASE_INSTALLING,
        Phase::Integrating => key::SOFTWARE_PHASE_INTEGRATING,
        Phase::Removing => key::SOFTWARE_PHASE_REMOVING,
        Phase::Starting => key::SOFTWARE_PHASE_STARTING,
        Phase::Indexing => key::SOFTWARE_PHASE_INDEXING,
        Phase::Preparing => key::SOFTWARE_PHASE_PREPARING,
        Phase::Declaring => key::SOFTWARE_DECLARED_PHASE_DECLARING,
        Phase::Applying => key::SOFTWARE_DECLARED_PHASE_APPLYING,
        Phase::Reconciling => key::SOFTWARE_DECLARED_PHASE_RECONCILING,
        Phase::Publishing => key::SOFTWARE_DECLARED_PHASE_PUBLISHING,
    }
}

/// What an order DID, as the history says it.
///
/// THE EXHAUSTIVE MAP, which fails to compile the day an order is added; the
/// history reads WORDS through [`order_key_of_word`], and a test holds the two
/// to the same sentence per order. Nothing on screen holds an `Order` whose
/// past tense it says — a job under way is not "installed" yet.
#[cfg(test)]
#[must_use]
pub const fn order_key(order: &ronoctua_software::Order) -> &'static str {
    use crate::key;
    use ronoctua_software::Order;
    match order {
        Order::Install { .. } => key::SOFTWARE_ORDER_INSTALL,
        Order::InstallFile { .. } => key::SOFTWARE_ORDER_INSTALL_FILE,
        Order::Remove { .. } => key::SOFTWARE_ORDER_REMOVE,
        Order::Upgrade { .. } => key::SOFTWARE_ORDER_UPGRADE,
        Order::Tidy { .. } => key::SOFTWARE_ORDER_TIDY,
        Order::Try { .. } => key::SOFTWARE_ORDER_TRY,
        Order::StopTrial { .. } => key::SOFTWARE_ORDER_STOP_TRIAL,
        Order::Keep { .. } => key::SOFTWARE_ORDER_KEEP,
        Order::Prepare { .. } => key::SOFTWARE_ORDER_PREPARE,
        Order::Refresh { .. } => key::SOFTWARE_ORDER_REFRESH,
        Order::Rollback { .. } => key::SOFTWARE_ORDER_ROLLBACK,
        Order::WebCreate { .. } => key::SOFTWARE_ORDER_WEB_CREATE,
        Order::WebEdit { .. } => key::SOFTWARE_ORDER_WEB_EDIT,
        Order::AndroidSession { .. } => key::SOFTWARE_ORDER_ANDROID_SESSION,
        Order::AndroidErase => key::SOFTWARE_ORDER_ANDROID_ERASE,
        Order::Permission { .. } => key::SOFTWARE_ORDER_PERMISSION,
        Order::AndroidWindows { .. } => key::SOFTWARE_ORDER_ANDROID_WINDOWS,
        Order::Declare { .. } => key::SOFTWARE_ORDER_DECLARE,
        Order::Forget { .. } => key::SOFTWARE_ORDER_FORGET,
        Order::Decline { .. } => key::SOFTWARE_ORDER_DECLINE,
        Order::Undecline { .. } => key::SOFTWARE_ORDER_UNDECLINE,
        Order::Reconcile { .. } => key::SOFTWARE_ORDER_RECONCILE,
        Order::RefreshPins => key::SOFTWARE_ORDER_REFRESH_PINS,
    }
}

/// The same, from the WORD the history file keeps (`journal::Entry::order`).
///
/// `None` for a word this build does not know — a history line written by a
/// newer CLI — which the row draws as the word itself rather than as a
/// bracketed key. The table is walked in [`Order::WORDS`]' order, and a test
/// proves every one of them has a sentence.
#[must_use]
pub fn order_key_of_word(word: &str) -> Option<&'static str> {
    use crate::key;
    Some(match word {
        "install" => key::SOFTWARE_ORDER_INSTALL,
        "install-file" => key::SOFTWARE_ORDER_INSTALL_FILE,
        "remove" => key::SOFTWARE_ORDER_REMOVE,
        "upgrade" => key::SOFTWARE_ORDER_UPGRADE,
        "tidy" => key::SOFTWARE_ORDER_TIDY,
        "try" => key::SOFTWARE_ORDER_TRY,
        "stop-trial" => key::SOFTWARE_ORDER_STOP_TRIAL,
        "keep" => key::SOFTWARE_ORDER_KEEP,
        "prepare" => key::SOFTWARE_ORDER_PREPARE,
        "refresh" => key::SOFTWARE_ORDER_REFRESH,
        "rollback" => key::SOFTWARE_ORDER_ROLLBACK,
        "web-create" => key::SOFTWARE_ORDER_WEB_CREATE,
        "web-edit" => key::SOFTWARE_ORDER_WEB_EDIT,
        "android-session" => key::SOFTWARE_ORDER_ANDROID_SESSION,
        "android-erase" => key::SOFTWARE_ORDER_ANDROID_ERASE,
        "permission" => key::SOFTWARE_ORDER_PERMISSION,
        "android-windows" => key::SOFTWARE_ORDER_ANDROID_WINDOWS,
        "declare" => key::SOFTWARE_ORDER_DECLARE,
        "forget" => key::SOFTWARE_ORDER_FORGET,
        "decline" => key::SOFTWARE_ORDER_DECLINE,
        "undecline" => key::SOFTWARE_ORDER_UNDECLINE,
        "reconcile" => key::SOFTWARE_ORDER_RECONCILE,
        "refresh-pins" => key::SOFTWARE_ORDER_REFRESH_PINS,
        // ── anchor: L18 ──
        _ => return None,
    })
}

/// One thing a Flatpak may reach, as the row that changes it names it.
#[must_use]
pub const fn grant_key(grant: Grant) -> &'static str {
    use crate::key;
    match grant {
        Grant::Files { .. } => key::SOFTWARE_GRANT_FILES,
        Grant::Network { .. } => key::SOFTWARE_GRANT_NETWORK,
        Grant::Sound { .. } => key::SOFTWARE_GRANT_SOUND,
        Grant::Devices { .. } => key::SOFTWARE_GRANT_DEVICES,
        Grant::Display { .. } => key::SOFTWARE_GRANT_DISPLAY,
        Grant::UnsetGtkTheme => key::SOFTWARE_GRANT_UNSET_GTK_THEME,
        Grant::Reset => key::SOFTWARE_GRANT_RESET,
    }
}

/// How far into the person's files a Flatpak reaches.
#[must_use]
pub const fn reach_key(reach: Reach) -> &'static str {
    use crate::key;
    match reach {
        Reach::ReadWrite => key::SOFTWARE_REACH_READ_WRITE,
        Reach::ReadOnly => key::SOFTWARE_REACH_READ_ONLY,
        Reach::Nothing => key::SOFTWARE_REACH_NOTHING,
    }
}

/// Which display protocol a Flatpak draws through.
#[must_use]
pub const fn display_key(display: Display) -> &'static str {
    use crate::key;
    match display {
        Display::Wayland => key::SOFTWARE_DISPLAY_WAYLAND,
        Display::X11 => key::SOFTWARE_DISPLAY_X11,
    }
}

/// What a trial may leave behind.
#[must_use]
pub const fn traces_key(traces: Traces) -> &'static str {
    use crate::key;
    match traces {
        Traces::Scratch => key::SOFTWARE_TRACES_SCRATCH,
        Traces::Home => key::SOFTWARE_TRACES_HOME,
        Traces::Sandbox => key::SOFTWARE_TRACES_SANDBOX,
    }
}

/// Which Android image.
#[must_use]
pub const fn image_key(image: AndroidImage) -> &'static str {
    use crate::key;
    match image {
        AndroidImage::Gapps => key::SOFTWARE_ANDROID_GAPPS,
        AndroidImage::Vanilla => key::SOFTWARE_ANDROID_VANILLA,
    }
}

/// Which source is recommended first.
#[must_use]
pub const fn prefer_key(prefer: Prefer) -> &'static str {
    use crate::key;
    match prefer {
        Prefer::NixFirst => key::SOFTWARE_PREFER_NIX_FIRST,
        Prefer::FlatpakFirst => key::SOFTWARE_PREFER_FLATPAK_FIRST,
    }
}

/// Who put a program there.
#[must_use]
pub const fn origin_key(origin: Origin) -> &'static str {
    use crate::key;
    match origin {
        Origin::Mine => key::SOFTWARE_ORIGIN_MINE,
        Origin::MineDeclared => key::SOFTWARE_DECLARED_ORIGIN_MINE_DECLARED,
        Origin::Distro { declinable: true } => key::SOFTWARE_DECLARED_ORIGIN_DISTRO,
        Origin::Distro { declinable: false } => key::SOFTWARE_DECLARED_ORIGIN_ESSENTIAL,
        Origin::Declared => key::SOFTWARE_ORIGIN_DECLARED,
    }
}

// ─── the declaration's words ─────────────────────────────────────────────────

/// How this machine stands to a declared entry, as a phrase: applied, on
/// its way (or not arrived), waiting, elsewhere, or the reason it was left
/// out.
#[must_use]
pub const fn standing_key(standing: &Standing) -> &'static str {
    use crate::key;
    match standing {
        Standing::Applied => key::SOFTWARE_DECLARED_STANDING_APPLIED,
        Standing::Arriving { failure: None } => key::SOFTWARE_DECLARED_STANDING_ARRIVING,
        Standing::Arriving { failure: Some(_) } => key::SOFTWARE_DECLARED_STANDING_NOT_ARRIVED,
        Standing::Waiting => key::SOFTWARE_DECLARED_STANDING_WAITING,
        Standing::NextRestart => key::SOFTWARE_DECLARED_STANDING_NEXT_RESTART,
        Standing::Elsewhere => key::SOFTWARE_DECLARED_STANDING_ELSEWHERE,
        Standing::Skipped { reason, .. } => skip_key(*reason),
    }
}

/// Why this machine left a declared entry out, as a sentence.
#[must_use]
pub const fn skip_key(reason: SkipWhy) -> &'static str {
    use crate::key;
    match reason {
        SkipWhy::AttrMissing => key::SOFTWARE_DECLARED_SKIPPED_ATTR_MISSING,
        SkipWhy::NotAPackage => key::SOFTWARE_DECLARED_SKIPPED_NOT_A_PACKAGE,
        SkipWhy::RefusedByMeta => key::SOFTWARE_DECLARED_SKIPPED_REFUSED_BY_META,
        SkipWhy::Held => key::SOFTWARE_DECLARED_SKIPPED_HELD,
        SkipWhy::SourceOff => key::SOFTWARE_DECLARED_SKIPPED_SOURCE_OFF,
        SkipWhy::OtherMachine => key::SOFTWARE_DECLARED_SKIPPED_OTHER_MACHINE,
        SkipWhy::FormatUnknown => key::SOFTWARE_DECLARED_SKIPPED_FORMAT_UNKNOWN,
    }
}

/// Why the update held an entry back (`local/held.json`'s reasons, which
/// `person-applied.json` carries as the detail of a `held` skip). `None` for
/// a word this build does not know, which the row draws as it is: it is
/// data the machine wrote.
#[must_use]
pub fn held_key(reason: &str) -> Option<&'static str> {
    use crate::key;
    Some(match reason {
        "eval-failed" => key::SOFTWARE_DECLARED_HELD_EVAL_FAILED,
        "fetch-failed" => key::SOFTWARE_DECLARED_HELD_FETCH_FAILED,
        "upstream-changed" => key::SOFTWARE_DECLARED_HELD_UPSTREAM_CHANGED,
        "build-failed" => key::SOFTWARE_DECLARED_HELD_BUILD_FAILED,
        _ => return None,
    })
}

/// What a decline did here, as a phrase.
#[must_use]
pub const fn decline_key(state: &DeclineState) -> &'static str {
    use crate::key;
    match state {
        DeclineState::Honoured => key::SOFTWARE_DECLARED_DECLINE_HONOURED,
        DeclineState::StillPresent => key::SOFTWARE_DECLARED_DECLINE_STILL_PRESENT,
        DeclineState::Essential { .. } => key::SOFTWARE_DECLARED_DECLINE_ESSENTIAL,
        DeclineState::Unknown => key::SOFTWARE_DECLARED_DECLINE_UNKNOWN,
        DeclineState::Elsewhere => key::SOFTWARE_DECLARED_DECLINE_ELSEWHERE,
        DeclineState::Waiting => key::SOFTWARE_DECLARED_DECLINE_WAITING,
        DeclineState::NextRestart => key::SOFTWARE_DECLARED_DECLINE_NEXT_RESTART,
    }
}

/// The stem of the sentence that says why the distro needs a program: the
/// registry's reason (`assets/distro-apps.nix`, `essential`) is its id, and
/// `essential-apps-say-why` holds every reason to a sentence in every
/// language — so the key is the reason's own, and a reason added there needs
/// no arm here.
pub const ESSENTIAL_WHY: &str = "software-essential-why-";

/// Why the distro needs a program, as its sentence. A reason with no
/// sentence in this build (a system newer than the texts it was built with)
/// is said by its id: data the machine wrote, never a made-up phrase.
#[must_use]
pub fn essential_said(reason: &str, texts: &ronoctua_i18n::Texts) -> String {
    let key = format!("{ESSENTIAL_WHY}{reason}");
    if texts.has(&key) {
        texts.t(&key)
    } else {
        reason.to_owned()
    }
}

/// A web app's freedesktop category, as a word. `None` for a category this
/// build does not offer — a `.desktop` edited by hand — which the row draws
/// as written: it is data somebody typed.
#[must_use]
pub fn category_key(category: &str) -> Option<&'static str> {
    use crate::key;
    Some(match category {
        "AudioVideo" => key::SOFTWARE_CATEGORY_AUDIOVIDEO,
        "Audio" => key::SOFTWARE_CATEGORY_AUDIO,
        "Video" => key::SOFTWARE_CATEGORY_VIDEO,
        "Development" => key::SOFTWARE_CATEGORY_DEVELOPMENT,
        "Education" => key::SOFTWARE_CATEGORY_EDUCATION,
        "Game" => key::SOFTWARE_CATEGORY_GAME,
        "Graphics" => key::SOFTWARE_CATEGORY_GRAPHICS,
        "Network" => key::SOFTWARE_CATEGORY_NETWORK,
        "Office" => key::SOFTWARE_CATEGORY_OFFICE,
        "Science" => key::SOFTWARE_CATEGORY_SCIENCE,
        "Settings" => key::SOFTWARE_CATEGORY_SETTINGS,
        "System" => key::SOFTWARE_CATEGORY_SYSTEM,
        "Utility" => key::SOFTWARE_CATEGORY_UTILITY,
        _ => return None,
    })
}

/// How one source stands for the current search, as a sentence.
///
/// `from` of an answer is DATA — the Nix index's revision, a catalogue's date
/// — and it is said only when there is one: an answer based on nothing in
/// particular needs no sentence at all, so `None` comes back and no row is
/// drawn for it.
#[must_use]
pub fn status_said(status: &Status, texts: &ronoctua_i18n::Texts) -> Option<String> {
    use crate::key;
    Some(match status {
        Status::Answered { from: Some(from) } => {
            texts.t_with(key::SOFTWARE_STATUS_FROM, &[("from", from.as_str())])
        }
        Status::Answered { from: None } => return None,
        // THE SOURCE IS THE ROW'S LABEL, so the value does not name it again.
        Status::Asking => texts.t(key::SOFTWARE_ASKING),
        Status::Refused { refusal } => texts.t(refusal_key(*refusal)),
        Status::Off => texts.t(key::SOFTWARE_STATUS_OFF),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_software::Order;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    fn every_language() -> Vec<ronoctua_i18n::Texts> {
        ronoctua_i18n::LANGUAGES.iter().map(|l| texts(l)).collect()
    }

    /// No bracket, no emptiness — the two ways a missing key shows.
    fn said_well(t: &ronoctua_i18n::Texts, key: &str) {
        let s = t.t(key);
        assert!(
            !s.is_empty() && !s.starts_with('['),
            "{key} in {}: {s:?}",
            t.language()
        );
    }

    #[test]
    fn every_refusal_has_a_sentence_of_its_own_in_every_language() {
        for t in every_language() {
            let said: Vec<String> = Refusal::ALL.iter().map(|r| t.t(refusal_key(*r))).collect();
            for r in Refusal::ALL {
                said_well(&t, refusal_key(r));
            }
            // A SENTENCE PER REASON: two refusals that read the same are one
            // answer to two questions, and the person cannot tell which.
            let mut unique = said.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(unique.len(), said.len(), "{}: {said:?}", t.language());
        }
    }

    #[test]
    fn every_source_badge_phase_and_order_word_has_text() {
        for t in every_language() {
            for s in Source::ALL {
                said_well(&t, source_key(s));
                said_well(&t, why_key(s));
            }
            for b in Badge::ALL {
                said_well(&t, badge_key(b));
            }
            for p in Phase::ALL {
                said_well(&t, phase_key(p));
            }
            for w in Order::WORDS {
                let k = order_key_of_word(w).unwrap_or_else(|| panic!("no sentence for `{w}`"));
                said_well(&t, k);
            }
            for x in Traces::ALL {
                said_well(&t, traces_key(x));
            }
            for i in AndroidImage::ALL {
                said_well(&t, image_key(i));
            }
            for p in Prefer::ALL {
                said_well(&t, prefer_key(p));
            }
            for o in Origin::ALL {
                said_well(&t, origin_key(o));
            }
            for s in [
                Standing::Applied,
                Standing::Waiting,
                Standing::NextRestart,
                Standing::Elsewhere,
            ] {
                said_well(&t, standing_key(&s));
            }
            for r in SkipWhy::ALL {
                said_well(&t, skip_key(r));
            }
            for h in [
                "eval-failed",
                "fetch-failed",
                "upstream-changed",
                "build-failed",
            ] {
                said_well(
                    &t,
                    held_key(h).unwrap_or_else(|| panic!("no sentence for {h}")),
                );
            }
            for d in [
                DeclineState::Honoured,
                DeclineState::StillPresent,
                DeclineState::Essential {
                    reason: "the-login-shell".into(),
                },
                DeclineState::Unknown,
                DeclineState::Elsewhere,
                DeclineState::Waiting,
                DeclineState::NextRestart,
            ] {
                said_well(&t, decline_key(&d));
            }
            // THE REGISTRY'S REASON is read by its own key, and that key is
            // the one `essential-apps-say-why` holds to a sentence.
            assert_eq!(
                essential_said("the-login-shell", &t),
                t.t(crate::key::SOFTWARE_ESSENTIAL_WHY_THE_LOGIN_SHELL)
            );
            assert_eq!(essential_said("not-a-reason-yet", &t), "not-a-reason-yet");
            for g in every_grant() {
                said_well(&t, grant_key(g));
            }
            for r in [Reach::ReadWrite, Reach::ReadOnly, Reach::Nothing] {
                said_well(&t, reach_key(r));
            }
            for d in [Display::Wayland, Display::X11] {
                said_well(&t, display_key(d));
            }
            for c in ronoctua_software::web::CATEGORIES {
                said_well(
                    &t,
                    category_key(c).unwrap_or_else(|| panic!("no word for {c}")),
                );
            }
        }
    }

    /// One of every grant, so a new one without a word fails here.
    fn every_grant() -> [Grant; 7] {
        [
            Grant::Files {
                reach: Reach::ReadOnly,
            },
            Grant::Network { on: true },
            Grant::Sound { on: true },
            Grant::Devices { on: false },
            Grant::Display {
                through: Display::X11,
            },
            Grant::UnsetGtkTheme,
            Grant::Reset,
        ]
    }

    #[test]
    fn every_grant_reach_and_display_word_is_covered() {
        // THE CRATE'S WORD LISTS are the census: as many sentences as words,
        // and each word's own.
        let grants: Vec<&str> = every_grant().iter().map(Grant::word).collect();
        assert_eq!(grants, Grant::WORDS);
        let reaches = [Reach::ReadWrite, Reach::ReadOnly, Reach::Nothing].map(Reach::word);
        assert_eq!(reaches, Reach::WORDS);
        let displays = [Display::Wayland, Display::X11].map(Display::word);
        assert_eq!(displays, Display::WORDS);
        for t in every_language() {
            let mut said: Vec<String> = every_grant().iter().map(|g| t.t(grant_key(*g))).collect();
            said.sort();
            said.dedup();
            assert_eq!(said.len(), Grant::WORDS.len(), "{}", t.language());
        }
    }

    #[test]
    fn the_word_map_and_the_order_map_agree() {
        // THE TWO WAYS IN name the same sentence: the history reads a WORD and
        // the queue holds an ORDER, and one of each must never read apart.
        let key = ronoctua_software::AppKey::Nix {
            attr: "hello".into(),
        };
        let orders = [
            Order::Install {
                key: key.clone(),
                consent: Default::default(),
            },
            Order::Remove {
                key,
                keep_data: true,
            },
            Order::AndroidErase,
            Order::Rollback { generation: 2 },
            Order::Permission {
                id: "org.gimp.GIMP".into(),
                grant: Grant::Reset,
            },
            Order::AndroidWindows { multi: true },
        ];
        for o in orders {
            assert_eq!(
                order_key_of_word(o.word()),
                Some(order_key(&o)),
                "{}",
                o.word()
            );
        }
        assert_eq!(order_key_of_word("never-heard-of-it"), None);
    }

    #[test]
    fn every_badge_and_source_has_its_own_glyph() {
        let mut glyphs: Vec<char> = Badge::ALL.iter().map(|b| badge_glyph(*b)).collect();
        let n = glyphs.len();
        glyphs.sort_unstable();
        glyphs.dedup();
        assert_eq!(glyphs.len(), n, "two badges share a drawing");
        let mut sources: Vec<char> = Source::ALL.iter().map(|s| source_glyph(*s)).collect();
        sources.push(tokens::icon::SOFTWARE_SOURCE_SYSTEM);
        let n = sources.len();
        sources.sort_unstable();
        sources.dedup();
        assert_eq!(sources.len(), n, "two sources share a drawing");
    }

    #[test]
    fn a_status_is_said_only_when_it_says_something() {
        let t = texts("en-GB");
        assert_eq!(status_said(&Status::Answered { from: None }, &t), None);
        assert!(status_said(&Status::Asking, &t).is_some());
        let from = status_said(
            &Status::Answered {
                from: Some("5b4f72e".into()),
            },
            &t,
        )
        .expect("a sentence");
        assert!(from.contains("5b4f72e"), "{from}");
        let refused = status_said(
            &Status::Refused {
                refusal: Refusal::Offline,
            },
            &t,
        )
        .expect("a sentence");
        assert_eq!(refused, t.t(refusal_key(Refusal::Offline)));
    }
}
