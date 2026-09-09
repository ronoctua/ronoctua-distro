//! Which language this process speaks: where the answer comes from, how it is
//! kept, and how the other doors hear about a change.
//!
//! # Three sources, in order
//!
//! 1. `$XDG_CONFIG_HOME/ronoctua/language.json` — what the person chose with
//!    `LEADER s l`. Theirs, hand-editable, and it wins.
//! 2. `LC_ALL`, `LC_MESSAGES`, `LANG` — POSIX's order, and the machine's
//!    default: the NixOS option `ronoctua.language` is what puts a name there,
//!    and `nix/modules/base.nix` generates the glibc locale of every declared
//!    language precisely so it CAN be there.
//! 3. `ronoctua_i18n::DEFAULT`, the first of `assets/i18n/languages.nix`.
//!
//! It is the shape `ronoctua_power::prefs` established — file over environment
//! over ground — and it is written here rather than shared because the two
//! answer about different things and would only be one function by coincidence.
//!
//! # Why a file at all, when `LANG` already says
//!
//! Because `LANG` is the MACHINE's answer and it takes a rebuild to change:
//! it comes from the NixOS module, and invariant 6 says nothing imperative.
//! Changing the interface's language was therefore a rebuild, for a setting
//! that is two values and belongs to whoever is reading the screen.
//!
//! The two axes stay apart, and `nix/modules/host.nix` says so at length: the
//! LOCALE decides date order, decimal mark, units and paper, and there are
//! hundreds of them; the LANGUAGE decides which of two translations is drawn.
//! This file moves the second one only, and the screen that offers it says so
//! rather than implying the first followed.
//!
//! # The file is the broadcast
//!
//! Same as the theme, for the same reason: the footer, the file manager's
//! window, the notification, update, display, power, network and bluetooth
//! windows and every buffer's host are separate processes, and the control
//! socket only goes INTO the footer. So nobody is told: the file is written
//! atomically and every door holds an inotify watch on it ([`watch`]). A door
//! that was not running picks the change up at its next start, from the same
//! file — no second vocabulary on the socket.

use std::path::{Path, PathBuf};

/// The file, under `$XDG_CONFIG_HOME/ronoctua`: `{ "language": "pt-BR" }`.
pub const FILE: &str = "language.json";

/// Where a language may come from. It is what the screen reports, so that
/// "this machine is in English" and "you chose English" are distinguishable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The person's own file.
    File,
    /// The environment — which on an installed machine is the NixOS option.
    Env,
    /// Nobody said, so the first declared language.
    Default,
}

/// How fast the watch answers.
///
/// [`ronoctua_files::notice::Pace::AtOnce`], the same as the theme's and for
/// the same reason: what a change costs here is a REPAINT, not a `readdir` per
/// event, and the burst a hold-off exists to absorb is a person's hand moving
/// down a list of two.
const PACE: ronoctua_files::notice::Pace = ronoctua_files::notice::Pace::AtOnce;

/// The configuration directory, when there is one.
fn home() -> Option<PathBuf> {
    ronoctua_store::config_dir()
}

/// The language named by a file, if it names one that exists.
///
/// # Errors
///
/// The file is there and does not parse. A file that is ABSENT is not an
/// error: it is every machine on its first morning.
fn read_choice(path: &Path) -> Result<Option<String>, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let parsed: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(parsed
        .get("language")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned))
}

/// `pt_BR.UTF-8` -> `pt-BR`, the way `ronoctua_i18n::Texts::from_env` does it.
///
/// Written here as well because the two ask different questions: that one
/// wants a bundle and falls back silently, this one wants to SAY which source
/// answered and what was wrong with the ones it passed.
fn tag_of(value: &str) -> String {
    value.split('.').next().unwrap_or("").replace('_', "-")
}

/// The language the sources name, where it came from, and what was wrong with
/// the sources that were passed over.
///
/// PURE: the directory and the environment values are parameters, so a test
/// never reads the real home of whoever runs it.
#[must_use]
pub fn resolve_in(dir: Option<&Path>, env: &[Option<&str>]) -> (String, Source, Vec<String>) {
    let mut complaints = Vec::new();
    if let Some(dir) = dir {
        match read_choice(&dir.join(FILE)) {
            Ok(None) => {}
            Ok(Some(id)) => {
                if exists(&id) {
                    return (id, Source::File, complaints);
                }
                complaints.push(format!("{FILE} names an unknown language `{id}`"));
            }
            Err(e) => complaints.push(format!("{FILE}: {e}")),
        }
    }
    for value in env.iter().flatten() {
        let tag = tag_of(value);
        if exists(&tag) {
            return (tag, Source::Env, complaints);
        }
        // NOT a complaint. `LANG` carries locales this system has no
        // translation for — `C.UTF-8` in the rescue console, `en_US` for
        // whoever wants American formats — and neither is a mistake to
        // report. Only a FILE naming an unknown language is somebody's
        // choice not being honoured.
    }
    (
        ronoctua_i18n::DEFAULT.to_owned(),
        Source::Default,
        complaints,
    )
}

/// Is this one of the declared languages?
#[must_use]
pub fn exists(id: &str) -> bool {
    ronoctua_i18n::LANGUAGES.contains(&id)
}

/// The language this process should speak, reading the real sources.
#[must_use]
pub fn resolve() -> (String, Source) {
    let env: Vec<Option<String>> = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .map(|v| std::env::var(v).ok())
        .collect();
    let env: Vec<Option<&str>> = env.iter().map(|v| v.as_deref()).collect();
    let (id, source, complaints) = resolve_in(home().as_deref(), &env);
    for c in &complaints {
        // LOUD, because the symptom is "the interface went back to English"
        // and there would otherwise be nothing to look at.
        tracing::warn!(complaint = %c, "a language source was passed over");
    }
    (id, source)
}

/// The texts this process should draw, from [`resolve`].
#[must_use]
pub fn texts() -> ronoctua_i18n::Texts {
    let (id, _) = resolve();
    ronoctua_i18n::Texts::fresh(&id).unwrap_or_else(|| {
        // Unreachable through `resolve`, which only returns declared names —
        // and cheaper to say so than to make the caller handle it.
        tracing::error!(language = id, "a declared language did not load");
        ronoctua_i18n::Texts::from_env()
    })
}

/// Writes the choice, atomically.
///
/// # Errors
///
/// There is no configuration directory, or it cannot be written.
pub fn persist(id: &str) -> std::io::Result<()> {
    let dir = home().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "no configuration directory")
    })?;
    persist_in(&dir, id)
}

/// [`persist`], with its root as a parameter.
///
/// # Errors
///
/// The directory cannot be written.
pub fn persist_in(dir: &Path, id: &str) -> std::io::Result<()> {
    let text = serde_json::json!({ "language": id }).to_string();
    ronoctua_xdg::write_atomically(dir, FILE, text.as_bytes())
}

/// A watch on the file, ringing `on_stir` from the watcher's thread.
///
/// `None`, with a line in the log, when there is no configuration directory or
/// no inotify instance left — the door then adopts a change at its next start,
/// which is not silent.
pub fn watch<F>(on_stir: F) -> Option<ronoctua_files::notice::Notice>
where
    F: Fn() -> bool + Send + 'static,
{
    let dir = home()?;
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(error = %e, "no configuration directory to watch the language in");
        return None;
    }
    match ronoctua_files::notice::Notice::on_file(&dir, FILE, PACE, on_stir) {
        Ok(n) => Some(n),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "no language watcher; a change reaches this door at its next start"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-language-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn other() -> String {
        ronoctua_i18n::LANGUAGES
            .iter()
            .find(|l| **l != ronoctua_i18n::DEFAULT)
            .expect("this system declares more than one language")
            .to_string()
    }

    /// THE FILE WINS over the environment, which is the whole point: the
    /// environment is the machine's answer and takes a rebuild to change.
    #[test]
    fn the_persons_file_beats_the_machines_environment() {
        let dir = scratch("wins");
        let mine = other();
        persist_in(&dir, &mine).unwrap();
        let (got, source, complaints) =
            resolve_in(Some(&dir), &[Some("en_GB.UTF-8"), Some("en_GB.UTF-8")]);
        assert_eq!(got, mine);
        assert_eq!(source, Source::File);
        assert!(complaints.is_empty(), "{complaints:?}");
    }

    /// With no file, the environment answers — and it arrives as a LOCALE,
    /// which is not the same spelling as a language tag.
    #[test]
    fn the_environment_is_read_as_a_locale_and_not_as_a_tag() {
        let dir = scratch("env");
        let mine = other();
        let locale = format!("{}.UTF-8", mine.replace('-', "_"));
        let (got, source, _) = resolve_in(Some(&dir), &[None, Some(&locale)]);
        assert_eq!(got, mine, "`{locale}` should resolve to `{mine}`");
        assert_eq!(source, Source::Env);
    }

    /// A locale this system has no translation for is NOT a complaint.
    ///
    /// `C.UTF-8` is what the rescue console runs in and `en_US` is there for
    /// whoever wants American formats; reporting either as a problem would
    /// put a warning in the log of every machine that is working correctly.
    #[test]
    fn an_untranslated_locale_falls_through_quietly() {
        let dir = scratch("quiet");
        let (got, source, complaints) =
            resolve_in(Some(&dir), &[Some("C.UTF-8"), Some("en_US.UTF-8")]);
        assert_eq!(got, ronoctua_i18n::DEFAULT);
        assert_eq!(source, Source::Default);
        assert!(complaints.is_empty(), "{complaints:?}");
    }

    /// A FILE naming a language that does not exist IS a complaint: it is
    /// somebody's choice not being honoured, and the symptom on screen is the
    /// interface going back to English with nothing said.
    #[test]
    fn a_file_naming_a_language_that_is_gone_says_so() {
        let dir = scratch("unknown");
        persist_in(&dir, "xx-YY").unwrap();
        let (got, source, complaints) = resolve_in(Some(&dir), &[]);
        assert_eq!(got, ronoctua_i18n::DEFAULT);
        assert_eq!(source, Source::Default);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("xx-YY"), "{complaints:?}");
    }

    /// A file that will not parse is reported, and does not take the door down.
    #[test]
    fn a_file_that_does_not_parse_is_reported_and_passed_over() {
        let dir = scratch("broken");
        std::fs::write(dir.join(FILE), b"{ this is not json").unwrap();
        let (got, source, complaints) = resolve_in(Some(&dir), &[]);
        assert_eq!(got, ronoctua_i18n::DEFAULT);
        assert_eq!(source, Source::Default);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
    }

    /// What is written is what is read back, for every declared language.
    #[test]
    fn every_declared_language_survives_the_round_trip() {
        let dir = scratch("round");
        for id in ronoctua_i18n::LANGUAGES {
            persist_in(&dir, id).unwrap();
            let (got, source, _) = resolve_in(Some(&dir), &[]);
            assert_eq!(&got, id);
            assert_eq!(source, Source::File);
            // And it really loads: a name that round-trips and has no bundle
            // would be a screen full of `[key]`.
            assert!(ronoctua_i18n::Texts::fresh(&got).is_some(), "{id}");
        }
    }

    /// The pace is the theme's, and a test holds it: a listing's hold-off
    /// would put a quarter of a second between the arrow and the language.
    #[test]
    fn the_watch_does_not_wait_for_a_burst_to_end() {
        assert_eq!(PACE, ronoctua_files::notice::Pace::AtOnce);
    }
}
