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
//! `None` when the window closes. Nothing here decides to listen; it carries
//! out a decision the cursor already made.

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
            listening: None,
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
    /// Which source this door has asked to have open.
    ///
    /// It shadows `View::listening` so the order is sent when it CHANGES and
    /// not on every keystroke. Repeating `Listen(Some(61))` would drop the
    /// open child and build another one, and a microphone that closed and
    /// reopened on every arrow is a meter that flickers.
    listening: Option<u32>,
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
        let want = self.view.listening();
        if want == self.listening {
            return;
        }
        self.listening = want;
        if let Some(h) = &self.audio {
            h.send(ronoctua_audio::Order::Listen(want));
        }
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: crate::audio::Errand) {
        use crate::audio::Errand;
        match errand {
            Errand::Nothing => {}
            // AN INFORMATIVE ROW COPIES ITSELF — `chrome::information_of`,
            // and `chrome::copied` for the one place the doors share.
            Errand::Copy(text) => {
                ui.say_for(
                    crate::chrome::copied(&text, &ui.texts),
                    crate::echo::Echo::Brief,
                );
            }
            Errand::Speak(key) => {
                ui.say(ui.texts.t(key));
            }
            Errand::Close => {
                // THE MICROPHONE CLOSES BEFORE THE WINDOW DOES. Dropping the
                // handle would close it too — the meter is a value with a
                // guard — but the order is sent while there is still a thread
                // to receive it, which is the difference between "it closed"
                // and "it closed when the process happened to exit".
                if let Some(h) = &self.audio {
                    h.send(ronoctua_audio::Order::Listen(None));
                }
                self.listening = None;
                ui.quit = true;
                return;
            }
            Errand::Order(order) => match &self.audio {
                Some(h) => h.send(order),
                None => {
                    ui.say(ui.texts.t(ronoctua_i18n::key::AUDIO_NO_PIPEWIRE));
                }
            },
            // THE FILE IS WRITTEN HERE, and the footer watches it. Both doors
            // write it, which is why `Store::parsed` is what decides whether
            // there is anything to overwrite.
            Errand::Keep(prefs) => {
                let store = ronoctua_audio::prefs::store();
                if let Err(e) = ronoctua_audio::prefs::keep(&store, prefs) {
                    tracing::warn!(error = %e, "could not write audio.json");
                    ui.say_for(
                        ui.texts.t(ronoctua_i18n::key::ECHO_AUDIO_WILL_NOT_PARSE),
                        crate::echo::Echo::Long,
                    );
                }
            }
        }
        ui.ask_paint();
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
        format!("*{}*", texts.t(ronoctua_i18n::key::AUDIO_TITLE))
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
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
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
