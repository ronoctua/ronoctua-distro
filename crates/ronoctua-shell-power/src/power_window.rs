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
//! that outlives this window: those are handed to the footer down the control
//! socket (`Ui::tell_the_footer`), as the commands the leader runs with the
//! state WANTED as their argument, and a footer that is not there is a
//! sentence on the header.

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
                    let wanted = crate::power_region::wished();
                    if door.content.wanted.get() != wanted {
                        door.content.wanted.set(wanted);
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
            wanted: std::cell::Cell::new(crate::power_region::wished()),
            _watch: watch,
        })
    })
}

/// The window: the view, and what it reads.
struct Power {
    view: crate::power::View,
    /// The last reading, from this process's own threads.
    snapshot: Option<ronoctua_power::Snapshot>,
    /// The orders thread, when the daemons were there.
    power: Option<ronoctua_power::Handle>,
    /// The profile the person chose (or the distro's floor) — drawn as a
    /// mark beside the profile rows, and set by a choice made here.
    wanted: std::cell::Cell<ronoctua_power::Profile>,
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
                    ui.say(crate::power_words::refused(&order, why, &words, &ui.texts));
                    ui.ask_paint();
                }
            }
        }
    }

    /// Carries out what the view asked for — the one handler the footer's
    /// region carries out too (`power_region::carry_out`), with this window's
    /// own threads and wish. A lock goes to the footer, which holds it.
    fn apply(&mut self, ui: &mut Ui, errand: crate::power::Errand) {
        crate::power_region::carry_out(errand, self.power.as_ref(), Some(&self.wanted), ui);
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
            self.wanted.get(),
            ui.columns,
            &ui.texts,
        );
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::power::Row> {
        self.view.rows(
            self.snapshot.as_ref(),
            self.wanted.get(),
            ui.columns,
            &ui.texts,
        )
    }
}

impl Content for Power {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::POWER_TITLE))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, _: Modifiers) {
        if let Some(press) = press_of(event) {
            self.pressed(ui, press);
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&ui.texts)
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A window with no daemons and no footer behind it.
    fn a_window() -> (Power, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let me = Power {
            view: crate::power::View::fresh(),
            snapshot: None,
            power: None,
            wanted: std::cell::Cell::new(ronoctua_power::Profile::Balanced),
            _watch: None,
        };
        (me, Ui::for_tests(texts))
    }

    /// A LOCK IS ASKED OF THE FOOTER WITH THE STATE WANTED, off the loop, and
    /// nothing tells the person to go anywhere.
    #[test]
    fn a_lock_goes_to_the_footer_with_the_state_wanted() {
        use ronoctua_power::{Keep, Order};
        for (keep, on, order) in [
            (Keep::Sleep, true, "command power-keep-awake on"),
            (Keep::Sleep, false, "command power-keep-awake off"),
            (Keep::Lid, true, "command power-keep-lid on"),
        ] {
            let (mut w, mut ui) = a_window();
            w.apply(
                &mut ui,
                crate::power::Errand::Order(Order::KeepAwake(keep, on)),
            );
            assert_eq!(ui.orders(), [order.to_owned()]);
            assert!(ui.message.is_none(), "{:?}", ui.message);
        }
    }
}
