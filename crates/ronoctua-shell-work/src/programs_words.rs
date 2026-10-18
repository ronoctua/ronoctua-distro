//! THE PERSON'S PROGRAMS AS THE SHELL SAYS THEM, view or no view — what
//! the footer, which lives while the session does, and the software centre
//! (`ronoctua-shell-software`) both say and read: how a job of the queue
//! ended and the notification it raises, the modeline's two marks and the
//! reading they are drawn from, and the words the launcher opens the centre
//! with — on a program's sheet, or on a search.
//!
//! A trial runs and a job ends with nobody looking at the view, so these
//! are the footer's too; and the footer names no feature, so they stand
//! under both, here, beside the centre's words (`software_words`) a failure's
//! sentence starts from.

use ronoctua_software::state::Snapshot;
use ronoctua_software::{Context, Order, Source};
use ronoctua_tokens as tokens;

/// How a job of the software engine ended, when the end is worth a
/// notification (`assets/notifications.nix`, one entry each). Everything else
/// the view's own line said when it was queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// Installed — from a source, a file, or a trial kept.
    Installed,
    /// A web app was made.
    WebCreated,
    /// Removed.
    Removed,
    /// Updated.
    Updated,
    /// Android's image is ready.
    AndroidReady,
    /// It did not work.
    Failed,
}

impl Ended {
    /// Every one, for the tests that hold the words total — the footer's
    /// among them, which give each its notification.
    pub const ALL: [Self; 6] = [
        Self::Installed,
        Self::WebCreated,
        Self::Removed,
        Self::Updated,
        Self::AndroidReady,
        Self::Failed,
    ];
}

/// How a job ended, when it is a software job whose end is worth saying.
///
/// A cancelled job says nothing — the person cancelled it — and neither do
/// the orders whose end the screen already shows where they were given: a
/// trial starting, a ring turned, space freed, a catalogue fetched.
#[must_use]
pub fn ended_of(job: &ronoctua_transfers::Job) -> Option<Ended> {
    let ronoctua_transfers::Work::Software { order, .. } = &job.work else {
        return None;
    };
    match job.stage {
        ronoctua_transfers::Stage::Failed => return Some(Ended::Failed),
        ronoctua_transfers::Stage::Done => {}
        _ => return None,
    }
    match order.as_ref() {
        Order::Install { .. }
        | Order::InstallFile { .. }
        | Order::Keep { .. }
        | Order::Declare { .. } => Some(Ended::Installed),
        Order::WebCreate { .. } => Some(Ended::WebCreated),
        Order::Remove { .. } | Order::Forget { .. } => Some(Ended::Removed),
        Order::Upgrade { .. } => Some(Ended::Updated),
        Order::Prepare {
            source: Source::Android,
            ..
        } => Some(Ended::AndroidReady),
        Order::Prepare { .. }
        | Order::Tidy { .. }
        | Order::Try { .. }
        | Order::StopTrial { .. }
        | Order::Refresh { .. }
        | Order::Rollback { .. }
        | Order::WebEdit { .. }
        | Order::AndroidSession { .. }
        | Order::AndroidErase
        | Order::Permission { .. }
        | Order::AndroidWindows { .. }
        // ── anchor: L18 ──
        | Order::Decline { .. }
        | Order::Undecline { .. }
        | Order::Reconcile { .. }
        | Order::RefreshPins => None,
    }
}

/// Why a job failed, as a sentence: the refusal's own, else the program's
/// words, else the plain "did not work". Never the refusal's WORD, which is
/// the CLI's name for it and not a sentence.
#[must_use]
pub fn failure_said(job: &ronoctua_transfers::Job, texts: &ronoctua_i18n::Texts) -> String {
    match (job.refusal, job.trouble.as_deref()) {
        (Some(why), _) => texts.t(crate::software_words::refusal_key(why)),
        (None, Some(trouble)) if !trouble.trim().is_empty() => trouble.trim().to_owned(),
        (None, _) => texts.t(crate::key::SOFTWARE_FAILED),
    }
}

/// The modeline's two marks, with nothing to say at rest: a trial running,
/// and Android's session up (it costs memory). Each is one glyph, placed on
/// the rung "a thing is not as you would assume" the other quiet marks share
/// (the footer's `modeline::segments`), and each opens its own level when clicked.
#[must_use]
pub fn segments(running: &Running) -> Vec<ronoctua_render::layout::Segment> {
    use ronoctua_render::layout::{Segment, Shrink, Side};
    let mut out = Vec::new();
    if !running.trials.is_empty() {
        out.push(Segment {
            id: TRIAL_MARK,
            text: tokens::icon::SOFTWARE_TRIAL.to_string(),
            priority: 2,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    if running.android {
        out.push(Segment {
            id: ANDROID_MARK,
            text: tokens::icon::SOFTWARE_ANDROID.to_string(),
            priority: 2,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    out
}

/// The trial mark's segment id — what a click on it is recognised by.
pub const TRIAL_MARK: &str = "software-trial";

/// The Android mark's.
pub const ANDROID_MARK: &str = "software-android";

/// The notification an end raises: its summary's key and its body's, both
/// taking `$app` (and the failure's body `$why`).
#[must_use]
pub const fn ended_keys(ended: Ended) -> (&'static str, &'static str) {
    use crate::key;
    match ended {
        Ended::Installed => (
            key::NOTIF_SOFTWARE_INSTALLED_SUMMARY,
            key::NOTIF_SOFTWARE_INSTALLED_BODY,
        ),
        Ended::WebCreated => (
            key::NOTIF_SOFTWARE_WEB_CREATED_SUMMARY,
            key::NOTIF_SOFTWARE_WEB_CREATED_BODY,
        ),
        Ended::Removed => (
            key::NOTIF_SOFTWARE_REMOVED_SUMMARY,
            key::NOTIF_SOFTWARE_REMOVED_BODY,
        ),
        Ended::Updated => (
            key::NOTIF_SOFTWARE_UPDATED_SUMMARY,
            key::NOTIF_SOFTWARE_UPDATED_BODY,
        ),
        Ended::AndroidReady => (
            key::NOTIF_SOFTWARE_ANDROID_READY_SUMMARY,
            key::NOTIF_SOFTWARE_ANDROID_READY_BODY,
        ),
        Ended::Failed => (
            key::NOTIF_SOFTWARE_FAILED_SUMMARY,
            key::NOTIF_SOFTWARE_FAILED_BODY,
        ),
    }
}

/// What the modeline's two marks are drawn from, with no view open: the
/// trials whose units are alive, by key and name, and whether Android's
/// session is up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Running {
    /// The trials alive, by key and name.
    pub trials: Vec<(String, String)>,
    /// Android's session is up.
    pub android: bool,
}

impl Running {
    /// From a snapshot the feed read, which has both.
    #[must_use]
    pub fn of(s: &Snapshot) -> Self {
        Self {
            trials: s
                .trials
                .iter()
                .filter(|t| t.running)
                .map(|t| (t.key.clone(), t.name.clone()))
                .collect(),
            android: s.android.session,
        }
    }

    /// Whether either mark is up — what arms the reading when a window
    /// closes, and nothing else does.
    #[must_use]
    pub fn any(&self) -> bool {
        !self.trials.is_empty() || self.android
    }

    /// The names of the trials running before and not now: the ones that
    /// ended.
    #[must_use]
    pub fn ended_since(&self, before: &Self) -> Vec<String> {
        before
            .trials
            .iter()
            .filter(|(key, _)| !self.trials.iter().any(|(k, _)| k == key))
            .map(|(_, name)| name.clone())
            .collect()
    }
}

/// Read what the marks are drawn from, on a thread: the trials' units
/// (`systemctl is-active`) and Android's session. Asked for only on an event —
/// the footer starting, a software job ending, a window closing while a mark
/// is up — never on a clock.
pub fn read_running(ctx: Context, post: impl FnOnce(Running) + Send + 'static) {
    let name = "ronoctua-software-running";
    if let Err(e) = std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let trials = ronoctua_software::trial::trials(&ctx)
                .into_iter()
                .filter(|t| t.running)
                .map(|t| (t.key, t.name))
                .collect();
            let android =
                ctx.tools.waydroid.is_some() && ronoctua_software::android::status(&ctx).session;
            post(Running { trials, android });
        })
    {
        tracing::error!(error = %e, thread = name, "could not start a software thread");
    }
}

/// The launcher's `C-i`: the sheet of the program behind one of its entries
/// ([`sheet_argument`]). A command of the footer's own, by name only — the
/// launcher's door into this view.
pub const SHEET: &str = "software-sheet";

/// The launcher's `C-d`: the same sheet, with the remove question up and on
/// "no".
pub const SHEET_TO_REMOVE: &str = "software-sheet-remove";

/// What the launcher hands [`SHEET`] and [`SHEET_TO_REMOVE`]: the entry's
/// desktop id and its name as the launcher has it, one line each.
#[must_use]
pub fn sheet_argument(desktop_id: &str, name: &str) -> String {
    format!("{desktop_id}\n{name}")
}

/// The entry's desktop id and name back out of [`sheet_argument`] — `None`
/// for words it did not write (the command run by name, by hand).
#[must_use]
pub fn sheet_of(argument: Option<&str>) -> Option<(&str, &str)> {
    argument
        .and_then(|a| a.split_once('\n'))
        .filter(|(id, _)| !id.trim().is_empty())
}

/// The word before a search's own words: `software search <words>` opens on
/// what a search for them finds — what typing them into the window would,
/// with the words given on the command line — and what the launcher hands
/// `software-open` to search the centre for what was typed there.
pub const SEARCH_WORD: &str = "search";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trial_that_stopped_running_is_said_once_and_the_marks_follow_the_reading() {
        let before = Running {
            trials: vec![
                ("hello".into(), "hello".into()),
                ("gimp".into(), "GIMP".into()),
            ],
            android: true,
        };
        let now = Running {
            trials: vec![("gimp".into(), "GIMP".into())],
            android: false,
        };
        assert_eq!(now.ended_since(&before), vec!["hello".to_owned()]);
        assert!(now.ended_since(&now).is_empty());
        assert!(now.any());
        assert!(!Running::default().any(), "nothing at rest");
    }

    /// FAILS IF: the entry the launcher's sheet names comes back as another
    /// one, or words it did not write — the command run by name, by hand —
    /// are taken for an entry.
    #[test]
    fn a_sheets_argument_names_its_entry() {
        let said = sheet_argument("org.gimp.GIMP.desktop", "GIMP");
        assert_eq!(
            sheet_of(Some(&said)),
            Some(("org.gimp.GIMP.desktop", "GIMP"))
        );
        assert_eq!(sheet_of(Some("org.gimp.GIMP.desktop")), None);
        assert_eq!(sheet_of(Some("\nGIMP")), None);
        assert_eq!(sheet_of(None), None);
    }
}
