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
//! # What this does NOT claim
//!
//! That every foreign application follows. GTK 4 maps this key onto
//! `gtk-xft-dpi` and moves while it is running — that much is known. Chromium
//! and Electron are said to, and it was NOT measured here: doing so needs one
//! of them open in a session with a portal, which no test in this repository
//! has. Qt follows only under the `full` profile, where a platform theme is
//! installed at all. And Ghostty follows nothing: it reads its own file.
//!
//! So this promises what it can and `docs/PENDING.md` carries the rest, named
//! — including how to find out.

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

/// The dconf key every toolkit that follows anything follows.
///
/// It is a MULTIPLIER, not a size: `1.25` means "a quarter larger than
/// whatever this application would have drawn". That is what makes it the
/// right thing to hand a foreign application — it keeps that application's
/// own idea of a comfortable size and moves it by the same amount the shell
/// moved its own.
pub const KEY: &str = "/org/gnome/desktop/interface/text-scaling-factor";

/// `dconf`, by path and not by name.
///
/// A user unit's `PATH` is minimal, and a bare name fails far from the cause
/// — the trap `CLAUDE.md` records. Outside Nix it falls back to the name.
const DCONF: &str = match option_env!("RONOCTUA_DCONF") {
    Some(c) => c,
    None => "dconf",
};

/// The multiplier a percentage is, written the way dconf reads a double.
///
/// Two decimals, and always with a point: dconf's `d` type is parsed by
/// GVariant, which does not read `1,25` however the person's locale spells a
/// number. `{:.2}` on an `f64` is locale-independent in Rust, and this is
/// where that fact is being relied on rather than assumed.
#[must_use]
pub fn multiplier(percent: u16) -> String {
    format!("{:.2}", f64::from(percent) / 100.0)
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
    match ronoctua_files::notice::Notice::on_file(dir, ronoctua_display::prefs::FILE, on_stir) {
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
/// which is why the waiting happens on a thread — the arrangement
/// `power_window`'s forwarding uses, for the same reason it gives there.
pub fn tell_them(percent: u16) {
    let value = multiplier(percent);
    match Command::new(DCONF)
        .args(["write", KEY, &value])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        // REAPED ON A THREAD, and not dropped. `Child`'s `Drop` does not
        // wait, so dropping it leaves a zombie for as long as this process
        // lives — one per change of size, and the display view's `←`/`→` on
        // the text row can produce one per keypress. Waiting here would block
        // the frame, which is the thing that must not happen; a thread does
        // neither.
        Ok(child) => {
            // THE CHILD IS NOT SURRENDERED UNTIL THE THREAD EXISTS.
            //
            // The first attempt at this moved it into the closure and logged
            // the spawn's failure — but `Builder::spawn` DROPS the closure
            // when it fails, taking the child with it, un-waited. So the
            // fallback was a comment describing code that was not there,
            // which is worse than the zombie it was written to prevent: the
            // comment made it look handled.
            //
            // Sharing it means the thread takes it if it starts, and this
            // takes it back if it does not. Blocking here is the lesser harm:
            // a machine that cannot spawn a thread has worse trouble than one
            // slow frame.
            let held = std::sync::Arc::new(std::sync::Mutex::new(Some(child)));
            let theirs = std::sync::Arc::clone(&held);
            let started = std::thread::Builder::new()
                .name("dconf".to_owned())
                .spawn(move || {
                    let taken = theirs.lock().ok().and_then(|mut c| c.take());
                    if let Some(mut c) = taken {
                        let _ = c.wait();
                    }
                });
            if let Err(e) = started {
                tracing::warn!(error = %e, "no thread to reap dconf with; waiting here");
                let taken = held.lock().ok().and_then(|mut c| c.take());
                if let Some(mut c) = taken {
                    let _ = c.wait();
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "no dconf; foreign applications keep their own text size");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_multiplier_is_a_double_gvariant_can_read() {
        // GVariant parses it, and it does not know what a comma means in a
        // number — a machine whose locale writes `1,25` would hand dconf
        // something it refuses, and the refusal would be on a child process
        // nobody is waiting for.
        assert_eq!(multiplier(100), "1.00");
        assert_eq!(multiplier(125), "1.25");
        assert_eq!(multiplier(70), "0.70");
        assert_eq!(multiplier(200), "2.00");
        for percent in [70u16, 75, 100, 155, 200] {
            let m = multiplier(percent);
            assert!(!m.contains(','), "{m}");
            assert!(m.parse::<f64>().is_ok(), "{m}");
        }
    }

    #[test]
    fn a_hundred_percent_is_the_toolkits_own_size() {
        // The key is a MULTIPLIER. Writing `1.0` is not "18 pixels", it is
        // "whatever you were going to do" — which is the only honest thing to
        // hand an application whose comfortable size is not ours to know.
        assert_eq!(multiplier(ronoctua_tokens::text::NORMAL), "1.00");
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
            all.push(crate::surface::chrome_size_of(token));
        }
        // And the fallback, for a run with no glyph at all.
        all.push(crate::surface::chrome_size(""));
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
