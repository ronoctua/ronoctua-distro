//! What foreign applications are told about the size of text.
//!
//! The shell's own text moves with `ronoctua_tokens::text::size` — one atomic,
//! and every place that puts a pixel down asks it. The `one-text-size` check
//! counts them; a number written here would be a number nobody updates. Firefox, Emacs and Ghostty are not ours, and
//! the only thing every one of them agrees to listen to is the SETTINGS
//! PORTAL, which answers out of dconf.
//!
//! # One place, and why it is dconf and not a file
//!
//! `nix/modules/foreign.nix` names three: `/etc/xdg/gtk-{3,4}.0/settings.ini`,
//! the dconf user profile, and Qt's platform theme. The first and the third
//! are built by Nix, in the store, read-only — they are the machine's floor,
//! not the person's choice, and nothing here can or should rewrite them.
//!
//! What is left is dconf, and it is the right one anyway: it is the layer the
//! portal answers from, and `programs.dconf.enable` and
//! `xdg-desktop-portal-gtk` are both already on for other reasons. A GTK 4
//! application, Chromium and Electron pick up `text-scaling-factor` from the
//! portal WHILE THEY ARE RUNNING; nothing has to be restarted and nothing has
//! to be logged out of.
//!
//! # What was MEASURED, and what is still argued
//!
//! Two of the sentences that used to stand here were wrong, and both were
//! wrong in the direction of comfort — they excused a gap that was not there
//! and hid one that was.
//!
//! - **Qt does not follow a multiplier.** `qgnomeplatform` 0.8.4, both majors,
//!   subscribes to `changed::cursor-blink-time`, `changed::cursor-size`,
//!   `changed::font-name`, `changed::gtk-theme`, `changed::icon-theme`,
//!   `changed::monospace-font-name` and `changed::titlebar-font` — read out of
//!   the shipped libraries. `text-scaling-factor` is not among them. The old
//!   text said Qt "follows under the `full` profile"; what the profile buys is
//!   darkness and a font NAME.
//! - **Ghostty does follow.** Its 1.3.1 binary carries `gtk-xft-dpi`, and its
//!   manual says under `font-size`: *"On Linux with GTK, font size is scaled
//!   according to both display-wide and text-specific scaling factors."* The
//!   old text said it "follows nothing".
//!
//! Those two facts together decided the channel. GTK renders at
//! `points x (gtk-xft-dpi/96)` and `gtk-xft-dpi` comes from the multiplier, so
//! writing the multiplier AND the font scales GTK twice; and there is no
//! channel that reaches Qt without reaching GTK. So the size travels in the
//! font description, the multiplier is put back, and Ghostty — which reads its
//! own `font-size` — is written a file of its own and sent `SIGUSR2`.
//!
//! **What is measured end to end**: that the size reaches Ghostty.
//! `ghostty-config-includes-theme` lays the file down and asks the binary for
//! its EFFECTIVE configuration, which answered 11 -> 22 at the ceiling of the
//! range. That is the only foreign application this repository can question
//! without a session.
//!
//! **What is still argued from the binaries, not from a screen**: that GTK and
//! Qt follow `font-name`. Both read it — the strings say so — but no test here
//! opens one. `docs/PENDING.md` carries that, named, and the row in the display
//! view says only what was measured.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// The directory `display.json` lives in, found ONCE.
///
/// A FUNCTION THAT READS OR WRITES DOES NOT DECIDE WHERE — the rule
/// `persistence-takes-its-root-as-a-parameter` holds, and it caught this file
/// the first time it was written. Otherwise a test writes into the home of
/// whoever runs it, which is what happened once with the file manager's
/// sidebar. It is `theme.rs`'s arrangement, for the same reason.
static HOME: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Where the size of the text is written down.
fn home() -> Option<&'static Path> {
    HOME.get_or_init(ronoctua_store::config_dir).as_deref()
}

/// The three keys a foreign toolkit reads a font from.
///
/// # Why the FONT and not the multiplier
///
/// There is a multiplier — `text-scaling-factor` — and for a long time it was
/// the only thing this module wrote. It reaches GTK 3, GTK 4 and, through
/// GTK's `gtk-xft-dpi`, Ghostty. What it does not reach is **Qt**, and that
/// was measured rather than assumed: `qgnomeplatform` 0.8.4, both majors,
/// subscribes to exactly
///
/// ```text
/// changed::cursor-blink-time  changed::cursor-size   changed::font-name
/// changed::gtk-theme          changed::icon-theme    changed::monospace-font-name
/// changed::titlebar-font
/// ```
///
/// on `org.gnome.desktop.interface`. `text-scaling-factor` is not in that
/// list. So the 591 MiB the `full` profile pays for a Qt platform theme buys
/// darkness and a font NAME — never a scale.
///
/// And the two channels cannot both be used: GTK renders at
/// `points x (gtk-xft-dpi/96)`, so scaling the font AND the multiplier scales
/// GTK twice. There is no channel that reaches Qt and misses GTK. Therefore
/// the size travels in the font description, the multiplier is put back to
/// where it was found, and the one consumer that reads neither — Ghostty,
/// which has a `font-size` of its own — is given a file (see [`SIZE_FILE`]).
pub const FONT_KEYS: [&str; 3] = [
    "/org/gnome/desktop/interface/font-name",
    "/org/gnome/desktop/interface/monospace-font-name",
    "/org/gnome/desktop/interface/document-font-name",
];

/// The multiplier, which this module now RESETS instead of writing.
///
/// It is not merely unused: a value left in it from an older version of this
/// system would multiply the size that now travels in the font description,
/// and the person would get the square of what they asked for.
pub const SCALING_KEY: &str = "/org/gnome/desktop/interface/text-scaling-factor";

/// Ghostty's own file, and why it is not the theme's.
///
/// Ghostty reads a `font-size` of its own (`nix/modules/terminal.nix`) and its
/// manual says GTK's text scaling reaches it — but the multiplier is no longer
/// what carries the size, so it has to be told directly. `terminal.nix`
/// includes this path LAST, and in Ghostty the later file wins.
///
/// NOT `ghostty-theme.conf`: that one is copied WHOLE out of the store by
/// `theme::copy_fragments`, so a size written into it would be erased by the
/// next change of theme. One writer per file.
pub const SIZE_FILE: &str = "ghostty-size.conf";

/// Where the person's own font descriptions are kept, as they were.
pub const UNTOUCHED_FILE: &str = "toolkits.json";

/// The two keys a foreign toolkit reads its POLARITY from.
///
/// `nix/modules/foreign.nix` writes the same pair into the dconf profile for
/// the theme the machine was BUILT with; this is the other half, for the theme
/// the person is wearing now. The two agree on the first boot and this one
/// owns every change after it — `/etc/xdg` is in the store and read-only, and
/// dconf is the only writable layer a GTK or a Qt application reads.
///
/// `color-scheme` is what a portal-aware application asks for (GTK 4,
/// Chromium, Electron) and `gtk-theme` is what GTK 3 and `qgnomeplatform`
/// watch — the second is in the subscription list quoted above [`FONT_KEYS`],
/// which is how we know a Qt application hears it.
pub const MODE_KEYS: [&str; 2] = [
    "/org/gnome/desktop/interface/color-scheme",
    "/org/gnome/desktop/interface/gtk-theme",
];

/// What was last written to [`MODE_KEYS`], so a preview does not fork `dconf`
/// per keystroke.
///
/// The theme chooser writes the file on every cursor move — that is what makes
/// the preview reach the other doors — and every one of those calls here. Two
/// `dconf` children per arrow key is a frame's worth of work for a value that
/// did not change. `copy_fragments` keeps the same rule for the same reason.
static WEARING: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);

/// Dress the foreign applications for this side of the ladder.
///
/// Stock themes and not ours: nothing in this system carries a token into a
/// foreign window (invariant 10's arrangement, one wall over), so what is said
/// is only which way round to be. `nix/modules/foreign.nix` picks the same two
/// names, and the day one of them changes both have to.
pub fn wear(dark: bool) {
    {
        let Ok(mut worn) = WEARING.lock() else {
            return;
        };
        if *worn == Some(dark) {
            return;
        }
        *worn = Some(dark);
    }
    let values = if dark {
        ["'prefer-dark'", "'Adwaita-dark'"]
    } else {
        ["'prefer-light'", "'Adwaita'"]
    };
    // OFF THE FRAME'S THREAD, for `tell_them`'s reason: two children, each
    // waited for, is two ways to stall a repaint.
    let started = std::thread::Builder::new()
        .name("toolkits-mode".to_owned())
        .spawn(move || {
            for (key, value) in MODE_KEYS.iter().zip(values) {
                if dconf(&["write", key, value]).is_none() {
                    tracing::debug!(
                        key,
                        "dconf would not take the mode; this machine does not do dconf"
                    );
                }
            }
        });
    if let Err(e) = started {
        tracing::warn!(error = %e, "no thread to dress the toolkits with");
    }
}

/// `dconf`, by path and not by name.
///
/// A user unit's `PATH` is minimal, and a bare name fails far from the cause
/// — the trap `AGENTS.md` records. Outside Nix it falls back to the name.
const DCONF: &str = match option_env!("RONOCTUA_DCONF") {
    Some(c) => c,
    None => "dconf",
};

/// One font description split into the family and the size in points.
///
/// A GNOME font description is `"<family and style> <points>"` — `Cantarell 11`,
/// `Noto Sans CJK JP 11`, `Cantarell Bold 12.5`. The family may hold spaces and
/// words; the size is the LAST whitespace-separated token and is a number.
///
/// `None` when the last token is not a number, which is what an empty value or
/// a hand-written oddity looks like. The caller then leaves that key alone —
/// rewriting something we could not read is how a person loses their font.
#[must_use]
pub fn split_font(description: &str) -> Option<(&str, f64)> {
    let trimmed = description.trim();
    let at = trimmed.rfind(char::is_whitespace)?;
    let points: f64 = trimmed[at + 1..].parse().ok()?;
    // A SIZE OF ZERO OR LESS IS NOT A SIZE, and scaling it would answer zero
    // for every percentage — a foreign application with no text at all.
    if !(points.is_finite() && points > 0.0) {
        return None;
    }
    Some((trimmed[..at].trim_end(), points))
}

/// A font description at a percentage of its own size.
///
/// ALWAYS FROM THE UNTOUCHED DESCRIPTION, never from what is in dconf now:
/// this is applied again on every change, and scaling an already-scaled value
/// compounds. 125% twice is 156%, and nothing on the screen would say why.
/// [`Untouched`] is what keeps the starting point.
///
/// One decimal, because a font description takes a fraction and dropping it
/// makes the steps of the ladder uneven: 11 pt at 105% is 11,55 and at 110% is
/// 12,1 — rounded to whole points both would be 12, and one press of the key
/// would do nothing.
#[must_use]
pub fn font_at(description: &str, percent: u16) -> Option<String> {
    let (family, points) = split_font(description)?;
    let wanted = points * f64::from(percent) / 100.0;
    Some(format!("{family} {wanted:.1}"))
}

/// The font descriptions as they were before this system first touched them.
///
/// # Why this is written down instead of computed
///
/// Because the operation is "the person's own font, at a percentage of its own
/// size", and it is applied again every time the number moves. Reading dconf
/// back would read what WE wrote last time, and each change would multiply the
/// one before it. Keeping the starting point makes every write idempotent and
/// makes 100% put back exactly what was found — which is what lets somebody
/// turn this feature off and get their desktop back.
///
/// Its own small file, hand-editable, in the shape every other domain uses:
/// the file IS the record, and there is no second channel to keep in step.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Untouched {
    /// `font-name`, `monospace-font-name` and `document-font-name`, in the
    /// order of [`FONT_KEYS`]. An entry is absent when that key was empty or
    /// unreadable, and then this system leaves it alone for ever.
    #[serde(default)]
    pub fonts: std::collections::BTreeMap<String, String>,
}

/// The store the untouched descriptions live in.
#[must_use]
pub fn untouched() -> ronoctua_store::Store<Untouched> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), UNTOUCHED_FILE)
}

/// The chosen size, read once and set for the process.
///
/// Called at the top of `main`, beside the theme's, and for the same reason:
/// every process of this binary — the footer, its windows, a buffer's host —
/// draws through one number, and reading it once is cheaper than threading
/// it. The short-lived subcommands pay one file read.
///
/// IT DOES NOT DRESS THE FOREIGN TOOLKITS. `ronoctua-shell leader` runs on
/// every press of the leader key, and spawning a `dconf write` per keypress
/// is not a courtesy. That is [`tell_them`]'s job and the footer's alone.
pub fn resolve_at_start() {
    let chosen = ronoctua_display::prefs::store().read().text;
    let held = ronoctua_tokens::text::allowed(chosen);
    ronoctua_tokens::text::set(held);
    // `debug` and not `info`, the note `theme::resolve_at_start` carries: a
    // line per keypress in the compositor's journal is noise.
    tracing::debug!(percent = held, "text size");
}

/// What a door with NO watch on the size's file says of itself ([`watch`]
/// gave nothing) to a view it hosts: a door that cannot see the size change
/// must not step it — the step would be computed from a size the file has
/// left behind (the screens' view, `View::blind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unwatched;

/// A watch on the file the size lives in.
///
/// EVERY DOOR INSTALLS ONE, exactly as every door installs the theme's. The
/// first version of this put a watch only in the footer, and the result was
/// worse than no watch at all: the footer moved and the file manager, the
/// notification window, the terminal and every buffer host stayed at the old
/// size — which is the "two doors side by side, ten percent apart"
/// photograph that made this system have ONE size in the first place.
///
/// `None`, with a line in the log, when there is no configuration directory
/// or no inotify instance left: the door then adopts a change at its next
/// start, which is late but not silent.
pub fn watch<F>(on_stir: F) -> Option<ronoctua_files::notice::Notice>
where
    F: Fn() -> bool + Send + 'static,
{
    let dir = home()?;
    // The directory has to exist for a watch to sit on it, and a door that
    // starts before anybody ever chose is the common case.
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(error = %e, "no configuration directory to watch the text size in");
        return None;
    }
    match ronoctua_files::notice::Notice::on_file(
        dir,
        ronoctua_display::prefs::FILE,
        ronoctua_files::notice::Pace::Coalesced,
        on_stir,
    ) {
        Ok(n) => Some(n),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "no text size watcher; a change reaches this door at its next start"
            );
            None
        }
    }
}

/// Reads the file again and applies it. `true` if the size changed.
///
/// What a door does when its watch fires. It does NOT tell the foreign
/// toolkits — that is the footer's alone, and doing it from every door would
/// spawn one `dconf` per open window for one change.
pub fn reload() -> bool {
    ronoctua_tokens::text::set(ronoctua_display::prefs::store().read().text)
}

/// Tells the foreign toolkits how big the text is now.
///
/// It does not wait for the answer and does not fail loudly: dressing another
/// application is a courtesy, and a machine without dconf is a machine where
/// the shell's own text still moved. What it must not do is block the frame,
/// which is why the whole of it happens on a thread — the arrangement
/// `power_window`'s forwarding uses, for the same reason it gives there.
pub fn tell_them(percent: u16) {
    // ALL OF IT ON ONE THREAD, and not one thread per command. Reading a key
    // back means waiting for a child's output, and there are three keys, a
    // reset, a file and a signal. Done here they would be six ways to stall a
    // frame; done there they are one thread that nobody waits for.
    let started = std::thread::Builder::new()
        .name("toolkits".to_owned())
        .spawn(move || dress_them(percent));
    if let Err(e) = started {
        // A machine that cannot spawn a thread has worse trouble than one slow
        // frame — the judgement the old code made here for the same reason.
        tracing::warn!(error = %e, "no thread to dress the toolkits with; doing it here");
        dress_them(percent);
    }
}

/// One `dconf` invocation, waited for, with its output.
///
/// `None` when the binary is not there or answered badly. Every caller treats
/// that as "this machine does not do dconf", which is a machine where the
/// shell's own text still moved.
fn dconf(args: &[&str]) -> Option<String> {
    let out = Command::new(DCONF)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// A GVariant string literal, as `dconf write` wants it.
///
/// Single quotes, with any of its own doubled — a family called `Bob's Sans`
/// is not a family anybody has, but a value that ends a quote early is a
/// command that means something else.
#[must_use]
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "\\'"))
}

/// The text inside a GVariant string literal, as `dconf read` answers it.
///
/// `None` for an empty answer, which is what an unset key gives, and for
/// anything not wrapped in quotes — both mean "there is nothing here to keep".
#[must_use]
fn unquoted(said: &str) -> Option<String> {
    let text = said.trim();
    let inner = text.strip_prefix('\'')?.strip_suffix('\'')?;
    if inner.is_empty() {
        return None;
    }
    Some(inner.replace("\\'", "'"))
}

/// Everything a change of size owes the applications that are not ours.
///
/// Runs on its own thread. Every step is independently skippable: a machine
/// with no dconf still gets the Ghostty file, and a machine with no Ghostty
/// still gets its GTK and Qt applications dressed.
fn dress_them(percent: u16) {
    remember_and_dress_the_fonts(percent);
    // AND THE MULTIPLIER GOES BACK TO WHERE IT WAS FOUND.
    //
    // It is not merely unused now that the size travels in the font: a value
    // left in it by an older version of this system would multiply the size
    // that now travels in the font description, and the person would get the
    // SQUARE of what they asked for — 125% twice is 156%, with nothing on the
    // screen saying why.
    if dconf(&["reset", SCALING_KEY]).is_none() {
        tracing::debug!("dconf would not reset the scaling factor");
    }
    tell_the_terminal(percent);
}

/// The three font keys, at a percentage of the size they had before this
/// system ever touched them.
fn remember_and_dress_the_fonts(percent: u16) {
    let store = untouched();
    let mut base = store.read();
    let mut learned = false;

    for key in FONT_KEYS {
        if base.fonts.contains_key(key) {
            continue;
        }
        // THE STARTING POINT IS TAKEN ONCE, and only from a value we can
        // read. A key that is unset, empty, or does not end in a number is
        // one this system leaves alone FOR EVER: rewriting a description we
        // could not parse is how somebody loses their font.
        let Some(found) = dconf(&["read", key]).as_deref().and_then(unquoted) else {
            continue;
        };
        if split_font(&found).is_none() {
            tracing::debug!(key, value = %found, "a font description this cannot read; leaving it");
            continue;
        }
        base.fonts.insert((*key).to_owned(), found);
        learned = true;
    }

    // WRITTEN BEFORE ANYTHING IS CHANGED. If the write fails, nothing has been
    // touched yet and the next attempt starts from the same untouched values —
    // whereas dressing first and failing to record would make the next change
    // scale what this one already scaled.
    if learned && let Err(e) = store.write(&base) {
        tracing::warn!(error = %e, "could not write down the fonts as they were; leaving them alone");
        return;
    }

    for key in FONT_KEYS {
        let Some(had) = base.fonts.get(key) else {
            continue;
        };
        let Some(want) = font_at(had, percent) else {
            continue;
        };
        if dconf(&["write", key, &quoted(&want)]).is_none() {
            tracing::debug!(key, "dconf would not take the font");
        }
    }
}

/// Ghostty's own size, and a nudge for the ones already running.
fn tell_the_terminal(percent: u16) {
    let Some(dir) = home() else {
        return;
    };
    // THE SAME ARITHMETIC AS THE FONTS, on the size `nix/modules/terminal.nix`
    // wrote — both read `font.size.terminalPoints`, so they cannot drift.
    let points = f64::from(ronoctua_tokens::font::TERMINAL_POINTS) * f64::from(percent) / 100.0;
    let body = format!(
        "# GENERATED by ronoctua-shell. Do not edit — `LEADER s d` rewrites it.\n\
         font-size = {points:.1}\n"
    );
    // ONLY WHEN THE BYTES MOVE. A write here is a rename, and a rename is what
    // a config watcher sees — the rule `theme::copy_fragments` already keeps.
    if std::fs::read(dir.join(SIZE_FILE)).is_ok_and(|had| had == body.as_bytes()) {
        return;
    }
    if let Err(e) = ronoctua_xdg::write_atomically(dir, SIZE_FILE, body.as_bytes()) {
        tracing::warn!(error = %e, "could not write the terminal's size");
        return;
    }
    wake_the_terminals();
}

/// Makes every running Ghostty read its configuration again.
///
/// # Why a signal, and why this one
///
/// Ghostty has no automatic reload — its own manual says so in as many words
/// ("Ghostty isn't capable of this yet"), so a new file reaches only the next
/// window unless something asks. There is a documented `reload_config` keybind
/// action, but this system binds no key of Ghostty's own, and the D-Bus route
/// is closed here on purpose: `nix/modules/terminal.nix` sets
/// `gtk-single-instance = false` and strips the D-Bus service, so there is no
/// well-known name to send an action to.
///
/// What is left is `SIGUSR2`. It is NOT in the manual — it was read out of the
/// 1.3.1 binary, which carries the string
/// `received SIGUSR2, reloading configuration` under Ghostty's own
/// `gtk_ghostty_application` log scope. `SIGUSR1` appears nowhere. So this is
/// observed behaviour of the version this system pins, and `ghostty-reloads-on-a-signal`
/// re-reads the binary on every build: the day the string goes, the check goes
/// red instead of the terminal quietly keeping yesterday's size.
///
/// A miss is not a failure. The signal is best effort: what it saves is
/// reopening the window.
fn wake_the_terminals() {
    // BY NAME AND ONLY OUR OWN. `pkill` matches the caller's own command line
    // — the trap `AGENTS.md` records — so the pattern is bracketed, and `-U`
    // keeps it to this person's processes on a machine that has others.
    let said = Command::new("pkill")
        .args([
            "-USR2",
            "-U",
            &format!("{}", rustix::process::getuid().as_raw()),
            "-x",
            "ghostty",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match said {
        // 1 is `pkill`'s "nothing matched", which is a machine with no
        // terminal open — the ordinary case, and not worth a line.
        Ok(code) if code.success() || code.code() == Some(1) => {}
        Ok(code) => tracing::debug!(?code, "pkill would not signal the terminals"),
        Err(e) => {
            tracing::debug!(error = %e, "no pkill; open terminals keep their size until reopened")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_font_description_splits_into_a_family_and_a_size() {
        // The family may hold spaces and words; the size is the last token.
        assert_eq!(split_font("Cantarell 11"), Some(("Cantarell", 11.0)));
        assert_eq!(
            split_font("Noto Sans CJK JP 11"),
            Some(("Noto Sans CJK JP", 11.0))
        );
        assert_eq!(
            split_font("Cantarell Bold 12.5"),
            Some(("Cantarell Bold", 12.5))
        );
        // AND WHAT CANNOT BE READ IS LEFT ALONE. Each of these makes the
        // caller skip that key for ever rather than rewrite it — losing
        // somebody's font is worse than not scaling it.
        assert_eq!(split_font(""), None);
        assert_eq!(split_font("Cantarell"), None);
        assert_eq!(split_font("Cantarell nought"), None);
        assert_eq!(split_font("Cantarell 0"), None);
        assert_eq!(split_font("Cantarell -11"), None);
    }

    #[test]
    fn the_size_is_always_taken_from_the_untouched_description() {
        // THE DEFECT THIS SHAPE EXISTS AGAINST is compounding. The size is
        // applied again on every change, so scaling what is already in dconf
        // would multiply the last change by this one: 125% twice is 156%, and
        // nothing on the screen would say why.
        let had = "Cantarell 11";
        assert_eq!(font_at(had, 100).as_deref(), Some("Cantarell 11.0"));
        assert_eq!(font_at(had, 125).as_deref(), Some("Cantarell 13.8"));
        assert_eq!(font_at(had, 200).as_deref(), Some("Cantarell 22.0"));
        assert_eq!(font_at(had, 70).as_deref(), Some("Cantarell 7.7"));
        // Applying twice from the SAME starting point answers the same thing,
        // which is what makes the write idempotent.
        assert_eq!(font_at(had, 125), font_at(had, 125));
        // And going back to a hundred puts back what was found.
        let (family, points) = split_font(had).unwrap();
        assert_eq!(
            font_at(had, 100).as_deref(),
            Some(&*format!("{family} {points:.1}"))
        );
        // A DECIMAL SURVIVES, because the ladder steps by five: 11 pt at 105%
        // is 11,55 and at 110% is 12,1. Rounded to whole points both are 12,
        // and one press of the key would do nothing.
        assert_ne!(font_at(had, 105), font_at(had, 110));
    }

    #[test]
    fn a_gvariant_string_survives_the_round_trip() {
        // `dconf read` answers a quoted literal and `dconf write` wants one.
        assert_eq!(quoted("Cantarell 11"), "'Cantarell 11'");
        assert_eq!(unquoted("'Cantarell 11'").as_deref(), Some("Cantarell 11"));
        assert_eq!(
            unquoted("'Cantarell 11'\n").as_deref(),
            Some("Cantarell 11")
        );
        // AN UNSET KEY ANSWERS NOTHING, and nothing is not a font.
        assert_eq!(unquoted(""), None);
        assert_eq!(unquoted("\n"), None);
        assert_eq!(unquoted("''"), None);
        // A quote inside the value must not end the literal early.
        assert_eq!(quoted("Bob's Sans 11"), "'Bob\\'s Sans 11'");
        assert_eq!(
            unquoted(&quoted("Bob's Sans 11")).as_deref(),
            Some("Bob's Sans 11")
        );
    }

    #[test]
    fn the_terminal_is_scaled_by_the_same_arithmetic() {
        // Ghostty takes POINTS, and the base is the token both this and
        // `nix/modules/terminal.nix` read — so the terminal cannot end up at a
        // different size from everything around it.
        let base = f64::from(ronoctua_tokens::font::TERMINAL_POINTS);
        assert!(
            (base - 11.0).abs() < f64::EPSILON,
            "the token moved: {base}"
        );
        for percent in [70u16, 100, 125, 200] {
            let want = base * f64::from(percent) / 100.0;
            let said = font_at(&format!("x {base}"), percent).unwrap();
            let got: f64 = said.rsplit(' ').next().unwrap().parse().unwrap();
            // AGAINST THE ROUNDING ITSELF, not a tolerance. A font
            // description carries one decimal, so the answer IS `want` rounded
            // to one — and `(13.8 - 13.75).abs()` is 0.050000000000000710,
            // which no epsilon of 0.05 admits. Comparing to the rounded value
            // says what is meant and has no epsilon to get wrong.
            let rounded = (want * 10.0).round() / 10.0;
            assert!(
                (got - rounded).abs() < f64::EPSILON * 16.0,
                "{percent}%: {got} against {rounded}"
            );
        }
    }
}

#[cfg(test)]
mod moves_with_the_size {
    //! THE ONLY TEST IN THIS CRATE THAT TOUCHES `text::set`.
    //!
    //! The size is process-wide and the tests run in parallel, so a second
    //! one would change it under this. It is the same rule `theme::set`
    //! carries, and it is why every assertion about a size lives here rather
    //! than beside the function it is about.

    use ronoctua_protocol::Scale;
    use ronoctua_tokens as tokens;

    /// Every size the host resolves at the size it is currently drawing at.
    fn everything() -> Vec<f32> {
        let mut all: Vec<f32> = [
            Scale::Small,
            Scale::Base,
            Scale::Normal,
            Scale::Large,
            Scale::Huge,
        ]
        .into_iter()
        .map(crate::paint::scale_in_px)
        .collect();

        // THE LEADING GLYPH OF A CHROME RUN, and one from EVERY exception the
        // measured table has. The first version of this pushed a codepoint
        // that is not a token of this system at all, so it fell to the
        // wildcard and gave the same number as the empty case beside it — two
        // entries proving one thing, and the four exception groups never
        // touched. A regression that made every glyph the plain icon size
        // would have passed.
        //
        // The glyphs are the TOKENS' own, never literals: invariant 9 reaches
        // a codepoint, and a `\u{...}` here would be one.
        for token in [
            tokens::icon::BATTERY_FULL,
            tokens::icon::FILE,
            tokens::icon::BLUETOOTH,
            tokens::icon::SYNC,
            tokens::icon::NOTIFICATION_PENDING,
        ] {
            all.push(crate::glyph::chrome_size_of(token));
        }
        // And the fallback, for a run with no glyph at all.
        all.push(crate::glyph::chrome_size(""));
        all.push(tokens::text::size());
        all
    }

    #[test]
    fn every_size_the_host_draws_at_moves_with_the_chosen_one() {
        // WRITTEN AS A PROPERTY, and that is the point: a list of the sizes
        // would have to be kept in step with the code, and the defect this
        // catches is exactly a size somebody forgot to put on a list.
        //
        // It caught one already. `font.size.small` is 15 against a base of
        // 18, and left unscaled it became the TALLER of the two below 84% —
        // a small run then took two cells instead of one and the picture
        // silently lost two of its thirteen runs, on a grid that had not
        // changed. Measured with `ronoctua-render-png --text`.
        tokens::text::set(tokens::text::NORMAL);
        let hundred = everything();
        assert!(!hundred.is_empty());
        assert!(hundred.iter().all(|s| *s > 0.0), "{hundred:?}");

        // AND THE STARTING VALUES ARE REALLY DIFFERENT. Without this the
        // coverage could go hollow again in silence: a glyph moved out of its
        // measured group, or the table flattened, and every entry would agree
        // with every other while still moving. Five glyphs, four measured
        // sizes and the fallback.
        let glyphs: Vec<f32> = hundred[5..10].to_vec();
        let mut apart = glyphs.clone();
        apart.sort_by(f32::total_cmp);
        apart.dedup();
        assert!(
            apart.len() >= 4,
            "the glyph sizes are not distinct enough to prove anything: {glyphs:?}"
        );

        for percent in [
            tokens::font::SCALE_FLOOR,
            85,
            150,
            tokens::font::SCALE_CEILING,
        ] {
            tokens::text::set(percent);
            let moved = everything();
            let want = f32::from(percent) / 100.0;
            assert_eq!(moved.len(), hundred.len());
            for (n, (then, now)) in hundred.iter().zip(&moved).enumerate() {
                let expected = then * want;
                assert!(
                    (now - expected).abs() < 0.001,
                    "size {n} is {now} at {percent}% and should be {expected}"
                );
            }
        }

        // AND THE SHAPE OF THE DEFECT, STATED DIRECTLY, in the SAME test —
        // two tests calling `text::set` in one crate would race each other,
        // which is the rule this module's header states and would otherwise
        // be the first thing to break it. `Heights` reserves cells by scale,
        // and a `Small` that outgrew `Normal` reserved two where the layout
        // expected one.
        for percent in [tokens::font::SCALE_FLOOR, 80, 85, 100, 200] {
            tokens::text::set(percent);
            let small = crate::paint::scale_in_px(Scale::Small);
            let normal = crate::paint::scale_in_px(Scale::Normal);
            assert!(
                small < normal,
                "at {percent}%: small {small} >= normal {normal}"
            );
        }

        tokens::text::set(tokens::text::NORMAL);
        assert_eq!(tokens::text::percent(), tokens::text::NORMAL);
    }
}
