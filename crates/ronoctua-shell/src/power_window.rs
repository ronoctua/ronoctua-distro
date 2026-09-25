//! The power view's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a ceiling in lines (`space.expansion`,
//! invariant 14), and the power view has eight sections: a glance at the
//! battery fits the footer, reading every fact and every switch does not. So
//! this exists — and everything it knows lives in [`crate::power::View`],
//! which the footer draws from too. What this module holds is a window, a GPU
//! and the events of both; what it decides is nothing. It is the arrangement
//! `notifications_window.rs` and `files_window.rs` share, and the same header.
//!
//! # Where its data comes from
//!
//! From the same three daemons the footer listens to, on threads of its own
//! (`ronoctua_power::watch`): a window has no clock, and the bus says when
//! something changed. Orders go to those daemons directly — a profile, a
//! threshold, a suspend are the same call from any process — EXCEPT the two
//! inhibitor locks, which are file descriptors and have to live in the process
//! that outlives this window: those are forwarded to the footer as the
//! commands `LEADER s p k` and `LEADER s p l` run.

use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Keysym, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.power";

/// Opens the power view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // `power.json`, watched: a profile chosen in the footer moves this
        // window's "(your choice)" mark without it being reopened. The footer
        // is the one that APPLIES the file; here it is read.
        let (wish_sender, wish_changed) = channel::channel::<()>();
        let watch = ronoctua_power::prefs::home().and_then(|dir| {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                tracing::warn!(error = %e, "no configuration directory to watch power.json in");
                return None;
            }
            ronoctua_files::notice::Notice::on_file(
                &dir,
                ronoctua_power::prefs::FILE,
                ronoctua_files::notice::Pace::Coalesced,
                move || wish_sender.send(()).is_ok(),
            )
            .ok()
        });
        w.handle
            .insert_source(wish_changed, |event, (), door: &mut Door<Power>| {
                if let channel::Event::Msg(()) = event {
                    let wanted = wished();
                    if door.content.wanted != wanted {
                        door.content.wanted = wanted;
                        door.ui.ask_paint();
                    }
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the power.json watch: {e}"))?;

        // THE DAEMONS, on threads of this process — the bus is the clock.
        let (power_sender, power_events) = channel::channel::<ronoctua_power::Event>();
        let power = match ronoctua_power::watch(move |e| {
            let _ = power_sender.send(e);
        }) {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::warn!(error = %e, "no power daemons; the window opens with nothing to read");
                None
            }
        };
        w.handle
            .insert_source(power_events, |event, (), door: &mut Door<Power>| {
                if let channel::Event::Msg(e) = event {
                    door.content.power_event(&mut door.ui, e);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the power channel: {e}"))?;

        Ok(Power {
            view: crate::power::View::fresh(),
            snapshot: None,
            power,
            wanted: wished(),
            _watch: watch,
        })
    })
}

/// The profile the person chose, or the distro's floor.
fn wished() -> ronoctua_power::Profile {
    let (wanted, _, complaints) = ronoctua_power::prefs::resolve_in(
        ronoctua_power::prefs::home().as_deref(),
        std::env::var(ronoctua_power::prefs::ENV).ok().as_deref(),
    );
    for c in complaints {
        tracing::warn!(%c, "power.json");
    }
    wanted
}

/// The window: the view, and what it reads.
struct Power {
    view: crate::power::View,
    /// The last reading, from this process's own threads.
    snapshot: Option<ronoctua_power::Snapshot>,
    /// The orders thread, when the daemons were there.
    power: Option<ronoctua_power::Handle>,
    /// The profile the person chose (or the distro's floor) — drawn as a
    /// mark beside the profile rows.
    wanted: ronoctua_power::Profile,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl Power {
    fn power_event(&mut self, ui: &mut Ui, e: ronoctua_power::Event) {
        match e {
            ronoctua_power::Event::Snapshot(s) => {
                // THE VIEW FOLLOWS THE NEW ROWS, the footer's reason: rows
                // come and go, and a pair the arrows were moving is a lie
                // once somebody else has written the thresholds.
                let moved = self.snapshot.as_ref().map(|b| b.thresholds.clone())
                    != Some(s.thresholds.clone());
                self.snapshot = Some(*s);
                if moved {
                    self.view.forget_pending();
                }
                let rows = self.view_rows(ui);
                self.view.settle(&rows);
                ui.ask_paint();
            }
            ronoctua_power::Event::Done { order, refused } => {
                if let Some((why, words)) = refused {
                    ui.message = Some(crate::power_words::refused(&order, why, &words, &ui.texts));
                    ui.ask_paint();
                }
            }
        }
    }

    /// "The lock is the footer's to hold", with the two sequences RESOLVED.
    ///
    /// The keys are an ARGUMENT and never text in the `.ftl`: a sentence that
    /// carried them went stale the day either moved, and the word "LEADER" is
    /// nobody's key — `nix/modules/scroll.nix` says `SUPER` and `nix/dev.nix`
    /// says `CTRL ALT SPC`. `crate::leader()` is the one place that knows.
    ///
    /// `leader()` and not `leader_chord()`: these are sequences typed after
    /// the leader, and the chord's label is the modifier held for the arrows —
    /// in the nested loop the two are not the same key.
    fn answer_in_the_footer(texts: &ronoctua_i18n::Texts) -> String {
        let leader = crate::leader();
        texts.t_with(
            ronoctua_i18n::key::POWER_ANSWER_IN_THE_FOOTER,
            &[
                (
                    "awake",
                    &format!("{leader} {}", crate::leader::KEEP_AWAKE_KEYS),
                ),
                ("lid", &format!("{leader} {}", crate::leader::KEEP_LID_KEYS)),
            ],
        )
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: crate::power::Errand) {
        use crate::power::Errand;
        match errand {
            Errand::Nothing => {}
            // AN INFORMATIVE ROW COPIES ITSELF — see
            // `chrome::information_of`, and `chrome::copied` for the
            // one place the six doors share.
            Errand::Copy(text) => {
                ui.message = Some(crate::chrome::copied(&text, &ui.texts));
            }
            Errand::Close => {
                ui.quit = true;
                return;
            }
            // THE LOCKS ARE THE FOOTER'S TO HOLD: an inhibitor is an fd, and
            // this window will close. The command the footer runs for the same
            // key is what is asked for — one door, through the other.
            Errand::Order(ronoctua_power::Order::KeepAwake(keep, _)) => {
                // NOT `status()`, WHICH WAITS. This runs inside the calloop
                // callback, and waiting for a child there freezes the window
                // for as long as the child takes — the same trap `AGENTS.md`
                // records about `systemd-run --scope` in the footer's loop. A
                // footer socket that is gone or slow made this window stop
                // painting; a `connect(2)` to a dead socket is fast, and a
                // busy one is not.
                //
                // AND A FAILURE IS A SENTENCE, not silence: `current_exe`
                // failing used to drop the whole thing with no message, which
                // from the outside is a key that did nothing.
                if !crate::door::forward(crate::power_words::keep_command(keep)) {
                    ui.message = Some(Self::answer_in_the_footer(&ui.texts));
                }
            }
            Errand::Order(order) => match &self.power {
                Some(h) => h.send(order),
                None => {
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::POWER_NO_DAEMON));
                }
            },
            // THE ORDER AND THE FILE, as in the footer: a choice made here
            // has to survive the next login. The footer, which watches the
            // file, may send the same order once more — idempotent at the
            // daemon, and only while its own reading still disagrees.
            Errand::Choose(profile) => {
                self.wanted = profile;
                match &self.power {
                    Some(h) => h.send(ronoctua_power::Order::SetProfile(profile)),
                    None => {
                        ui.message = Some(ui.texts.t(ronoctua_i18n::key::POWER_NO_DAEMON));
                    }
                }
                if let Some(dir) = ronoctua_power::prefs::home()
                    && let Err(e) = ronoctua_power::prefs::persist(&dir, profile)
                {
                    tracing::warn!(error = %e, "could not write power.json");
                }
            }
        }
        ui.ask_paint();
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::power::Press) {
        ui.message = None;
        // THE WIDTH THIS WINDOW DRAWS AT, so the key acts on the row on
        // screen — see `power::View::key`.
        let errand = self.view.key(
            press,
            self.snapshot.as_ref(),
            self.wanted,
            ui.columns,
            &ui.texts,
        );
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::power::Row> {
        self.view
            .rows(self.snapshot.as_ref(), self.wanted, ui.columns, &ui.texts)
    }
}

impl Content for Power {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::POWER_TITLE))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, _: Modifiers) {
        if let Some(press) = press_of(event) {
            self.pressed(ui, press);
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&ui.texts)
    }

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
            self.pressed(ui, crate::power::Press::Enter);
        }
    }
}

/// A `KeyEvent` as the view sees it — the same mapping the footer does.
fn press_of(event: &KeyEvent) -> Option<crate::power::Press> {
    use crate::power::Press;
    Some(match event.keysym {
        Keysym::Escape => Press::Escape,
        Keysym::Return | Keysym::KP_Enter => Press::Enter,
        Keysym::Left => Press::Left,
        Keysym::Right => Press::Right,
        Keysym::Up => Press::Up,
        Keysym::Down => Press::Down,
        _ => return None,
    })
}
