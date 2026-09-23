//! Mice, touchpads and cursors — the pointer settings and the seat every
//! pointing device is on.
//!
//! A thread in the shape of `ronoctua-display`'s: it holds a connection to
//! the compositor, carries out [`Order`]s, and says what it sees as
//! [`Event`]s. Two doors run one each — the footer and the window — and only
//! the footer's DETECTS: it is the one alive for the whole session, and it is
//! the one that gives a device plugged in with no view open a cursor of its
//! own.
//!
//! # What an order does, in this order
//!
//! 1. Read `pointers.json` from disk — never a copy held since the last
//!    order, which is how two doors avoid writing each other's work back
//!    (see [`prefs`]).
//! 2. Change the one thing the order names, and plan the seats
//!    ([`plan::plan`]).
//! 3. Write the file and the fragment the compositor includes
//!    ([`fragment`]), each only when its text changed.
//! 4. Ask the compositor for what changed, live — or re-read its
//!    configuration, when a device has to LEAVE a seat, because nothing else
//!    takes one off.

#![forbid(unsafe_code)]

pub mod fragment;
pub mod keyboards;
pub mod plan;
pub mod prefs;
pub mod typing;

use std::sync::mpsc;

use anyhow::{Context as _, Result};
use ronoctua_compositor::{Compositor, Device, DeviceKind, FocusFollows, Seat, SeatAsk, Setting};

pub use plan::{CursorPlan, DevicePlan, Plan};
pub use prefs::{COLOURS, FIRST, Preferences};
pub use typing::Typing;

/// What the thread sees.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    /// Every input device the compositor reports.
    pub devices: Vec<Device>,
    /// Every seat.
    pub seats: Vec<Seat>,
    /// What was chosen.
    pub prefs: Preferences,
    /// What the choices make of the devices present.
    pub plan: Plan,
    /// Whether `pointers.json` could not be read — then nothing is written
    /// over it, and the screen says why its choices are missing.
    pub settings_failed: bool,
    /// Whether there is a compositor to ask at all.
    pub connected: bool,
    /// Every keyboard's twin and its cursor — empty in Together mode.
    pub keyboards: Vec<keyboards::KeyboardPlan>,
}

/// Where a device should go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// An existing cursor, by number.
    Cursor(u16),
    /// A cursor of its own, numbered after every one there is.
    New,
}

/// What can be asked of the thread.
#[derive(Debug, Clone, PartialEq)]
pub enum Order {
    /// Read everything again.
    Look,
    /// Whether new devices get a cursor of their own.
    Detect(bool),
    /// Switch one device on or off.
    DeviceOn(String, bool),
    /// Put one device on a cursor.
    DevicePlace(String, Place),
    /// One setting of one device.
    DeviceSetting(String, Setting),
    /// One setting for every device of a kind.
    KindSetting(DeviceKind, Setting),
    /// Switch one cursor, and every device on it, on or off.
    CursorOn(u16, bool),
    /// Paint one cursor with another of the [`COLOURS`].
    CursorColour(u16, u8),
    /// The size of every cursor.
    Size(u16),
    /// Hide a still cursor after this many milliseconds; `0` never.
    HideAfter(u32),
    /// Hide the cursor while typing.
    HideWhileTyping(bool),
    /// Whether focus follows the pointer.
    Focus(FocusFollows),
    /// Which cursor the keyboards type through.
    Typing(Typing),
    /// Forget a device that is not plugged in.
    Forget(String),
}

/// What the thread says.
#[derive(Debug, Clone)]
pub enum Event {
    /// A fresh reading. It follows every order.
    Snapshot(Box<Snapshot>),
    /// An order finished — with why not, when it did not.
    Done {
        /// What was asked.
        order: Order,
        /// The refusal's words.
        refused: Option<String>,
    },
}

/// The way to send orders to the thread.
#[derive(Debug)]
pub struct Handle {
    orders: mpsc::Sender<Order>,
}

impl Handle {
    /// Sends one. A thread that has gone away is a warning, not a panic.
    pub fn send(&self, order: Order) {
        if self.orders.send(order).is_err() {
            tracing::warn!("the pointer thread is gone; the order was dropped");
        }
    }
}

/// Opens a connection, or says there is nobody to talk to.
///
/// THE ONE PLACE IN THIS CRATE THAT NAMES AN ADAPTER —
/// `only-the-composition-root-names-the-compositor` counts it.
fn open_the_compositor() -> Result<Box<dyn Compositor>> {
    ronoctua_scroll::Scroll::open()
}

/// Starts the thread.
///
/// `detects`: whether this door places devices nobody placed. The footer
/// does; a window does not, so two doors never race to number one mouse.
///
/// # Errors
///
/// The thread could not be started.
pub fn watch<F>(detects: bool, on_event: F) -> Result<Handle>
where
    F: Fn(Event) + Send + Sync + 'static,
{
    watch_with(detects, open_the_compositor, prefs::store(), on_event)
}

/// The same, with the compositor and the file given — what a test drives.
///
/// # Errors
///
/// The thread could not be started.
pub fn watch_with<F>(
    detects: bool,
    open: fn() -> Result<Box<dyn Compositor>>,
    store: ronoctua_store::Store<Preferences>,
    on_event: F,
) -> Result<Handle>
where
    F: Fn(Event) + Send + Sync + 'static,
{
    let (orders, inbox) = mpsc::channel();
    // THE EVENTS COME IN ON A SECOND THREAD and become `Look`s on the same
    // queue as everything else, so a device arriving is handled in order
    // with the orders and never beside one.
    let looks = orders.clone();
    // ONLY THE DOOR THAT DETECTS moves typing: two doors answering the same
    // focus event would ask the same focus twice.
    let typing_store = detects.then(|| store.clone());
    std::thread::Builder::new()
        .name("pointer-events".to_owned())
        .spawn(move || listen(open, &looks, typing_store.as_ref()))
        .context("could not start the pointer event thread")?;
    std::thread::Builder::new()
        .name("pointers".to_owned())
        .spawn(move || {
            let mut hands = Hands {
                wm: open().ok(),
                open,
                store,
                detects,
                fragment_said: None,
                keys_said: None,
            };
            let first = hands.carry(&Order::Look);
            on_event(Event::Snapshot(Box::new(first.0)));
            while let Ok(order) = inbox.recv() {
                // A BURST OF DEVICE EVENTS IS ONE LOOK: a dongle announces a
                // mouse, a keyboard and a consumer-control device at once,
                // and three plans in a row would attach the mouse three times.
                if order == Order::Look {
                    let mut others = Vec::new();
                    while let Ok(next) = inbox.try_recv() {
                        if next != Order::Look {
                            others.push(next);
                        }
                    }
                    for next in others {
                        let (snap, refused) = hands.carry(&next);
                        on_event(Event::Snapshot(Box::new(snap)));
                        on_event(Event::Done {
                            order: next,
                            refused,
                        });
                    }
                }
                let (snap, refused) = hands.carry(&order);
                on_event(Event::Snapshot(Box::new(snap)));
                if order != Order::Look {
                    on_event(Event::Done { order, refused });
                }
            }
        })
        .context("could not start the pointer thread")?;
    Ok(Handle { orders })
}

/// Turns the compositor's device events into `Look`s, reconnecting when the
/// compositor goes away and comes back.
fn listen(
    open: fn() -> Result<Box<dyn Compositor>>,
    looks: &mpsc::Sender<Order>,
    typing: Option<&ronoctua_store::Store<Preferences>>,
) {
    // A SECOND CONNECTION for the questions a focus event raises: the first
    // one is the event stream, and a stream is not asked anything.
    let mut asks: Option<Box<dyn Compositor>> = None;
    let mut before = typing::Focuses::new();
    loop {
        let stream = open().and_then(|c| c.watch());
        let Ok(mut events) = stream else {
            std::thread::sleep(std::time::Duration::from_secs(2));
            continue;
        };
        loop {
            match events.next_one() {
                Ok(Some(ronoctua_compositor::Event::DevicesChanged)) => {
                    if looks.send(Order::Look).is_err() {
                        return;
                    }
                }
                Ok(Some(ronoctua_compositor::Event::FocusChanged(_))) => {
                    if let Some(store) = typing {
                        follow_typing(open, &mut asks, &mut before, store);
                    }
                }
                Ok(Some(ronoctua_compositor::Event::Leaving) | None) | Err(_) => break,
                Ok(Some(_)) => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// Moves typing to where the followed cursor just focused (see [`typing`]).
///
/// The file is read on every focus event, and that is the cost of "each
/// order reads the file": the setting changes in another thread, and a copy
/// kept here would be a second place for it. With typing on the first
/// cursor, which is the default, nothing else is asked.
fn follow_typing(
    open: fn() -> Result<Box<dyn Compositor>>,
    asks: &mut Option<Box<dyn Compositor>>,
    before: &mut typing::Focuses,
    store: &ronoctua_store::Store<Preferences>,
) {
    let chosen = store.read().typing;
    // IN INDEPENDENT MODE EVERY KEYBOARD IS ON ITS OWN CURSOR'S SEAT, and
    // types where that cursor chose without anything asked of the
    // compositor: moving `seat0`'s focus there would move a keyboard nobody
    // pressed.
    let independent =
        ronoctua_keyboards::store().read().mode == ronoctua_keyboards::Mode::Independent;
    if chosen == Typing::First || independent {
        before.clear();
        return;
    }
    if asks.is_none() {
        *asks = open().ok();
    }
    let Some(wm) = asks.as_mut() else {
        return;
    };
    let Ok(seats) = wm.seats() else {
        // A DEAD SOCKET: the next event reconnects.
        *asks = None;
        return;
    };
    if let Some(window) = typing::goes_to(chosen, before, &seats)
        && let Err(e) = wm.focus_window(window)
    {
        tracing::warn!(error = %e, window, "could not move typing to the window");
        *asks = None;
    }
    *before = typing::focuses(&seats);
}

/// The thread's own state — a connection and what it last wrote, never the
/// preferences (see the module's first point).
struct Hands {
    wm: Option<Box<dyn Compositor>>,
    open: fn() -> Result<Box<dyn Compositor>>,
    store: ronoctua_store::Store<Preferences>,
    detects: bool,
    /// The fragment's text as last written, so an unchanged one is not
    /// written again.
    fragment_said: Option<String>,
    /// The keyboards' part of it, as last written.
    keys_said: Option<String>,
}

impl Hands {
    fn wm(&mut self) -> Option<&mut Box<dyn Compositor>> {
        if self.wm.is_none() {
            self.wm = (self.open)().ok();
        }
        self.wm.as_mut()
    }

    /// Carries one order out, and reads what came of it.
    fn carry(&mut self, order: &Order) -> (Snapshot, Option<String>) {
        let refused = self.obey(order).err().map(|e| format!("{e:#}"));
        if refused.is_some() {
            // A FAILED CALL MAY BE A DEAD SOCKET: the next order reconnects.
            self.wm = None;
        }
        (self.look(), refused)
    }

    fn look(&mut self) -> Snapshot {
        // `parsed` answers `None` only for a file that is there and will not
        // parse — a missing one is the default, which is a fresh machine.
        let (prefs, settings_failed) = match self.store.parsed() {
            Some(p) => (p, false),
            None => (Preferences::default(), true),
        };
        let (devices, seats, connected) = match self.wm() {
            Some(wm) => (
                wm.devices().unwrap_or_default(),
                wm.seats().unwrap_or_default(),
                true,
            ),
            None => (Vec::new(), Vec::new(), false),
        };
        let plan = plan::plan(&devices, &prefs);
        let keyboards = keyboards_of(&devices, &plan);
        Snapshot {
            devices,
            seats,
            prefs,
            plan,
            settings_failed,
            connected,
            keyboards,
        }
    }

    fn obey(&mut self, order: &Order) -> Result<()> {
        let now = self.look();
        if now.settings_failed && *order != Order::Look {
            anyhow::bail!(
                "{} could not be read, and is left as it is rather than overwritten",
                self.store
                    .path()
                    .map_or_else(|| prefs::FILE.to_owned(), |p| p.display().to_string())
            );
        }
        let mut chosen = changed(now.prefs.clone(), order, &now.plan);
        let mut plan = plan::plan(&now.devices, &chosen);
        if self.detects || *order != Order::Look {
            chosen = plan::remembered(chosen, &plan);
            plan = plan::plan(&now.devices, &chosen);
        }
        if chosen != now.prefs && !now.settings_failed {
            self.store
                .write(&chosen)
                .context("writing the pointer settings")?;
        }
        let keys = keyboards_of(&now.devices, &plan);
        let keys_text = fragment::keyboards_text(&keys)?;
        let text = fragment::text_of(&plan, &chosen)? + &keys_text;
        // A KEYBOARD'S SETTINGS HAVE NO LIVE ASK: layout and repeat reach a
        // device by the file, read again. Only when they changed.
        let keys_changed = self.keys_said.as_deref() != Some(keys_text.as_str());
        self.keys_said = Some(keys_text);
        let fragment_changed = self.fragment_said.as_deref() != Some(text.as_str());
        if fragment_changed {
            write_fragment(&text)?;
            self.fragment_said = Some(text);
        }
        let Some(wm) = self.wm() else {
            return if *order == Order::Look {
                Ok(())
            } else {
                anyhow::bail!("there is no compositor to ask")
            };
        };
        apply(
            wm.as_mut(),
            &now.seats,
            &plan,
            &chosen,
            fragment_changed,
            &keys,
            keys_changed,
        )
    }
}

/// The keyboards placed, from the keyboards' own file and the units the kernel
/// lists now.
fn keyboards_of(devices: &[Device], plan: &Plan) -> Vec<keyboards::KeyboardPlan> {
    let chosen = ronoctua_keyboards::store().read();
    if chosen.mode != ronoctua_keyboards::Mode::Independent {
        return Vec::new();
    }
    let units = ronoctua_keyboards::units().unwrap_or_default();
    keyboards::plan(
        &chosen,
        &units,
        devices,
        plan,
        &keyboards::Machine::from_env(),
    )
}

/// The preferences with one order carried out.
fn changed(mut p: Preferences, order: &Order, plan: &Plan) -> Preferences {
    match order {
        Order::Look => {}
        Order::Detect(on) => p.detect = *on,
        Order::DeviceOn(id, on) => p.devices.entry(id.clone()).or_default().enabled = *on,
        Order::DevicePlace(id, place) => {
            let n = match place {
                Place::Cursor(n) => *n,
                Place::New => {
                    let highest = plan
                        .cursors
                        .iter()
                        .map(|c| c.number)
                        .chain(p.devices.values().filter_map(|d| d.cursor))
                        .max()
                        .unwrap_or(FIRST);
                    highest + 1
                }
            };
            p.devices.entry(id.clone()).or_default().cursor = Some(n);
        }
        Order::DeviceSetting(id, s) => p.devices.entry(id.clone()).or_default().settings.set(*s),
        Order::KindSetting(kind, s) => match kind {
            DeviceKind::Touchpad => p.touchpads.set(*s),
            _ => p.mice.set(*s),
        },
        Order::CursorOn(n, on) => {
            let mut c = p.cursor(*n);
            c.enabled = *on;
            p.cursors.insert(*n, c);
        }
        Order::CursorColour(n, colour) => {
            let mut c = p.cursor(*n);
            c.colour = *colour % COLOURS;
            p.cursors.insert(*n, c);
        }
        // THE NEAREST SIZE THE THEMES CARRY, never one between.
        Order::Size(s) => {
            p.size = prefs::SIZES
                .iter()
                .copied()
                .min_by_key(|x| x.abs_diff(*s))
                .unwrap_or(prefs::SIZE);
        }
        Order::HideAfter(ms) => p.hide_after_ms = *ms,
        Order::HideWhileTyping(b) => p.hide_while_typing = *b,
        Order::Focus(f) => p.focus = *f,
        Order::Typing(t) => p.typing = *t,
        Order::Forget(id) => {
            p.devices.remove(id);
        }
    }
    p
}

/// Makes the compositor match the plan.
///
/// Live where it can be — a device moved to a seat of ours, a setting, a
/// cursor's theme — and by a re-read where it cannot: taking a device off a
/// seat, which only a re-read of the file (now without that seat) does.
fn apply(
    wm: &mut dyn Compositor,
    seats: &[Seat],
    plan: &Plan,
    prefs: &Preferences,
    fragment_changed: bool,
    keys: &[keyboards::KeyboardPlan],
    keys_changed: bool,
) -> Result<()> {
    let seat_now = |id: &str| {
        seats
            .iter()
            .find(|s| s.devices.iter().any(|d| d == id))
            .map(|s| s.name.clone())
    };
    let mut must_reread = false;
    for c in &plan.cursors {
        for id in &c.devices {
            let here = seat_now(id);
            if here.as_deref() == Some(c.seat.as_str()) {
                continue;
            }
            if c.number == FIRST {
                // BACK TO THE FIRST SEAT: nothing detaches, so the file (in
                // which this device is no longer attached anywhere) is read
                // again and the fallback takes it.
                must_reread = true;
            } else {
                wm.ask_seat(&c.seat, SeatAsk::Attach(id.clone()))?;
            }
        }
    }
    // THE KEYBOARDS' TWINS, the same way: attached live to a seat of ours,
    // back to the first seat by a re-read. A twin's settings (layout, repeat)
    // have no live ask, so a change in them is a re-read too.
    for k in keys {
        let seat = plan::seat_of(k.cursor);
        if seat_now(&k.twin).as_deref() == Some(seat.as_str()) {
            continue;
        }
        if k.cursor == FIRST {
            must_reread |= seat_now(&k.twin).is_some_and(|s| s != seat);
        } else {
            wm.ask_seat(&seat, SeatAsk::Attach(k.twin.clone()))?;
        }
    }
    must_reread |= keys_changed && !keys.is_empty();
    // A SEAT OF OURS WITH NOTHING ON IT is one the file no longer names, and
    // only a re-read removes it — or a cursor would be left on the screen
    // that no hand moves.
    if seats.iter().any(|s| {
        s.name.starts_with("ronoctua-cursor-")
            && !plan.cursors.iter().any(|c| c.seat == s.name)
            && !keys.iter().any(|k| plan::seat_of(k.cursor) == s.name)
    }) {
        must_reread = true;
    }
    // THE FILE WAS WRITTEN BEFORE THIS POINT whether or not it changed, so a
    // re-read reads the plan — `fragment_changed` only says whether there was
    // anything new in it, which a re-read does not need.
    let _ = fragment_changed;
    if must_reread {
        return wm.reread().context("re-reading the configuration");
    }
    for c in &plan.cursors {
        wm.ask_seat(
            &c.seat,
            SeatAsk::Cursor {
                theme: fragment::theme_name(c.colour),
                size: prefs.size,
            },
        )?;
    }
    wm.ask_seat("*", SeatAsk::HideAfter(prefs.hide_after_ms))?;
    wm.ask_seat("*", SeatAsk::HideWhileTyping(prefs.hide_while_typing))?;
    wm.ask_focus(prefs.focus)?;
    for d in &plan.devices {
        for s in d.to_ask() {
            wm.ask_input(&d.id, s)?;
        }
    }
    Ok(())
}

/// Writes the fragment, atomically.
fn write_fragment(text: &str) -> Result<()> {
    let Some(dir) = ronoctua_store::config_dir() else {
        anyhow::bail!("there is no config directory to write the cursor settings to");
    };
    ronoctua_store::write_atomically(&dir, fragment::FILE, text.as_bytes())
        .with_context(|| format!("writing {}", dir.join(fragment::FILE).display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_compositor::{PointerSettings, Said, Scripted, SendEvents};

    fn mouse(id: &str) -> Device {
        Device {
            id: id.to_owned(),
            name: id.to_owned(),
            kind: DeviceKind::Mouse,
            settings: PointerSettings {
                send_events: Some(SendEvents::Enabled),
                natural_scroll: Some(false),
                ..PointerSettings::default()
            },
        }
    }

    fn two_mice() -> (Vec<Device>, Vec<Seat>) {
        let devices = vec![mouse("1:1:One"), mouse("2:2:Two")];
        let seats = vec![Seat {
            name: "seat0".into(),
            devices: vec!["1:1:One".into(), "2:2:Two".into()],
            focus: None,
        }];
        (devices, seats)
    }

    #[test]
    fn a_new_cursor_is_attached_live_and_themed() {
        let (devices, seats) = two_mice();
        let prefs = Preferences {
            detect: true,
            ..Preferences::default()
        };
        let plan = plan::remembered(prefs.clone(), &plan::plan(&devices, &prefs));
        let p = plan::plan(&devices, &plan);
        let mut wm = Scripted {
            devices,
            seats: seats.clone(),
            ..Scripted::default()
        };
        apply(&mut wm, &seats, &p, &plan, true, &[], false).unwrap();
        assert!(wm.told.contains(&Said::Seat(
            "ronoctua-cursor-2".into(),
            SeatAsk::Attach("2:2:Two".into())
        )));
        assert!(wm.told.contains(&Said::Seat(
            "ronoctua-cursor-2".into(),
            SeatAsk::Cursor {
                theme: "ronoctua-pointer-2".into(),
                size: prefs::SIZE
            }
        )));
        assert!(
            !wm.told.contains(&Said::Reread),
            "nothing had to leave a seat"
        );
    }

    #[test]
    fn a_device_given_back_to_the_first_cursor_needs_a_reread() {
        let (devices, _) = two_mice();
        let seats = vec![
            Seat {
                name: "seat0".into(),
                devices: vec!["1:1:One".into()],
                focus: None,
            },
            Seat {
                name: "ronoctua-cursor-2".into(),
                devices: vec!["2:2:Two".into()],
                focus: None,
            },
        ];
        let mut prefs = Preferences::default();
        prefs.devices.entry("1:1:One".into()).or_default().cursor = Some(1);
        prefs.devices.entry("2:2:Two".into()).or_default().cursor = Some(1);
        let p = plan::plan(&devices, &prefs);
        let mut wm = Scripted::default();
        apply(&mut wm, &seats, &p, &prefs, true, &[], false).unwrap();
        assert_eq!(wm.told, vec![Said::Reread]);
    }

    #[test]
    fn an_order_changes_one_thing() {
        let p = Preferences::default();
        let plan = Plan::default();
        let off = changed(p.clone(), &Order::CursorOn(3, false), &plan);
        assert!(!off.cursor(3).enabled);
        assert_eq!(
            off.cursor(3).colour,
            prefs::born_colour(3),
            "and keeps its colour"
        );
        let coloured = changed(p.clone(), &Order::CursorColour(2, 11), &plan);
        assert_eq!(
            coloured.cursor(2).colour,
            3,
            "a colour goes round the palette"
        );
        let tap = changed(
            p,
            &Order::KindSetting(DeviceKind::Touchpad, Setting::Tap(false)),
            &plan,
        );
        assert_eq!(tap.touchpads.tap, Some(false));
    }

    #[test]
    fn a_device_sent_to_a_new_cursor_gets_the_next_number() {
        let (devices, _) = two_mice();
        let detecting = Preferences {
            detect: true,
            ..Preferences::default()
        };
        let prefs = plan::remembered(detecting.clone(), &plan::plan(&devices, &detecting));
        let p = plan::plan(&devices, &prefs);
        let moved = changed(prefs, &Order::DevicePlace("1:1:One".into(), Place::New), &p);
        assert_eq!(moved.devices["1:1:One"].cursor, Some(3));
    }
}
