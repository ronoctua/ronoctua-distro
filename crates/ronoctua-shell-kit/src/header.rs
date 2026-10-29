//! WHETHER A WINDOW WEARS ITS HEADER, and whether the footer shows the keys it
//! gave up — the two switches of the "no header" mode, read where a WINDOW can
//! reach them.
//!
//! # Why they live here and not with the rest of the modeline's switches
//!
//! The footer owns `modeline.json` (`ronoctua_shell_footer::adapt::Prefs`): it
//! writes it and watches it. But a window is a SEPARATE process, and it cannot
//! read the footer's `Prefs` — the door depends on this crate, never on the
//! footer. So the two switches that a window must obey — is my header drawn at
//! all, and (the footer's own concern) are the keys I handed over shown — are
//! declared HERE, the crate every door already shares, and the footer reuses
//! them in its `Prefs`. One spelling of `"hidden"`/`"shown"` on the wire, and
//! one default, for the file both sides read.
//!
//! A window reads a SUBSET of that file — these two fields and nothing else —
//! the way the locale and the theme are read across processes
//! (`crate::when::prefs`, `crate::theme`): `ronoctua_store::Store`, a watch on
//! the file (`Notice::on_file`), and a process-wide value adopted at start and
//! on every ring. The footer stays the only writer; a window only ever reads.
//!
//! # Why the default is HIDDEN
//!
//! The distro ships "no header, keys in the minibuffer" (the owner's ask). A
//! field a hand never changed is absent from `modeline.json`, so ABSENT MUST BE
//! HIDDEN — which a raw `bool` cannot be under `#[serde(default)]` without a
//! hand-written `Default`. A two-word enum with a `#[default]` variant says it
//! plainly and serialises as one word, exactly as `build_switch::Show` does.

use serde::{Deserialize, Serialize};

/// Whether a window draws its header — the top line that carries the title and
/// the window's shortcuts. ABSENT (the distro default) IS `Hidden`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Header {
    /// No header: the window's list reclaims line 0, and its shortcuts move to
    /// the footer's minibuffer while it is focused.
    #[default]
    Hidden,
    /// The header is drawn, as it always was.
    Shown,
}

impl Header {
    /// Whether it is the distro default — so `modeline.json` keeps nothing for
    /// a setting nobody changed, the house rule for every switch in that file.
    #[must_use]
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Hidden)
    }

    /// The other of the two — what `RET` on its row, or a click on its glyph,
    /// turns it into.
    #[must_use]
    pub const fn toggle(self) -> Self {
        match self {
            Self::Hidden => Self::Shown,
            Self::Shown => Self::Hidden,
        }
    }
}

/// Whether the footer's minibuffer shows the shortcuts a hidden header gave up.
/// ABSENT (the distro default) IS `Hidden`. Only ever consulted while the
/// header is hidden — with a header on screen the keys are on it, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shortcuts {
    /// The minibuffer does not show the relocated shortcuts — the glyph beside
    /// the chevron turns them on.
    #[default]
    Hidden,
    /// The focused window's shortcuts are shown on the free minibuffer.
    Shown,
}

impl Shortcuts {
    /// Whether it is the distro default — see [`Header::is_default`].
    #[must_use]
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Hidden)
    }

    /// The other of the two.
    #[must_use]
    pub const fn toggle(self) -> Self {
        match self {
            Self::Hidden => Self::Shown,
            Self::Shown => Self::Hidden,
        }
    }
}

/// The file the switches live in — the footer's, read here field by field.
pub const FILE: &str = "modeline.json";

/// The two switches a window reads out of `modeline.json`, and NOTHING else of
/// it. `#[serde(default)]` so every other field the footer keeps there —
/// `adapt`, `hours`, `hidden`, `build_show` — is passed over, and a file that
/// has neither switch reads as the distro default (both `Hidden`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Switches {
    /// Whether windows wear their header.
    pub header: Header,
    /// Whether the footer shows a hidden header's shortcuts.
    pub shortcuts: Shortcuts,
}

/// What this process is obeying right now.
///
/// Read on every paint that asks whether the header is drawn, so it is a lock
/// held for the length of a copy of two words, never a file — the shape
/// `crate::when::prefs::current` already has, and for the same reason.
static CURRENT: std::sync::RwLock<Option<Switches>> = std::sync::RwLock::new(None);

/// The switches in effect now.
#[must_use]
pub fn current() -> Switches {
    CURRENT.read().map_or_else(
        |poisoned| poisoned.into_inner().unwrap_or_default(),
        |guard| guard.unwrap_or_default(),
    )
}

/// Whether windows must draw NO header.
#[must_use]
pub fn hide_header() -> bool {
    matches!(current().header, Header::Hidden)
}

/// Whether the footer must NOT show a hidden header's shortcuts.
#[must_use]
pub fn hide_shortcuts() -> bool {
    matches!(current().shortcuts, Shortcuts::Hidden)
}

/// Re-reads the file into [`current`]. Called at start and when the watch rings
/// — the two moments the theme and the locale are adopted.
pub fn adopt() {
    let switches = read();
    if let Ok(mut guard) = CURRENT.write() {
        *guard = Some(switches);
    }
}

/// Sets [`current`] directly — for tests, and for a process that has just
/// written the file and must not race its own watch.
pub fn set(switches: Switches) {
    if let Ok(mut guard) = CURRENT.write() {
        *guard = Some(switches);
    }
}

/// What the file says now, or the distro default — it never fails: a file that
/// will not parse costs the two switches, which read as `Hidden`, and the
/// footer's own reader is what says so on the screen.
#[must_use]
pub fn read() -> Switches {
    let Some(dir) = ronoctua_store::config_dir() else {
        return Switches::default();
    };
    read_at(&dir.join(FILE))
}

/// [`read`], with its path as a parameter — `<dir>/modeline.json`.
#[must_use]
pub fn read_at(path: &std::path::Path) -> Switches {
    let Some(dir) = path.parent() else {
        return Switches::default();
    };
    ronoctua_store::Store::<Switches>::at(Some(dir.to_path_buf()), FILE).read()
}

/// A watch on the file, ringing `on_stir` from the watcher's thread — how a
/// window hears the mode turned in the footer's settings, with nothing between
/// them but the file.
///
/// `None`, with a line in the log, when there is no configuration directory or
/// no inotify instance left: the process then adopts the change at its next
/// start, which is not silent.
pub fn watch<F>(on_stir: F) -> Option<ronoctua_files::notice::Notice>
where
    F: Fn() -> bool + Send + 'static,
{
    watch_in(&ronoctua_store::config_dir()?, on_stir)
}

/// [`watch`], with its root as a parameter — the directory is made here,
/// because a watch needs one to sit on and a window that starts before anybody
/// ever changed the mode is the common case.
pub fn watch_in<F>(dir: &std::path::Path, on_stir: F) -> Option<ronoctua_files::notice::Notice>
where
    F: Fn() -> bool + Send + 'static,
{
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(error = %e, "no configuration directory to watch modeline.json in");
        return None;
    }
    // AT ONCE, like the locale's and the language's: what arrives is a hand on
    // the footer's settings, and the window is on the screen beside it.
    match ronoctua_files::notice::Notice::on_file(
        dir,
        FILE,
        ronoctua_files::notice::Pace::AtOnce,
        on_stir,
    ) {
        Ok(n) => Some(n),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "no modeline watcher; the mode reaches this window at its next start"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the distro default stops being "no header, keys hidden", or a
    /// switch stops being omitted from the wire when it is at its default.
    #[test]
    fn the_default_is_hidden_and_absent() {
        let d = Switches::default();
        assert!(matches!(d.header, Header::Hidden));
        assert!(matches!(d.shortcuts, Shortcuts::Hidden));
        assert!(d.header.is_default() && d.shortcuts.is_default());
        // THE DEFAULT SERIALISES AS NOTHING the footer would keep — the two are
        // skipped there by `is_default`, and here the words read back whole.
        assert_eq!(Header::default().toggle(), Header::Shown);
        assert_eq!(Shortcuts::default().toggle(), Shortcuts::Shown);
    }

    /// FAILS IF: a window stops reading only its two fields — the other
    /// switches of the footer's file must be passed over, not refused.
    #[test]
    fn it_reads_only_its_two_fields_out_of_the_footers_file() {
        let root =
            std::env::temp_dir().join(format!("ronoctua-header-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        let path = root.join(FILE);
        // THE FOOTER'S FILE, with fields this reader does not know, and one it
        // does: the hour another door turned on, and the header a hand showed.
        std::fs::write(
            &path,
            br#"{"hours":true,"adapt":{"kiosk":false},"header":"shown"}"#,
        )
        .expect("seed");
        let s = read_at(&path);
        assert!(matches!(s.header, Header::Shown), "the field it knows");
        assert!(
            matches!(s.shortcuts, Shortcuts::Hidden),
            "an absent switch is the default, not a refusal"
        );
        // A FILE THAT WILL NOT PARSE costs the two switches, never panics.
        std::fs::write(&path, br#"{"header":"show"#).expect("break it");
        assert_eq!(read_at(&path), Switches::default());
        let _ = std::fs::remove_dir_all(&root);
    }
}
