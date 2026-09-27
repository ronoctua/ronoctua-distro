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
    reexports::calloop::{self, channel},
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
                    door.content.screens_changed();
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
                    door.content.screens_changed();
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
            handle: w.handle.clone(),
            view,
            snapshot: ronoctua_display::Snapshot::default(),
            screens,
            tick: tick_sender,
            ticking: false,
            said: 0,
            _watch: watch,
        })
    })
}

/// The window: the view, and what it reads.
struct Display {
    /// The loop, for the one-shot timer that takes a message off the line —
    /// see [`Display::say`].
    handle: calloop::LoopHandle<'static, Door<Self>>,
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
    /// How many messages have been said, so a timer only clears its own.
    ///
    /// The footer's line fades after `ECHO_SECONDS` and this one did not: it
    /// sat until the next keypress, which on a window somebody is reading
    /// rather than typing in is for ever. `files_window::say` carries the same
    /// pair and the owner's report that produced it.
    said: u64,
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

    /// Carry out what the screens view asked of THIS door.
    ///
    /// The part that is the window's and not the view's: a sentence goes to
    /// its own message line, and closing means leaving.
    fn doing(&mut self, ui: &mut Ui, doing: crate::display::Doing) {
        if let Some(said) = doing.said {
            self.say(ui, said);
        }
        if let Some(order) = doing.order {
            self.order(ui, order);
        }
        if doing.close {
            self.leave(ui);
        }
    }

    /// Hands one order to the reading thread, or says there is none.
    fn order(&mut self, ui: &mut Ui, order: ronoctua_display::Order) {
        match &self.screens {
            Some(h) => h.send(order),
            None => {
                let said = ui.texts.t(ronoctua_i18n::key::DISPLAY_NO_THREAD);
                self.say(ui, said);
            }
        }
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::display::Press) {
        ui.message = None;
        // THE WIDTH THIS WINDOW DRAWS AT, so the key acts on the row on
        // screen — see `display::View::key`.
        let errand = self.view.key(
            press,
            &self.snapshot,
            ui.columns,
            std::time::Instant::now(),
            &ui.texts,
        );
        self.apply(ui, errand);
    }

    /// Puts a sentence on the message line, and takes it off again.
    ///
    /// **THE ONE PLACE** a message is set in this door, which is the shape the
    /// file manager's window arrived at the hard way: nine assignments there
    /// meant nine chances to forget what a message needs, and all nine had
    /// forgotten the same thing — the message never went away. The owner found
    /// it with `copy the path`, where the footer's `copied` fades after three
    /// seconds and that door's stayed for the rest of the session.
    ///
    /// # Why a one-shot timer, and not a clock
    ///
    /// Because the clock this window HAS is armed only while a countdown runs
    /// (see the module header), and a message can outlive one — or arrive with
    /// no question at all. A clock kept for the message would be a wake-up per
    /// second for the life of the window, which is the property the countdown
    /// is arranged to avoid. A one-shot keeps it exactly — a window with
    /// nothing to say wakes not once — and pays for the three seconds after a
    /// message, which is the only time there is anything to do.
    ///
    /// The same three seconds as the footer, from the same constant: a message
    /// that lingered here and not there would be two answers to "how long is a
    /// message on screen".
    fn say(&mut self, ui: &mut Ui, text: String) {
        tracing::info!(echo = %text, "message line");
        ui.message = Some(text);
        self.said = self.said.wrapping_add(1);
        let mine = self.said;
        let armed = self.handle.insert_source(
            calloop::timer::Timer::from_duration(std::time::Duration::from_secs(
                crate::surface::ECHO_SECONDS,
            )),
            move |_, (), door: &mut Door<Self>| {
                // ONLY ITS OWN. Between arming and firing the line may have
                // been replaced or cleared by a key; wiping whatever is there
                // would take a newer sentence down early.
                if door.content.said == mine {
                    door.ui.message = None;
                    door.ui.ask_paint();
                }
                calloop::timer::TimeoutAction::Drop
            },
        );
        if let Err(e) = armed {
            // NOT SILENT, and the message STAYS. A line that outstays its
            // welcome is a smaller failure than one that vanishes because a
            // timer could not be hung, and the log says which happened.
            tracing::warn!(error = %e, "no timer for the message line; it will stay");
        }
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
    fn screens_changed(&self) {
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
        self.screens_changed();
    }

    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::DISPLAY_TITLE))
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

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(
            &self.snapshot,
            ui.columns,
            std::time::Instant::now(),
            &ui.texts,
        )
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
