//! Which theme this process draws in: where the answer comes from, how it is
//! kept, and how the other doors hear about a change.
//!
//! # Three sources, in order
//!
//! 1. `$XDG_CONFIG_HOME/ronoctua/theme.json` — what the person chose with
//!    `LEADER s t`. Theirs, hand-editable, and it wins.
//! 2. `RONOCTUA_THEME` — the machine's default, exported by the NixOS option
//!    `ronoctua.theme`. What a fresh machine shows.
//! 3. The first theme in `assets/themes/themes.nix`.
//!
//! A source that names a theme that does not exist, or a file that does not
//! parse, falls through to the next and says so at `warn` — loudly, because a
//! file that stopped parsing means the person's choice is silently back to the
//! default, and the symptom would be "my theme reset" with nothing in the log.
//!
//! # The file is the broadcast
//!
//! The footer, the file manager's window, the notification and update windows
//! and every buffer's host are separate processes, and the control socket only
//! goes INTO the footer. So the footer does not tell anybody: it writes the
//! file, atomically, and every door holds an inotify watch on it
//! ([`watch`]). The write is the message; a door that was not running picks
//! it up at its next start, from the same file. No second socket vocabulary,
//! which `leader.rs` has been deliberately removing.
//!
//! # The fragments
//!
//! Two consumers outside the shell follow the theme through files the
//! GENERATOR wrote (`nix/themes.nix`, one renderer): niri's border colours as
//! a `window-rule` its config includes, and Ghostty's palette. On a change the
//! footer COPIES the chosen theme's fragments into the configuration
//! directory — `$RONOCTUA_THEMES` says where the generator's output is. The
//! shell renders nothing itself: two renderers of one fragment is how they
//! diverge.
//!
//! # The desk
//!
//! The file carries a second answer beside the theme: what the compositor
//! paints where no window is — black, or the theme's own `surface.backdrop`
//! ([`Backdrop`]). The footer never paints the desk, so it is not a theme: it
//! is a choice ABOUT the theme, kept in the same file and carried by the same
//! fragment. The generator writes one niri fragment per answer, each with the
//! border and a `layout { background-color }`, and which one the footer copies
//! IS the answer. A file without the field is on black, which is what every
//! machine was on before the field existed.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ronoctua_tokens::theme::{self, Theme};

/// The file, under `$XDG_CONFIG_HOME/ronoctua`:
/// `{ "theme": "forest", "backdrop": "theme" }`. The second field is optional
/// and its absence means black.
pub const FILE: &str = "theme.json";

/// The variable naming the generator's output, where the fragments are.
pub const FRAGMENTS_ENV: &str = "RONOCTUA_THEMES";

/// The fragment that says which way round a theme is — `dark` or `light`.
/// Written by `nix/scripts/gen-theme.py`; read by [`dress_foreign_apps`].
pub const MODE_FRAGMENT: &str = "mode";

/// What the compositor paints where no window is — the option `LEADER s t`
/// offers in its second column. Two answers, by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backdrop {
    /// The pixel switched off.
    Black,
    /// The theme's own desk, `surface.backdrop`: the accent's hue one step
    /// below the footer's chrome, solved by the generator per theme.
    ///
    /// AND THE DEFAULT, at the owner's word. It was `Black` — a machine that
    /// never chose showed the pixel switched off under its windows, and a
    /// theme's own desk was something one had to go and ask for. Every theme
    /// solves that desk and every gate holds it to being distinct from the
    /// content's black, so making it what a fresh machine shows costs nothing
    /// and stops the work being invisible by default.
    #[default]
    Theme,
}

impl Backdrop {
    /// Every answer, in the order the chooser lists them.
    pub const ALL: [Self; 2] = [Self::Black, Self::Theme];

    /// The id, as the file spells it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Black => "black",
            Self::Theme => "theme",
        }
    }

    /// The answer an id names.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.id() == id)
    }

    /// The text key of its name on the chooser's row (invariant 8).
    #[must_use]
    pub const fn text_key(self) -> &'static str {
        match self {
            Self::Black => ronoctua_i18n::key::THEME_BACKDROP_BLACK,
            Self::Theme => ronoctua_i18n::key::THEME_BACKDROP_THEME,
        }
    }

    /// Where it sits in [`Self::ALL`].
    #[must_use]
    pub fn position(self) -> usize {
        Self::ALL.iter().position(|b| *b == self).unwrap_or(0)
    }

    /// The generator's fragment for the compositor with this desk under the
    /// windows: the theme's border as a `window-rule`, and the desk as a
    /// `layout { background-color }` that niri merges into its own.
    const fn fragment(self) -> &'static str {
        match self {
            Self::Black => "niri-black.kdl",
            Self::Theme => "niri-backdrop.kdl",
        }
    }
}

/// What the file says, whole: the theme and the desk under the windows. The
/// two are written together and the fragments follow the pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub theme: Theme,
    pub backdrop: Backdrop,
}

/// The fragments copied on a change: `(name in the theme's directory, name in
/// the configuration directory)`. niri includes the first with `optional=true`
/// and Ghostty the second with `?`, so a machine that never chose has neither
/// and is on the default — and [`sync_with_file`] keeps that true, removing
/// both when there is no file. The first depends on the desk: one generated
/// file per answer, and copying is choosing. A PREVIEW writes BOTH, because a
/// preview writes the file (see [`show_everywhere`]) — a terminal opened over
/// one wears the palette on screen, which is the point, and `Esc` puts both
/// back.
#[must_use]
pub const fn fragments(backdrop: Backdrop) -> [(&'static str, &'static str); 2] {
    [
        (backdrop.fragment(), "niri-theme.kdl"),
        ("ghostty.conf", "ghostty-theme.conf"),
    ]
}

/// The configuration directory, found ONCE per process in
/// [`resolve_at_start`] and kept here for every reader and writer below.
///
/// The rule is the repository's (`persistence-takes-its-root-as-a-parameter`):
/// a function that reads or writes does not decide where — the application
/// finds the directory once and hands it over, or `cargo test` writes into the
/// home of whoever runs it. The pure functions take the directory as an
/// argument; the process-facing ones take it from here.
static HOME: OnceLock<Option<PathBuf>> = OnceLock::new();

/// The directory, found on the first call and never again — so nothing
/// depends on WHO called first, and a door that somehow ran before
/// [`resolve_at_start`] would still find its file rather than fail quietly.
fn home() -> Option<&'static Path> {
    HOME.get_or_init(ronoctua_xdg::config_dir).as_deref()
}

/// Where the answer came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The person's file.
    File,
    /// The machine's default, from the environment.
    Env,
    /// The first declared theme.
    Default,
}

/// The theme the sources name, where it came from, and what was wrong with the
/// sources that were passed over.
///
/// PURE: the directory and the environment value are parameters, so a test
/// never reads the real home of whoever runs it — the rule every reader in
/// this system follows.
#[must_use]
pub fn resolve_in(dir: Option<&Path>, env: Option<&str>) -> (Theme, Source, Vec<String>) {
    let mut complaints = Vec::new();
    if let Some(dir) = dir {
        match read_choice(&dir.join(FILE)) {
            Ok(None) => {}
            Ok(Some(Written { theme: id, .. })) => match theme::by_id(&id) {
                Some(t) => return (t, Source::File, complaints),
                None => complaints.push(format!("{FILE} names an unknown theme `{id}`")),
            },
            Err(e) => complaints.push(format!("{FILE}: {e}")),
        }
    }
    if let Some(id) = env {
        match theme::by_id(id) {
            Some(t) => return (t, Source::Env, complaints),
            None => complaints.push(format!("{} names an unknown theme `{id}`", theme::ENV)),
        }
    }
    (theme::default_one(), Source::Default, complaints)
}

/// The file's two fields as it spells them, before either is looked up.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Written {
    theme: String,
    /// Absent in a file written before the desk existed, and that means black.
    backdrop: Option<String>,
}

/// `Ok(None)` when there is no file; `Err` when there is one and it does not
/// say a theme.
fn read_choice(path: &Path) -> Result<Option<Written>, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let theme = value
        .get("theme")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "no `theme` field".to_owned())?;
    let backdrop = value
        .get("backdrop")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    Ok(Some(Written {
        theme: theme.to_owned(),
        backdrop,
    }))
}

/// The desk the file asks for, and what was wrong with the answer when it
/// is not one of the two. No file, no field, or a file that does not parse
/// (which [`resolve_in`] already reports) all mean black: the desk is a
/// choice about the theme, and the theme's own fallbacks do the falling.
///
/// PURE, like [`resolve_in`]: the directory is a parameter.
#[must_use]
pub fn backdrop_in(dir: Option<&Path>) -> (Backdrop, Vec<String>) {
    let Some(dir) = dir else {
        return (Backdrop::default(), Vec::new());
    };
    match read_choice(&dir.join(FILE)) {
        Ok(Some(Written {
            backdrop: Some(id), ..
        })) => match Backdrop::by_id(&id) {
            Some(b) => (b, Vec::new()),
            None => (
                Backdrop::default(),
                vec![format!(
                    "{FILE} names an unknown backdrop `{id}`; the desk is black"
                )],
            ),
        },
        _ => (Backdrop::default(), Vec::new()),
    }
}

/// The desk the file says, for this process — the footer asks when its
/// chooser opens and when the file changes under it.
#[must_use]
pub fn committed_backdrop() -> Backdrop {
    let (b, complaints) = backdrop_in(home());
    for c in complaints {
        tracing::warn!(complaint = %c, "the desk in the file was passed over");
    }
    b
}

/// The three sources read once, and the theme set for the process. Called at
/// the top of `main`, before any door; every process of this binary goes
/// through it.
pub fn resolve_at_start() {
    let env = std::env::var(theme::ENV).ok();
    let (t, source, complaints) = resolve_in(home(), env.as_deref());
    for c in complaints {
        tracing::warn!(complaint = %c, "a theme source was passed over");
    }
    theme::set(t);
    // `debug`, not `info`: the short-lived subcommands (`leader`, `command`)
    // pass through here on every keypress, and a line per keypress in the
    // compositor's journal is noise; a CHANGE is logged at `info` by `reload`.
    tracing::debug!(theme = t.id(), ?source, "theme");
}

/// Reads the sources again and applies them. `true` if the theme changed.
///
/// What a door does when its watch fires — and what the footer's own write
/// makes it do too, harmlessly: the same index comes back and nothing repaints.
pub fn reload() -> bool {
    let env = std::env::var(theme::ENV).ok();
    let (t, _, complaints) = resolve_in(home(), env.as_deref());
    for c in complaints {
        tracing::warn!(complaint = %c, "a theme source was passed over");
    }
    let changed = theme::set(t);
    if changed {
        tracing::info!(theme = t.id(), "the theme changed under this process");
    }
    changed
}

/// Writes the choice, atomically, and refreshes the fragments beside it.
///
/// # Errors
///
/// There is no configuration directory, or it cannot be written.
pub fn persist(choice: Choice) -> std::io::Result<()> {
    let dir = home().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "no configuration directory")
    })?;
    persist_in(dir, fragments_root().as_deref(), choice)
}

/// [`persist`], with its two roots as parameters — see [`put_back_in`] for why
/// the pair of them exists.
fn persist_in(dir: &Path, root: Option<&Path>, choice: Choice) -> std::io::Result<()> {
    let text = serde_json::json!({ "theme": choice.theme.id(), "backdrop": choice.backdrop.id() })
        .to_string();
    ronoctua_xdg::write_atomically(dir, FILE, text.as_bytes())?;
    if let Some(root) = root {
        copy_fragments(root, choice, dir, &fragments(choice.backdrop));
        dress_foreign_apps(root, choice);
    }
    Ok(())
}

/// Shows a pair to EVERYTHING, which means WRITING THE FILE.
///
/// # Why a preview writes the file a choice writes
///
/// Because the file IS the broadcast. The footer changes its own colours in
/// memory; every other door — the six windows and the compositor — learns of
/// a theme by watching `theme.json` and reading it back. A preview that kept
/// off the file was a preview of one strip of the screen, which is what the
/// owner reported: the minibuffer changed and the windows, the desk and the
/// terminals did not.
///
/// A second channel was the alternative and was refused for the reason this
/// repository refuses it elsewhere: it is a second thing to keep in step, and
/// every reader would have to learn a precedence between the two.
///
/// # What it costs, and what pays it back
///
/// The file that means "chosen" is written while nobody has chosen yet. A
/// footer that dies mid-preview leaves the previewed pair persisted — the
/// same class of loose end the desk's own preview had, one file down. It is a
/// PREFERENCE, reversible with the same two keys that set it, and never data:
/// what is left behind is exactly what `RET` would have written.
///
/// `Esc` is what undoes it: see [`put_everything_back`], which needs to be
/// told whether there was a file before, because by then there always is one.
pub fn show_everywhere(choice: Choice) {
    if let Err(e) = persist(choice) {
        tracing::warn!(error = %e, "the preview could not be shown to the other doors");
    }
}

/// `Esc` after a preview: everything is shown what it was shown before.
///
/// `chosen_before` is the chooser's, taken when it opened — see
/// [`crate::theme_chooser::View::chosen_before`]. It cannot be worked out
/// here any more: the preview wrote the file, so the file always exists by
/// now, and a machine that never chose must not be given one by having looked
/// at the list.
pub fn put_everything_back(committed: Choice, chosen_before: bool) {
    let Some(dir) = home() else {
        return;
    };
    put_everything_back_in(dir, fragments_root().as_deref(), committed, chosen_before);
}

/// [`put_everything_back`], with its two roots as parameters.
fn put_everything_back_in(dir: &Path, root: Option<&Path>, committed: Choice, chosen_before: bool) {
    if chosen_before {
        if let Err(e) = persist_in(dir, root, committed) {
            tracing::warn!(error = %e, "the committed theme could not be put back");
        }
        return;
    }
    if let Err(e) = std::fs::remove_file(dir.join(FILE))
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(error = %e, "the preview's file could not be taken away again");
    }
    remove_fragments(dir);
}

/// Has a pair ever been written? Asked ONCE, when the chooser opens.
#[must_use]
pub fn chosen_before() -> bool {
    home().is_some_and(|dir| matches!(read_choice(&dir.join(FILE)), Ok(Some(_))))
}

/// [`put_compositor_back`], with its two roots as parameters.
fn put_back_in(dir: &Path, root: Option<&Path>, committed: Choice) {
    if matches!(read_choice(&dir.join(FILE)), Ok(Some(_))) {
        if let Some(root) = root {
            copy_fragments(root, committed, dir, &fragments(committed.backdrop));
            dress_foreign_apps(root, committed);
        }
    } else {
        remove_fragments(dir);
    }
}

/// Where the generator's output is, or `None` with the reason in the log.
fn fragments_root() -> Option<PathBuf> {
    let root = std::env::var_os(FRAGMENTS_ENV).map(PathBuf::from);
    if root.is_none() {
        tracing::warn!(
            variable = FRAGMENTS_ENV,
            "not set; the compositor and the terminal keep the default theme"
        );
    }
    root
}

/// Copies `which` of the pair's fragments from the generator's output `root`
/// into `dir`. The roots are parameters, so a test never touches the home of
/// whoever runs it.
fn copy_fragments(root: &Path, choice: Choice, dir: &Path, which: &[(&str, &str)]) {
    for (from, to) in which {
        let source = root.join(choice.theme.id()).join(from);
        match std::fs::read(&source) {
            Ok(bytes) => {
                // UNCHANGED BYTES ARE NOT WRITTEN: the footer's own write rings
                // its own watch, and a second copy would be a second reload of
                // the compositor for nothing.
                if std::fs::read(dir.join(to)).is_ok_and(|had| had == bytes) {
                    continue;
                }
                if let Err(e) = ronoctua_xdg::write_atomically(dir, to, &bytes) {
                    tracing::warn!(fragment = to, error = %e, "could not write the fragment");
                }
            }
            Err(e) => {
                tracing::warn!(fragment = %source.display(), error = %e, "could not read the fragment");
            }
        }
    }
}

/// THE SEVENTH FRAGMENT, and it is not a colour.
///
/// A palette cannot be asked which side of the ladder it is on — every gate in
/// the generator measures a MAGNITUDE — so the generator writes the answer
/// down, one word, beside the six colour fragments. This is what reads it.
///
/// It is not copied into the configuration directory like the other six: niri
/// and Ghostty learn a theme by being given a file, and a toolkit learns one
/// through dconf. Nothing would ever read the copy.
fn dress_foreign_apps(root: &Path, choice: Choice) {
    let source = root.join(choice.theme.id()).join(MODE_FRAGMENT);
    match std::fs::read_to_string(&source) {
        Ok(word) => match word.trim() {
            "dark" => crate::toolkits::wear(true),
            "light" => crate::toolkits::wear(false),
            other => tracing::warn!(
                mode = other,
                "the theme's mode fragment says something this shell does not know"
            ),
        },
        // A shell built before the generator wrote this, or a store path that
        // is not there: the toolkits keep what the build gave them, which is
        // the mode of the theme the machine was declared with.
        Err(e) => {
            tracing::debug!(fragment = %source.display(), error = %e, "no mode fragment; the toolkits keep what they had")
        }
    }
}

/// Removes every fragment from `dir`: what "never chose" looks like on disk.
/// A fragment that is not there is not an error.
fn remove_fragments(dir: &Path) {
    for (_, to) in fragments(Backdrop::default()) {
        match std::fs::remove_file(dir.join(to)) {
            Ok(()) => tracing::info!(fragment = to, "removed: the file says no theme"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(fragment = to, error = %e, "could not remove the fragment"),
        }
    }
}

/// The fragments are made to agree with the file — at the footer's start,
/// because a rebuild may have moved the generator's output, and whenever the
/// file changes under it from outside, because the fragments must never say
/// a theme the file does not. No file, no fragments: a footer that died in
/// the middle of a preview, or a file the person deleted by hand, leaves
/// nothing behind that the compositor would keep obeying.
pub fn sync_with_file() {
    let Some(dir) = home() else {
        return;
    };
    let committed = Choice {
        theme: theme::current(),
        backdrop: committed_backdrop(),
    };
    put_back_in(dir, fragments_root().as_deref(), committed);
}

/// How fast the theme's watch answers, and it is the ONE watch in the shell
/// that does not coalesce.
///
/// The chooser previews by WRITING this file on every keystroke, and the
/// hold-off that serves a listing puts a quarter of a second between the arrow
/// and the colour — measured, 5 ms for the first change and 253 for each one
/// behind it. A listing absorbs a burst because its answer is a `readdir` per
/// event; this one's answer is a repaint, and the burst is a person's hand.
///
/// Named, rather than written into the call, so a test can hold it: the
/// difference the pace makes is measured where the watcher lives, and nothing
/// else would notice this line going back to the other value.
const PACE: ronoctua_files::notice::Pace = ronoctua_files::notice::Pace::AtOnce;

/// A watch on the file, ringing `on_stir` from the watcher's thread when it
/// changes. `None`, with a line in the log, when there is no configuration
/// directory or no inotify instance left — the door then adopts a change at
/// its next start, which is not silent and not the reported bug coming back.
pub fn watch<F>(on_stir: F) -> Option<ronoctua_files::notice::Notice>
where
    F: Fn() -> bool + Send + 'static,
{
    let dir = home()?;
    // The directory has to exist for a watch to sit on it, and a door that
    // starts before anyone ever chose is the common case.
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(error = %e, "no configuration directory to watch the theme in");
        return None;
    }
    match ronoctua_files::notice::Notice::on_file(dir, FILE, PACE, on_stir) {
        Ok(n) => Some(n),
        Err(e) => {
            tracing::warn!(error = %e, "no theme watcher; a change reaches this door at its next start");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-theme-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn second() -> Option<Theme> {
        theme::all().nth(1)
    }

    #[test]
    fn the_file_beats_the_environment_which_beats_the_default() {
        let Some(other) = second() else {
            return; // one theme declared: nothing to prefer
        };
        let dir = scratch("precedence");
        let (t, source, complaints) = resolve_in(Some(&dir), None);
        assert_eq!((t, source), (theme::default_one(), Source::Default));
        assert!(complaints.is_empty());

        let (t, source, _) = resolve_in(Some(&dir), Some(other.id()));
        assert_eq!((t, source), (other, Source::Env));

        std::fs::write(
            dir.join(FILE),
            format!("{{\"theme\":\"{}\"}}", theme::default_one().id()),
        )
        .unwrap();
        let (t, source, complaints) = resolve_in(Some(&dir), Some(other.id()));
        assert_eq!((t, source), (theme::default_one(), Source::File));
        assert!(complaints.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bad_source_is_passed_over_and_named() {
        let dir = scratch("complaints");
        std::fs::write(dir.join(FILE), "{\"theme\":\"no-such-theme\"}").unwrap();
        let (t, source, complaints) = resolve_in(Some(&dir), Some("also-unknown"));
        assert_eq!((t, source), (theme::default_one(), Source::Default));
        assert_eq!(complaints.len(), 2, "{complaints:?}");
        assert!(complaints[0].contains("no-such-theme"));
        assert!(complaints[1].contains("also-unknown"));

        std::fs::write(dir.join(FILE), "not json").unwrap();
        let (_, source, complaints) = resolve_in(Some(&dir), None);
        assert_eq!(source, Source::Default);
        assert_eq!(complaints.len(), 1);

        std::fs::write(dir.join(FILE), "{\"colour\":\"owl\"}").unwrap();
        let (_, _, complaints) = resolve_in(Some(&dir), None);
        assert!(complaints[0].contains("no `theme` field"), "{complaints:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A generator's output with the fragments of the default theme, faked.
    fn fake_root(tag: &str) -> PathBuf {
        let root = scratch(tag).join("themed");
        let dir = root.join(theme::default_one().id());
        std::fs::create_dir_all(&dir).unwrap();
        for b in Backdrop::ALL {
            std::fs::write(dir.join(b.fragment()), format!("desk {}\n", b.id())).unwrap();
        }
        std::fs::write(dir.join("ghostty.conf"), "palette\n").unwrap();
        root
    }

    #[test]
    fn the_themes_watch_does_not_coalesce() {
        // Every other door repaints from this watch, and the chooser writes
        // the file once per keystroke. `notice::Pace` carries the measurement;
        // this is the line that would silently undo it.
        assert_eq!(PACE, ronoctua_files::notice::Pace::AtOnce);
    }

    #[test]
    fn a_preview_reaches_every_door_and_leaves_nothing_behind_without_a_file() {
        // IT USED TO WRITE THE COMPOSITOR'S FRAGMENT AND NOTHING ELSE, and
        // that is what the owner reported: the minibuffer changed colour, the
        // windows did not, and what they were shown was a theme worn by one
        // strip of the screen. The file is the channel every other door
        // watches, so a preview writes it — and `Esc` is what makes that safe.
        let dir = scratch("preview");
        let root = fake_root("preview-root");
        let committed = Choice {
            theme: theme::default_one(),
            backdrop: Backdrop::Black,
        };
        let tried = Choice {
            backdrop: Backdrop::Theme,
            ..committed
        };

        // A MACHINE THAT NEVER CHOSE. The preview writes the file and both
        // fragments — everything the doors read.
        persist_in(&dir, Some(&root), tried).unwrap();
        assert!(dir.join(FILE).exists(), "the other doors are told nothing");
        assert_eq!(
            std::fs::read_to_string(dir.join("niri-theme.kdl")).unwrap(),
            "desk theme\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("ghostty-theme.conf")).unwrap(),
            "palette\n"
        );
        // `Esc`: EVERYTHING goes away again. Looking at a list must not be
        // what gives a machine a choice it never made — and by now the file
        // exists, so only the chooser can still say it did not.
        put_everything_back_in(&dir, Some(&root), committed, false);
        assert!(
            !dir.join(FILE).exists(),
            "a machine that never chose was given a file by previewing"
        );
        assert!(!dir.join("niri-theme.kdl").exists());
        assert!(!dir.join("ghostty-theme.conf").exists());

        // A MACHINE THAT CHOSE. `Esc` puts back what it had, in all three.
        persist_in(&dir, Some(&root), committed).unwrap();
        persist_in(&dir, Some(&root), tried).unwrap();
        put_everything_back_in(&dir, Some(&root), committed, true);
        assert!(dir.join(FILE).exists());
        assert_eq!(
            read_choice(&dir.join(FILE)).unwrap().unwrap().backdrop,
            Some(Backdrop::Black.id().to_owned()),
            "the file kept the previewed desk"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("niri-theme.kdl")).unwrap(),
            "desk black\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("ghostty-theme.conf")).unwrap(),
            "palette\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn the_desk_is_the_theme_unless_the_file_says_black() {
        let dir = scratch("desk");
        let owl = theme::default_one().id();
        // THE DEFAULT IS THE THEME'S OWN DESK NOW, at the owner's word, and
        // this test read the other way round. No file, a file from before the
        // field, an unknown answer: all three fall to the default, and only
        // the unknown one is a complaint — the other two are what a machine
        // that never chose looks like.
        assert_eq!(backdrop_in(Some(&dir)), (Backdrop::Theme, Vec::new()));
        std::fs::write(dir.join(FILE), format!("{{\"theme\":\"{owl}\"}}")).unwrap();
        assert_eq!(backdrop_in(Some(&dir)), (Backdrop::Theme, Vec::new()));
        std::fs::write(
            dir.join(FILE),
            format!("{{\"theme\":\"{owl}\",\"backdrop\":\"purple\"}}"),
        )
        .unwrap();
        let (b, complaints) = backdrop_in(Some(&dir));
        assert_eq!(b, Backdrop::Theme);
        assert!(complaints[0].contains("purple"), "{complaints:?}");
        std::fs::write(
            dir.join(FILE),
            format!("{{\"theme\":\"{owl}\",\"backdrop\":\"theme\"}}"),
        )
        .unwrap();
        assert_eq!(backdrop_in(Some(&dir)), (Backdrop::Theme, Vec::new()));
        // AND THE OTHER ANSWER IS STILL REACHABLE, which is the half of this
        // that the default's change does not cover: `black` in the file has
        // to mean black, or the second column would offer a choice that does
        // nothing.
        std::fs::write(
            dir.join(FILE),
            format!("{{\"theme\":\"{owl}\",\"backdrop\":\"black\"}}"),
        )
        .unwrap();
        assert_eq!(backdrop_in(Some(&dir)), (Backdrop::Black, Vec::new()));
        // And the default without a directory.
        assert_eq!(backdrop_in(None).0, Backdrop::Theme);
        for b in Backdrop::ALL {
            assert_eq!(Backdrop::by_id(b.id()), Some(b));
            assert_eq!(Backdrop::ALL[b.position()], b);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_is_written_is_what_is_read() {
        let dir = scratch("round-trip");
        let t = second().unwrap_or_else(theme::default_one);
        let text =
            serde_json::json!({ "theme": t.id(), "backdrop": Backdrop::Theme.id() }).to_string();
        ronoctua_xdg::write_atomically(&dir, FILE, text.as_bytes()).unwrap();
        assert_eq!(
            read_choice(&dir.join(FILE)),
            Ok(Some(Written {
                theme: t.id().to_owned(),
                backdrop: Some("theme".to_owned())
            }))
        );
        assert_eq!(resolve_in(Some(&dir), None).0, t);
        assert_eq!(backdrop_in(Some(&dir)).0, Backdrop::Theme);
        assert!(!dir.join(format!("{FILE}.new")).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
