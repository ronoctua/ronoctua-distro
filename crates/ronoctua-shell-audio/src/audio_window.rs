//! Sound, in a window of its own — the second door onto `crate::audio`.
//!
//! # What is here and what is not
//!
//! A window, and the channels of its own daemon. It DECIDES nothing — which
//! rows exist and what a key does to them is `audio.rs`, and both doors ask
//! the same view the same questions. The plumbing every window shares is
//! `crate::door`.
//!
//! # The one thing this door has that the others do not
//!
//! **It opens a microphone.** The view says which source belongs open
//! (`View::listening`) and this asks for it — after a key, after a click, and
//! `None` when the window closes — through the meter both doors share
//! (`audio_region::Meter`). Nothing here decides to listen; it carries out a
//! decision the cursor already made.

use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.audio";

/// Opens the sound view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // `audio.json`, watched like the theme: a setting changed in the
        // footer moves this window without it being reopened. Both doors
        // write this file, which is why it is read back rather than assumed.
        let (wish_sender, wish_changed) = channel::channel::<()>();
        let watch = ronoctua_audio::prefs::home().and_then(|dir| {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                tracing::warn!(error = %e, "no configuration directory to watch audio.json in");
                return None;
            }
            ronoctua_files::notice::Notice::on_file(
                &dir,
                ronoctua_audio::prefs::FILE,
                ronoctua_files::notice::Pace::Coalesced,
                move || wish_sender.send(()).is_ok(),
            )
            .ok()
        });
        w.handle
            .insert_source(wish_changed, |event, (), door: &mut Door<AudioWindow>| {
                if let channel::Event::Msg(()) = event {
                    let fresh = ronoctua_audio::prefs::store().read();
                    if fresh != door.content.view.preferences() {
                        door.content.view.took_preferences(fresh);
                        door.ui.ask_paint();
                    }
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the audio.json watch: {e}"))?;

        // PIPEWIRE, on threads of this process — the monitor is the clock.
        let (audio_sender, audio_events) = channel::channel::<ronoctua_audio::Event>();
        let audio = match ronoctua_audio::watch(move |e| {
            let _ = audio_sender.send(e);
        }) {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::warn!(error = %e, "no sound server; the window opens with nothing to read");
                None
            }
        };
        w.handle
            .insert_source(audio_events, |event, (), door: &mut Door<AudioWindow>| {
                if let channel::Event::Msg(e) = event {
                    door.content.audio_event(&mut door.ui, e);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the audio channel: {e}"))?;

        Ok(AudioWindow {
            view: crate::audio::View::fresh(ronoctua_audio::prefs::store().read()),
            snapshot: None,
            audio,
            meter: crate::audio_region::Meter::default(),
            _watch: watch,
        })
    })
}

/// The window: the view, and what it reads.
struct AudioWindow {
    view: crate::audio::View,
    /// The last reading, from this process's own threads.
    snapshot: Option<ronoctua_audio::Snapshot>,
    /// The orders thread, when the sound server was there.
    audio: Option<ronoctua_audio::Handle>,
    /// Which source this door has asked to have open — the meter both doors
    /// open a microphone through (`audio_region::Meter`).
    meter: crate::audio_region::Meter,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl AudioWindow {
    fn audio_event(&mut self, ui: &mut Ui, e: ronoctua_audio::Event) {
        match e {
            ronoctua_audio::Event::Snapshot(s) => {
                self.snapshot = Some(*s);
                self.view
                    .settle_in(self.snapshot.as_ref(), ui.columns, &ui.texts);
                // A DEVICE CAN BE UNPLUGGED WHILE ITS ROW IS UNDER THE
                // CURSOR, and then the microphone this door asked for is a
                // source that no longer exists. `settle` has just moved the
                // cursor; this is what tells the meter about it.
                self.follow_the_microphone();
                ui.ask_paint();
            }
            // A LEVEL IS NOT A SNAPSHOT. It arrives ten times a second and
            // the graph does not, so folding one into the other would make
            // every meter frame look like the devices had changed — and would
            // redraw the whole list for a bar that may not even have moved.
            ronoctua_audio::Event::Level(level) => {
                if self.view.took_level(level, ui.columns) {
                    ui.ask_paint();
                }
            }
            ronoctua_audio::Event::Done { refused, .. } => {
                if let Some((why, words)) = refused {
                    ui.say(crate::audio_words::refused(why, &words, &ui.texts));
                    ui.ask_paint();
                }
            }
        }
    }

    /// Tell the meter which source the cursor is on, when that changed.
    fn follow_the_microphone(&mut self) {
        self.meter
            .follow_the_microphone(&self.view, self.audio.as_ref());
    }

    /// Carries out what the view asked for — the one handler the footer's
    /// region carries out too (`audio_region::carry_out`), with this
    /// window's own thread and microphone. The file it writes the footer
    /// watches.
    fn apply(&mut self, ui: &mut Ui, errand: crate::audio::Errand) {
        crate::audio_region::carry_out(errand, &mut self.meter, self.audio.as_ref(), None, ui);
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::audio::Press) {
        ui.message = None;
        let errand = self
            .view
            .key(press, self.snapshot.as_ref(), ui.columns, &ui.texts);
        self.follow_the_microphone();
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::audio::Row> {
        self.view
            .rows(self.snapshot.as_ref(), ui.columns, &ui.texts)
    }
}

impl Content for AudioWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::AUDIO_TITLE))
    }

    /// A key, read by its whole notation through the reader the footer
    /// calls too (`audio::press_of_key`): a letter held with `Ctrl` or `Alt`
    /// is not the letter, in either door.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::audio::press_of_key)
        {
            self.pressed(ui, press);
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view
            .hint(self.snapshot.as_ref(), ui.columns, &ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    /// A click takes a row the cursor stands on, and only that row lights
    /// under the pointer — the view's one answer ([`crate::audio::Row::takes`]),
    /// which the footer's region gives too.
    fn takes(&self, ui: &Ui) -> Vec<bool> {
        self.view_rows(ui)
            .iter()
            .map(crate::audio::Row::takes)
            .collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19: a click
    /// puts the cursor on the row, and a click on the row it is already on is
    /// that row's `RET`, through the same `pressed` a key goes through.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        let again = self
            .view
            .clicked(row, &rows, self.snapshot.as_ref(), ui.columns, &ui.texts);
        // THE ROW COUNT MAY HAVE MOVED, because clicking a microphone opens
        // one and the level row comes with it — which is why the list is
        // followed after this and not before.
        self.follow_the_microphone();
        if again {
            self.pressed(ui, crate::audio::Press::Enter);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window over [`crate::audio::a_machine`] — no PipeWire behind it and
    /// no `audio.json` watched, on the settings a fresh view keeps — and
    /// never this machine's.
    fn a_window() -> (AudioWindow, Ui) {
        let ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        let snapshot = crate::audio::a_machine();
        let mut view = crate::audio::View::fresh(ronoctua_audio::prefs::Preferences::default());
        view.settle_in(Some(&snapshot), ui.columns, &ui.texts);
        let w = AudioWindow {
            view,
            snapshot: Some(snapshot),
            audio: None,
            meter: crate::audio_region::Meter::default(),
            _watch: None,
        };
        (w, ui)
    }

    /// FAILS IF: the window's resting pointer lights a row a click takes
    /// nothing on — a title, the air between sections, a sentence about a
    /// whole section — or leaves unlit one it takes. THE LIGHT IS THE CLICK:
    /// every row clicked once, on a window of its own, and lit exactly where
    /// something happened. The footer's region has its own.
    #[test]
    fn the_windows_pointer_lights_what_the_click_takes() {
        let held = crate::door::held_to_the_click(a_window, crate::door::seen);
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }
}
