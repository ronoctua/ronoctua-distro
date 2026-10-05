//! Mice, touchpads and every cursor, in a window of its own — the second
//! door onto `crate::pointers`.
//!
//! A window and nothing else: which rows exist and what a key does to them is
//! `pointers.rs`, and both doors ask the same view the same questions. The
//! plumbing every window shares is `crate::door`.
//!
//! # It does not DETECT
//!
//! The footer's pointer thread is the one that gives a new device a cursor of
//! its own, because it is alive for the whole session; this door's thread
//! only reads and carries out what is asked here (`watch(false, …)`). Two
//! doors numbering one mouse would race.

use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.pointers";

/// Opens the pointers view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // `pointers.json`, watched like the theme: a setting changed in the
        // footer moves this window without it being reopened. Both doors
        // write this file, so what is drawn is always a fresh read of it.
        let (wish_sender, wish_changed) = channel::channel::<()>();
        let watch = ronoctua_pointers::prefs::home().and_then(|dir| {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                tracing::warn!(error = %e, "no configuration directory to watch pointers.json in");
                return None;
            }
            ronoctua_files::notice::Notice::on_file(
                &dir,
                ronoctua_pointers::prefs::FILE,
                ronoctua_files::notice::Pace::Coalesced,
                move || wish_sender.send(()).is_ok(),
            )
            .ok()
        });
        w.handle
            .insert_source(
                wish_changed,
                |event, (), door: &mut Door<PointersWindow>| {
                    if let channel::Event::Msg(()) = event
                        && let Some(h) = door.content.pointers_thread.as_ref()
                    {
                        h.send(ronoctua_pointers::Order::Look);
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the pointers.json watch: {e}"))?;

        // THE POINTER THREAD, reading and asking — never detecting (see the
        // module's header).
        let (pointer_sender, pointer_events) = channel::channel::<ronoctua_pointers::Event>();
        let pointers_thread = match ronoctua_pointers::watch(false, move |e| {
            let _ = pointer_sender.send(e);
        }) {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::warn!(error = %e, "no pointer thread; the window opens with nothing to read");
                None
            }
        };
        w.handle
            .insert_source(
                pointer_events,
                |event, (), door: &mut Door<PointersWindow>| {
                    if let channel::Event::Msg(e) = event {
                        door.content.pointer_event(&mut door.ui, e);
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the pointer channel: {e}"))?;

        Ok(PointersWindow {
            view: crate::pointers::View::fresh(),
            snapshot: None,
            pointers_thread,
            _watch: watch,
        })
    })
}

/// The window: the view, and what it reads.
struct PointersWindow {
    view: crate::pointers::View,
    /// The last reading, from this process's own threads.
    snapshot: Option<ronoctua_pointers::Snapshot>,
    /// The pointer thread, when it could be started.
    pointers_thread: Option<ronoctua_pointers::Handle>,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl PointersWindow {
    fn pointer_event(&mut self, ui: &mut Ui, e: ronoctua_pointers::Event) {
        match e {
            ronoctua_pointers::Event::Snapshot(s) => {
                // THE CURSOR STAYS ON ITS ROW BY IDENTITY: a device plugged
                // in adds rows above it, and an index would drift.
                let spot = self
                    .view_rows(ui)
                    .get(self.view.cursor())
                    .map(|r| r.spot.clone());
                self.snapshot = Some(*s);
                if let Some(spot) = spot {
                    self.view
                        .follow(&spot, self.snapshot.as_ref(), ui.columns, &ui.texts);
                }
                self.view
                    .settle_in(self.snapshot.as_ref(), ui.columns, &ui.texts);
                ui.ask_paint();
            }
            ronoctua_pointers::Event::Done { refused, .. } => {
                if let Some(words) = refused {
                    ui.say(words);
                    ui.ask_paint();
                }
            }
        }
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: crate::pointers::Errand) {
        use crate::pointers::Errand;
        match errand {
            Errand::Nothing => {}
            // AN INFORMATIVE ROW COPIES ITSELF — `chrome::information_of`.
            Errand::Copy(text) => {
                ui.say_for(
                    crate::chrome::copied(&text, &ui.texts),
                    crate::echo::Echo::Brief,
                );
            }
            Errand::Close => {
                ui.quit = true;
                return;
            }
            // THIS DOOR SENDS IT rather than telling somebody to go to the
            // footer: the control socket is how a window runs a command — the
            // keyboards' buffer, which the footer opens in either door — told
            // off the loop, and a footer that is not there is said.
            Errand::Run(id) => ui.tell_the_footer(id, None),
            Errand::Order(order) => match &self.pointers_thread {
                Some(h) => h.send(order),
                None => {
                    ui.say(ui.texts.t(ronoctua_i18n::key::POINTERS_NO_COMPOSITOR));
                }
            },
        }
        ui.ask_paint();
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::pointers::Press) {
        ui.message = None;
        let errand = self
            .view
            .key(press, self.snapshot.as_ref(), ui.columns, &ui.texts);
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::pointers::Row> {
        self.view
            .rows(self.snapshot.as_ref(), ui.columns, &ui.texts)
    }
}

impl Content for PointersWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::POINTERS_TITLE))
    }

    /// A key, read by its whole notation through the reader the footer
    /// calls too (`pointers::press_of_key`): a letter held with `Ctrl` or `Alt`
    /// is not the letter, in either door.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::pointers::press_of_key)
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
        if self.view.clicked(row, &rows) {
            self.pressed(ui, crate::pointers::Press::Enter);
        }
    }
}
