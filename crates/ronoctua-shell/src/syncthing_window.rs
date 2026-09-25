//! Syncthing, in a window of its own — the second door onto `crate::syncthing`.
//!
//! # What is here and what is not
//!
//! The plumbing of a window is `crate::door`'s; what is here is the view, the
//! daemon's thread and the one file of its own this window follows. It
//! DECIDES nothing — which rows exist and what a key does to them is
//! `syncthing.rs`, and both doors ask the same view the same questions.
//!
//! # The one thing this door CANNOT do
//!
//! **Type.** Adding a machine means a 63-character id, and adding a folder
//! means a path; the one place anything is typed in this system is the
//! footer's minibuffer, and this window has none. So it forwards the request
//! down the control socket and SAYS SO on its message line — the rule the
//! network view's password and the glance's time zone already follow.

use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Keysym, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.syncthing";

/// Opens the Syncthing view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // `syncthing.json`, watched like the theme: a setting changed in the
        // footer moves this window without it being reopened. Both doors write
        // this file, which is why it is read back rather than assumed.
        let (wish_sender, wish_changed) = channel::channel::<()>();
        let watch = ronoctua_syncthing::prefs::home().and_then(|dir| {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                tracing::warn!(error = %e, "no configuration directory to watch syncthing.json in");
                return None;
            }
            ronoctua_files::notice::Notice::on_file(
                &dir,
                ronoctua_syncthing::prefs::FILE,
                ronoctua_files::notice::Pace::Coalesced,
                move || wish_sender.send(()).is_ok(),
            )
            .ok()
        });
        w.handle
            .insert_source(
                wish_changed,
                |event, (), door: &mut Door<SyncthingWindow>| {
                    if let channel::Event::Msg(()) = event {
                        let fresh = ronoctua_syncthing::prefs::store().read();
                        if fresh != door.content.view.preferences() {
                            door.content.view.took_preferences(fresh);
                            door.ui.ask_paint();
                        }
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the syncthing.json watch: {e}"))?;

        // SYNCTHING, on threads of this process — the long poll is the clock.
        let (syncthing_sender, syncthing_events) = channel::channel::<ronoctua_syncthing::Event>();
        let syncthing = Some(ronoctua_syncthing::watch(move |e| {
            let _ = syncthing_sender.send(e);
        }));
        w.handle
            .insert_source(
                syncthing_events,
                |event, (), door: &mut Door<SyncthingWindow>| {
                    if let channel::Event::Msg(e) = event {
                        door.content.syncthing_event(&mut door.ui, e);
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the syncthing channel: {e}"))?;

        Ok(SyncthingWindow {
            view: crate::syncthing::View::fresh(ronoctua_syncthing::prefs::store().read()),
            snapshot: None,
            syncthing,
            _watch: watch,
        })
    })
}

/// The window.
struct SyncthingWindow {
    view: crate::syncthing::View,
    /// The last reading, from this process's own threads.
    snapshot: Option<ronoctua_syncthing::Snapshot>,
    /// The orders thread.
    syncthing: Option<ronoctua_syncthing::Handle>,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl SyncthingWindow {
    fn syncthing_event(&mut self, ui: &mut Ui, e: ronoctua_syncthing::Event) {
        match e {
            ronoctua_syncthing::Event::Snapshot(s) => {
                self.snapshot = Some(*s);
                self.view
                    .settle_in(self.snapshot.as_ref(), ui.columns, &ui.texts);
                ui.ask_paint();
            }
            ronoctua_syncthing::Event::Done { refused, .. } => {
                let Some(why) = refused else { return };
                ui.message = Some(ui.texts.t(crate::syncthing::refusal_key(why)));
                ui.ask_paint();
            }
        }
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: crate::syncthing::Errand) {
        use crate::syncthing::Errand;
        match errand {
            Errand::Nothing => {}
            // AN INFORMATIVE ROW COPIES ITSELF — `chrome::information_of`,
            // and `chrome::copied` for the one place the doors share. It earns
            // its keep here more than anywhere: a machine id is 63 characters
            // nobody types twice.
            Errand::Copy(text) => {
                ui.message = Some(crate::chrome::copied(&text, &ui.texts));
            }
            Errand::Speak(key) => {
                ui.message = Some(ui.texts.t(key));
            }
            Errand::Close => {
                ui.quit = true;
                return;
            }
            Errand::Order(order) => match &self.syncthing {
                Some(h) => h.send(*order),
                None => {
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::ECHO_SYNCTHING_NOT_RUNNING));
                }
            },
            // THE FILE IS WRITTEN HERE, and the footer watches it. Both doors
            // write it, which is why the crate's own `keep` is what decides
            // whether there is anything to overwrite — `Store::parsed`, not
            // `read`, and the reason is that a reader's default written back
            // over a file that will not parse is that file gone.
            Errand::Keep(prefs) => {
                let store = ronoctua_syncthing::prefs::store();
                if let Err(e) = ronoctua_syncthing::prefs::keep(&store, prefs) {
                    tracing::warn!(error = %e, "could not write syncthing.json");
                    ui.message = Some(
                        ui.texts
                            .t(ronoctua_i18n::key::ECHO_SYNCTHING_WILL_NOT_PARSE),
                    );
                }
            }
            Errand::Web(address) => Self::open_the_page(ui, &address),
            // TYPING IS THE FOOTER'S, and this window says so. Sending the
            // request down the control socket would open the minibuffer over
            // there while the person is looking at here, which is worse than
            // a sentence: it is a prompt on another screen.
            Errand::Ask(_) => {
                ui.message = Some(
                    ui.texts
                        .t(ronoctua_i18n::key::SYNCTHING_TYPED_IN_THE_FOOTER),
                );
            }
        }
        ui.ask_paint();
    }

    /// Open Syncthing's own page in whatever opens links here.
    fn open_the_page(ui: &mut Ui, address: &str) {
        let Some(opener) = option_env!("RONOCTUA_XDG_OPEN").filter(|p| !p.is_empty()) else {
            tracing::error!("RONOCTUA_XDG_OPEN was not set at build time");
            ui.message = Some(ui.texts.t(ronoctua_i18n::key::ECHO_SYNCTHING_STOPPED));
            return;
        };
        if address.is_empty() {
            ui.message = Some(ui.texts.t(ronoctua_i18n::key::ECHO_SYNCTHING_STOPPED));
            return;
        }
        // DETACHED, and never waited on: this runs inside the calloop
        // callback, and a browser that takes a second to start would freeze
        // the window for that second.
        match std::process::Command::new(opener)
            .arg(format!("http://{address}"))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => drop(child),
            Err(e) => tracing::warn!(error = %e, "could not open the page"),
        }
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::syncthing::Press) {
        ui.message = None;
        let errand = self
            .view
            .key(press, self.snapshot.as_ref(), ui.columns, &ui.texts);
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::syncthing::Row> {
        self.view
            .rows(self.snapshot.as_ref(), ui.columns, &ui.texts)
    }
}

impl Content for SyncthingWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::SYNCTHING_TITLE))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) = press_of(event, mods.shift) {
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
    /// puts the cursor on the row, and a click on the row it is already on
    /// is that row's `RET`, through the same `pressed` a key goes through.
    /// The door follows the cursor afterwards against the list as it is
    /// THEN, which matters here: clicking a folder or a drawer changes which
    /// level is showing.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        let again = self
            .view
            .clicked(row, &rows, self.snapshot.as_ref(), ui.columns, &ui.texts);
        if again {
            self.pressed(ui, crate::syncthing::Press::Enter);
        }
    }
}

/// A `KeyEvent` as the view sees it — the same mapping the footer does.
///
/// BOTH DOORS GO THROUGH `syncthing::OF_THE_VIEW`. This one turns SCTK's keysym
/// into the table's notation and asks the view; the footer turns its own
/// `Key` into the same notation and asks the same question. A key written in
/// only one of the two would be a key one door answers and the other does
/// not, which is the defect `bluetooth-keys-reach-both-doors` was written for.
fn press_of(event: &KeyEvent, shifted: bool) -> Option<crate::syncthing::Press> {
    // THE NOTATION CARRIES SHIFT, unlike the sound window's. This view's value
    // keys ARE the shifted arrows, and a notation built without the modifier
    // turns `S-<right>` into a plain one — which walks the list instead of
    // turning the ring, silently.
    let notation = match event.keysym {
        Keysym::Escape => return Some(crate::syncthing::Press::Escape),
        Keysym::Return | Keysym::KP_Enter => "RET".to_owned(),
        Keysym::Up => "<up>".to_owned(),
        Keysym::Down => "<down>".to_owned(),
        Keysym::Left | Keysym::Right => {
            let arrow = if event.keysym == Keysym::Left {
                "<left>"
            } else {
                "<right>"
            };
            if shifted {
                format!("S-{arrow}")
            } else {
                arrow.to_owned()
            }
        }
        _ => event.utf8.as_deref()?.to_owned(),
    };
    crate::syncthing::pressed(&notation)
}
