//! The screens view's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a ceiling in lines (`space.expansion`,
//! invariant 14), and one screen's sheet is eighteen rows: a glance at what is
//! plugged in fits the footer, reading a monitor whole does not. So this
//! exists — and everything it knows lives in [`crate::display::View`], which
//! the footer draws from too. What this module holds is a window, a GPU and
//! the events of both; what it decides is nothing. It is the arrangement
//! `bluetooth_window.rs` and `power_window.rs` share, and the same header.
//!
//! # Where its data comes from
//!
//! From the compositor and the kernel, on a thread of its own
//! (`ronoctua_display::watch`), and orders go to it directly: a mode, a scale,
//! a brightness are the same call from any process.
//!
//! # THE ONE THING THIS DOOR HAS THAT THE OTHERS DO NOT: A CLOCK
//!
//! FOUR of the other nine doors keep one UNCONDITIONALLY, for the life of the
//! window: the network's and the update's beat once a second, the notification
//! manager's every two, and the buffer host's once a second whenever the
//! memory panel is what it is hosting. The file manager's arms one ONLY when
//! its inotify watcher failed to start, and says so beside it; the power
//! window, the radio's and the terminal have none. This one is armed only
//! while there is something to count, and disarms itself — which is the
//! difference worth writing down, and the count beside `arm_the_clock` says
//! the same number.
//!
//! What it counts is the COUNTDOWN: a change that can leave a screen
//! unreadable is applied provisionally and goes back by itself if nobody
//! answers. The footer's clock is no help — `Footer::display_tick` returns at
//! once unless the FOOTER's own view is open, and the two processes hold
//! separate `View`s — so without a clock here the question would stand for
//! ever, saying the same number, and the change would never be put back at
//! all.
//!
//! ARMED ONLY WHILE ONE RUNS: `Asking` says whether it is on the clock (the
//! forget question is not), and the tick does not re-arm when the question has
//! gone. In repose this window wakes for nothing.

use crate::door::{Canvas, Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.display";

/// Opens the screens view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // `display.json`, WATCHED A SECOND TIME, for what it carries beside
        // the size of the text.
        //
        // THE FILE CARRIES TWO THINGS. The door's own watch answers for the
        // size; this one answers for every screen's `Wanted`, so a scale
        // kept in the FOOTER reaches this window — without it, this window
        // wrote its own stale copy back on the next keep, silently undoing
        // the footer's change. It is the second-writer defect AGENTS.md
        // records for the notification history, one file over.
        let (wanted_sender, wanted_changed) = channel::channel::<()>();
        let watch = crate::toolkits::watch(move || wanted_sender.send(()).is_ok());
        w.handle
            .insert_source(wanted_changed, |event, (), door: &mut Door<Display>| {
                if let channel::Event::Msg(()) = event {
                    door.content.look_again();
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the display.json watch: {e}"))?;

        // THE READER, on a thread of this process. It is the same `watch` the
        // footer starts: the compositor over the IPC, the backlights out of
        // `/sys`, and the monitors over DDC/CI when a reading asks deeply.
        let (screens_sender, screens_events) = channel::channel::<ronoctua_display::Event>();
        let screens = match ronoctua_display::watch(move |e| {
            let _ = screens_sender.send(e);
        }) {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::warn!(error = %e, "no screen thread; the window opens with nothing to read");
                None
            }
        };
        w.handle
            .insert_source(screens_events, |event, (), door: &mut Door<Display>| {
                if let channel::Event::Msg(e) = event {
                    door.content.screens_event(&mut door.ui, e);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the screens channel: {e}"))?;

        // AND WHAT THE COMPOSITOR SAYS ABOUT ITS OWN CONFIGURATION.
        //
        // THE SCREENS MOVING IS THE ONE THING THIS WINDOW MUST HEAR.
        //
        // A refusal no longer arrives as an event: a re-read is something
        // this door ASKS for, and the compositor's answer comes back on the
        // same call — so the door that wrote the fragment is the door that is
        // told it would not parse, at the moment of the gesture.
        //
        // What does arrive unasked is a monitor moving, and that is worth a
        // thread: it catches a change NOBODY HERE MADE — a command typed in a
        // terminal, a lid, a cable.
        let (wm_sender, wm_events) = channel::channel::<ronoctua_compositor::Event>();
        crate::daemon::listen_to_the_compositor(wm_sender);
        w.handle
            .insert_source(wm_events, |event, (), door: &mut Door<Display>| {
                if let channel::Event::Msg(ronoctua_compositor::Event::ScreensChanged) = event {
                    // NOT THROUGH `order`, which says "the screens cannot be
                    // read" when there is no reader. Nobody asked for this, so
                    // a window with no reader has nothing to report and
                    // should not replace whatever is on the message line with
                    // a complaint about an event the person did not cause.
                    door.content.look_again();
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the compositor's channel: {e}"))?;

        // AND THE COUNTDOWN'S OWN SECOND. Four siblings keep a clock too;
        // what is different here is that it is armed only while there is
        // something to count — see this module's header.
        let (tick_sender, ticks) = channel::channel::<()>();
        w.handle
            .insert_source(ticks, |event, (), door: &mut Door<Display>| {
                if let channel::Event::Msg(()) = event {
                    door.content.countdown(&mut door.ui);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the countdown channel: {e}"))?;

        let mut view = crate::display::View::fresh();
        if screens.is_none() {
            // A VIEW WITH NO READER SAYS SO, rather than blaming the
            // compositor — the footer's door does the same.
            view.without_a_reader();
        }
        if watch.is_none() {
            // AND A DOOR THAT CANNOT SEE THE FILE CHANGE does not step the
            // size of the text — see `View::blind` for what it would ask for
            // instead.
            view.without_a_watch();
        }
        Ok(Display {
            view,
            snapshot: ronoctua_display::Snapshot::default(),
            screens,
            tick: tick_sender,
            ticking: false,
            _watch: watch,
        })
    })
}

/// The window, DETACHED, over a reading handed in whole — what
/// its camera (`Camera`) draws. No reader thread and no watch: an order
/// is said to have no thread to go to, and nothing on this machine is read.
pub(crate) fn photographed(snapshot: ronoctua_display::Snapshot) -> Display {
    let (tick, _) = channel::channel::<()>();
    let mut view = crate::display::View::fresh();
    view.saw_a_reading();
    Display {
        view,
        snapshot,
        screens: None,
        tick,
        ticking: false,
        _watch: None,
    }
}

/// The screens' window's camera (`photograph --door display`): over a scene
/// of screens planted whole (`crate::photo::scene`) and the way
/// `display.json` read — walked and drawn by `crate::photo::scenes`.
#[derive(Debug)]
pub(crate) struct Camera {
    /// The scene of screens.
    screens: String,
    /// How `display.json` read.
    settings: String,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            screens: "two".to_owned(),
            settings: "read".to_owned(),
        }
    }
}

impl crate::photo::scenes::Scene for Camera {
    const DOOR: &'static str = "display";
    const IS: &'static str = "the screens' window (`LEADER s d w`) over a scene of screens";
    const WORDS: &'static str = "\
  --screens <scene>     two (side by side), stacked, dark (the second off,
                        with what is filed for it), one (default: two)
  --settings <read>     how `display.json` read — read (whole), failed (not
                        JSON: nothing of it), repaired (a value of it
                        repaired, the rest read) (default: read)";

    fn word(
        &mut self,
        flag: &str,
        value: &mut dyn FnMut() -> anyhow::Result<String>,
    ) -> anyhow::Result<bool> {
        match flag {
            "--screens" => self.screens = value()?,
            "--settings" => {
                let said = value()?;
                anyhow::ensure!(
                    ["read", "failed", "repaired"].contains(&said.as_str()),
                    "`--settings {said}` is not a reading; the readings are read, failed and repaired"
                );
                self.settings = said;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn shoot(self, ui: Ui, asked: &crate::photo::scenes::Asked) -> anyhow::Result<()> {
        let mut snapshot = crate::photo::scene(&self.screens)?;
        snapshot.settings_failed = self.settings == "failed";
        snapshot.settings_repaired = self.settings == "repaired";
        let window = photographed(snapshot);
        crate::photo::scenes::walked(crate::door::Detached::new(ui, window), asked)
    }
}

/// `ronoctua-shell photograph --door display …`, to its end.
pub fn camera(rest: &[String]) -> ! {
    crate::photo::scenes::taken(rest, Camera::default())
}

/// The window: the view, and what it reads.
pub(crate) struct Display {
    view: crate::display::View,
    /// The last reading, from this process's own threads.
    snapshot: ronoctua_display::Snapshot,
    /// The orders thread, when the reader started.
    screens: Option<ronoctua_display::Handle>,
    /// How the countdown wakes this window — see the module's header.
    tick: channel::Sender<()>,
    /// Whether a second is already on its way, so two questions in a row do
    /// not leave two timers running.
    ticking: bool,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl Display {
    fn screens_event(&mut self, ui: &mut Ui, e: ronoctua_display::Event) {
        match e {
            ronoctua_display::Event::Snapshot(now) => {
                // THE VIEW FOLLOWS THE NEW ROWS, the footer's reason: a
                // monitor unplugged takes its row away, and the list reorders
                // under a cursor standing in it.
                self.snapshot = *now;
                // AND THE VIEW KNOWS IT HAS BEEN READ TO. The first frame of
                // this window is drawn against a default `Snapshot`, whose
                // `compositor` is `false` — see `display::View::looked`.
                self.view.saw_a_reading();
                let rows = self.view_rows(ui);
                self.view.settle(&self.snapshot, &rows);
                ui.ask_paint();
            }
            ronoctua_display::Event::Done { order, refused } => {
                let Some(why) = refused else {
                    return;
                };
                // EVERYTHING A REFUSAL DOES TO THE VIEW is `display::refusal`,
                // and the footer's door reads the same one — see it for why
                // it is not written out twice.
                let moment = std::time::Instant::now();
                let doing = crate::display::refusal(
                    &mut self.view,
                    &order,
                    &why,
                    &self.snapshot,
                    ui.columns,
                    moment,
                    &ui.texts,
                );
                // AND THE PAGE FOLLOWS. The question is five rows and the
                // sheet under it eighteen: settling the cursor without moving
                // the page leaves the restored row off the glass until the
                // next keypress — and nobody is pressing, which is what a
                // refusal arriving by itself means. The countdown beside this
                // does it too.
                if doing.follow {
                    let n = self.view_rows(ui).len();
                    ui.follow(self.view.cursor(), n);
                }
                self.doing(ui, doing);
                ui.ask_paint();
            }
        }
    }

    /// One second of a standing countdown — see this module's header.
    ///
    /// IT REARMS ITSELF, and only while there is something to count. The
    /// alternative is a window that wakes once a second for the life of the
    /// session to redraw a screen nothing is changing on.
    fn countdown(&mut self, ui: &mut Ui) {
        self.ticking = false;
        let errand = self.view.ran_out(std::time::Instant::now());
        let ran_out = !matches!(errand, crate::display::Errand::Nothing);
        if ran_out {
            // AND IT SAYS SO. This is the door's one feature the other nine
            // do not have, and the only observable it had was the file NOT
            // changing — an absence, which a door whose clock never armed
            // satisfies just as well. `tests/display-vm.nix` reads this.
            tracing::info!("the countdown ran out; putting the change back");
        }
        // THE SECONDS ON THE SCREEN COUNT DOWN whether or not it ran out, so
        // the frame is drawn either way: a question that says "15s" for
        // fifteen seconds is a question nobody believes.
        self.apply(ui, errand);
        // AND THE CURSOR SETTLES ON THE ROWS THAT COME BACK. `ran_out` calls
        // `answered`, which restores the remembered TARGET; only `View::key`
        // settles an INDEX, and nobody has pressed a key — that is the whole
        // situation this exists for. Without it the sheet came back with the
        // question's row 3 still in the cursor, lighting a fact four rows
        // above the knob the hand had turned, and the page did not move to it.
        //
        // THE ORDER IS NOT LOAD-BEARING: `ran_out` takes the question down
        // itself (`answered` does the `take`), so `rows()` yields the sheet
        // either way. The footer's twin settles before its errand and is
        // equally right.
        if ran_out {
            let rows = self.view_rows(ui);
            self.view.settle(&self.snapshot, &rows);
            ui.follow(self.view.cursor(), rows.len());
        }
        // NO RE-ARM HERE: `apply` is the one place that remembers, and it has
        // just run.
    }

    /// Puts one second on the wire, when a question is waiting for it.
    ///
    /// A CHANNEL AND A THREAD rather than a `calloop` timer, because the
    /// timer would have to be inserted and removed from inside its own
    /// callback — and the thread is what every other one-shot in this
    /// repository already is.
    fn arm_the_clock(&mut self) {
        if self.ticking
            || !self
                .view
                .asking()
                .is_some_and(crate::display::Asking::on_the_clock)
        {
            return;
        }
        let wire = self.tick.clone();
        // NAMED, and a failure to spawn is not a panic: `thread::spawn` would
        // bring the window down mid-question with the provisional change
        // standing, which is the one outcome the countdown exists to prevent.
        // Every other thread in this tree is a named `Builder` for the first
        // reason and this one is for the second.
        match std::thread::Builder::new()
            .name("display-countdown".into())
            .spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let _ = wire.send(());
            }) {
            Ok(_) => self.ticking = true,
            Err(e) => tracing::warn!(error = %e, "no clock; the question will wait for a key"),
        }
    }

    /// Goes away, after everything this door asked for has been carried out.
    ///
    /// # Why it waits, and why the guard is not "is a question standing"
    ///
    /// A preview lives in the compositor's RUNNING configuration, and the only
    /// thing that takes one down is this process's own thread asking for a
    /// re-read. Setting `quit` and returning dropped the sender and let `main`
    /// end, so a queued order was never carried out — and no door opened
    /// afterwards can help, because `previewed` is per-thread. The change stood
    /// until the compositor restarted.
    ///
    /// THE FIRST VERSION OF THIS GUARDED ON A STANDING QUESTION, and that is
    /// exactly the case that is already safe. The dangerous one is a question
    /// ANSWERED a moment ago: holding `Esc` out of a preview is three key
    /// repeats thirty milliseconds apart — answer, level, close — and the
    /// `Undo` the first one queued is hundreds of milliseconds of work. By the
    /// time `Close` arrives `asking` is `None` and the old guard skipped the
    /// wait. Answering "put it back" and then clicking the window's close
    /// button is the same race with a mouse.
    ///
    /// So it waits for whatever is outstanding, and only ADDS an `Undo` when a
    /// question is still standing — which is the one case where nothing has
    /// been queued yet.
    fn leave(&mut self, ui: &mut Ui) {
        if let Some(h) = self.screens.take() {
            let standing = self
                .view
                .asking()
                .is_some_and(crate::display::Asking::on_the_clock);
            if standing {
                tracing::info!("closing over a standing preview; putting it back first");
            }
            h.finish(standing.then_some(ronoctua_display::Order::Undo));
        }
        ui.quit = true;
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: crate::display::Errand) {
        // THE TABLE IS `display::doing`, and it is the same one the footer's
        // door reads — see it for why it is not written out twice.
        let doing = crate::display::doing(errand, &ui.texts);
        let closing = doing.close;
        self.doing(ui, doing);
        if closing {
            return;
        }
        // AND THE CLOCK IS ARMED IF A QUESTION WENT UP. Every gesture that
        // raises one comes through here, so this is the one place that has to
        // remember — see `arm_the_clock`.
        self.arm_the_clock();
        ui.ask_paint();
    }

    /// Carry out what the screens view asked of THIS door — through the one
    /// handler the footer's region carries it out with too
    /// (`display_region::carry_out`): a sentence on this window's line, an
    /// order to its reader or why there is none. Closing is this door's own
    /// leaving, which waits for what it sent (`leave`).
    fn doing(&mut self, ui: &mut Ui, doing: crate::display::Doing) {
        let close = doing.close;
        crate::display_region::carry_out(
            crate::display::Doing {
                close: false,
                ..doing
            },
            self.screens.as_ref().map(ronoctua_display::Handle::orders),
            ui,
        );
        if close {
            self.leave(ui);
        }
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::display::Press) {
        ui.message = None;
        // THE WIDTH THIS WINDOW DRAWS AT, so the key acts on the row on
        // screen — see `display::View::key`.
        // A HELD KEY WALKS THE ROW AND WRITES NOTHING (`View::hold`): the
        // write is owed to the key coming up, in `released`.
        self.view.hold(ui.repeating);
        let errand = self.view.key(
            press,
            &self.snapshot,
            ui.columns,
            std::time::Instant::now(),
            &ui.texts,
        );
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::display::Row> {
        self.view.rows(
            &self.snapshot,
            ui.columns,
            std::time::Instant::now(),
            &ui.texts,
        )
    }

    /// A monitor arrived or left, and somebody other than this door said so.
    ///
    /// `ronoctua_display::watch` reads again only when an order arrives, so
    /// without a signal the window never saw a second monitor appear, and kept
    /// drawing a row for one that had been unplugged: `RET` on it opened a
    /// sheet whose only content was that the screen was gone. The
    /// compositor's own `output` event is the signal, and `display.json`
    /// changing under this window is the other.
    ///
    /// SHALLOW, for the reason the footer gives: asking every monitor over
    /// DDC/CI on a hotplug is a fifth of a second per port.
    fn look_again(&self) {
        if let Some(h) = self.screens.as_ref() {
            h.send(ronoctua_display::Order::Look { deep: false });
        }
    }
}

impl Content for Display {
    /// A SCREEN PLUGGED IN OR TAKEN AWAY re-reads the list — and NOT a
    /// screen that merely changed (`update_output`), which is the event this
    /// view's own orders produce: every `Try`, `Keep` and `Undo` moves an
    /// output, and a whole reading per batch would be several walks of
    /// `/sys/class/drm` per keypress.
    fn outputs_changed(&mut self, _: &mut Ui) {
        self.look_again();
    }

    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::DISPLAY_TITLE))
    }

    /// A key, through the reader the footer calls too
    /// (`display_words::press_of_key`).
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::display_words::press_of_key)
        {
            self.pressed(ui, press);
        }
    }

    /// THE HELD KEY CAME UP: a place walked to under it is written, once.
    fn released(&mut self, ui: &mut Ui, _: &KeyEvent) {
        let errand = self.view.let_go(&self.snapshot);
        self.apply(ui, errand);
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(
            &self.snapshot,
            ui.columns,
            std::time::Instant::now(),
            &ui.texts,
        )
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    /// A click takes a row the cursor stands on, and only that row lights
    /// under the pointer — the view's one answer
    /// ([`crate::display::Row::takes`]), which the footer's region gives too.
    fn takes(&self, ui: &Ui) -> Vec<bool> {
        self.view_rows(ui)
            .iter()
            .map(crate::display::Row::takes)
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
        if self.view.clicked(row, &rows) {
            self.pressed(ui, crate::display::Press::Enter);
        }
    }

    /// The compositor asked this window to go away.
    ///
    /// A STANDING PREVIEW GOES BACK FIRST, and it took the final gate to find
    /// this. A change that can blind a screen is applied over the IPC and held
    /// up by a question; the thing that takes it down is this process's own
    /// thread asking for a re-read (`Order::Undo`). Setting `quit` and
    /// returning dropped the sender and let `main` end, so the queued order
    /// was never carried out — and a door opened afterwards cannot help,
    /// because `previewed` is per-thread and a fresh one's is empty.
    ///
    /// `Handle::finish` is what waits; the door cannot, because the answer
    /// comes back through a `calloop` source this function is inside of.
    fn closing(&mut self, ui: &mut Ui) -> bool {
        self.leave(ui);
        true
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        crate::door::list::paint(self, ui, canvas);
        // AND A FRAME THAT WENT OUT SAYS SO. The title and the `app_id` are
        // set before the event loop, so the compositor lists this window
        // whether or not it ever drew — every assertion a VM could make about
        // it was an ABSENCE (no error sentence), and absences pass over a
        // `paint` whose body is deleted. `buffer.rs` logs the same line for
        // the same reason, and `tests/display-vm.nix` reads it.
        // AT DEBUG, which is where `toolkits.rs` put the decision: "a line per
        // keypress in the compositor's journal is noise".
        tracing::debug!(
            width = ui.width,
            height = ui.height,
            columns = ui.columns,
            rows = self.view_rows(ui).len(),
            "the screens window drew"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(all: &[&str]) -> Vec<String> {
        all.iter().map(|w| (*w).to_owned()).collect()
    }

    /// The window as its camera opens it, over two planted screens
    /// (`photo::scene`): no reader thread and no watch, nothing of this
    /// machine read.
    fn a_window() -> (Display, Ui) {
        let snapshot = crate::photo::scene("two").expect("the scene of two screens");
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        (photographed(snapshot), Ui::for_tests(texts))
    }

    /// FAILS IF: a word of the screens' camera is lost, a walk is not taken,
    /// another door is taken for this one, or a reading of `display.json`
    /// other than the three is let through.
    #[test]
    fn the_cameras_words_are_the_walk_and_the_screens() {
        use crate::photo::scenes::parse;
        let asked = parse(
            &words(&[
                "--door",
                "display",
                "--screens",
                "dark",
                "--key",
                "S-<up>",
                "--columns",
                "100",
            ]),
            &mut Camera::default(),
        )
        .expect("the words");
        assert_eq!(asked.door, "display");
        assert_eq!(asked.steps.len(), 1);
        assert!(parse(&words(&["--door", "files"]), &mut Camera::default()).is_err());
        // HOW `display.json` READ is one of three.
        let mut camera = Camera::default();
        parse(
            &words(&["--door", "display", "--settings", "repaired"]),
            &mut camera,
        )
        .expect("the words");
        assert_eq!(camera.settings, "repaired");
        assert!(
            parse(
                &words(&["--door", "display", "--settings", "half"]),
                &mut Camera::default()
            )
            .is_err()
        );
    }

    /// FAILS IF: the window's resting pointer lights a row a click takes
    /// nothing on — a title, the air between sections, a sentence — or
    /// leaves unlit one it takes. THE LIGHT IS THE CLICK: every row clicked
    /// once, on a window of its own, and lit exactly where something
    /// happened. The footer's region has its own.
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
