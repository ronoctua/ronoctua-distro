//! The one boundary with the screens — section 4.2.
//!
//! Three sources, one reading. The COMPOSITOR says what a monitor can do and
//! what it is doing (through `ronoctua-niri`, the only crate allowed to speak
//! its protocol); the KERNEL says how bright a built-in panel is; and the
//! MONITOR ITSELF answers, over DDC/CI, when it is one on a cable.
//!
//! What joins them is the connector's name: niri's `eDP-1` is the kernel's
//! `card1-eDP-1`, and inside that directory are both the backlight and the
//! I2C adapter. See [`connector`].
//!
//! # Two halves of remembering, and why BOTH have to be said
//!
//! A change made over niri's IPC is **transient**: it lives in the running
//! configuration and does not reach any file. So [`Order::Try`] is the
//! preview, and [`Order::Keep`] writes [`fragment`] — the file niri includes
//! and watches — which is what makes it survive a restart.
//!
//! THE RELOAD DOES NOT UNDO THE PREVIEW, and believing it did was the first
//! version of this crate. niri drops transient output changes only when the
//! configuration it re-reads DIFFERS from the last disk load; when the file
//! it parses is unchanged it takes the other branch and puts the transient
//! ones back (`src/niri.rs`: "Output config did not change from the last disk
//! load, so we need to preserve the transient changes"). Cancelling by
//! rewriting the same bytes therefore preserved exactly what it meant to
//! discard, for ever.
//!
//! So the rule here is blunter and does not depend on that logic at all:
//! after writing the file, this crate SAYS OUT LOUD, over the IPC, what the
//! file now means — every knob, including the ones nobody set, because "not
//! chosen" is the instruction `Automatic` and not the absence of one. The
//! file is what survives a restart; the IPC is what is true right now; and
//! [`Order::Undo`] is the second of those alone.
//!
//! # `Ok` is not "it happened"
//!
//! niri answers `Applied` to a mode the monitor does not have, and falls back
//! to the preferred one with a warning nobody reads. So every order here is
//! followed by a fresh [`Snapshot`], and what confirms a change is the
//! reading, never the answer.

pub mod backlight;
pub mod connector;
pub mod ddc;
pub mod fragment;
pub mod prefs;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc;

use anyhow::{Context as _, Result};
use ronoctua_niri::{Ask, Logical, Mode, ModeAsked, Naming, Niri, Output, Transform};

use fragment::Wanted;
use prefs::Preferences;

/// How a screen's brightness can be moved, and where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Light {
    /// A backlight in `/sys` — a built-in panel.
    Panel(backlight::Panel),
    /// The monitor's own control, reached over DDC/CI. In percent.
    Monitor(u8),
}

impl Light {
    /// The brightness a person reads.
    #[must_use]
    pub fn percent(&self) -> u8 {
        match self {
            Self::Panel(p) => p.percent(),
            Self::Monitor(p) => *p,
        }
    }

    /// Whether it can actually be moved from here.
    ///
    /// A panel whose udev rule is missing reads fine and refuses every write,
    /// and the row has to say so BEFORE somebody presses the key.
    #[must_use]
    pub const fn movable(&self) -> bool {
        match self {
            Self::Panel(p) => p.writable,
            Self::Monitor(_) => true,
        }
    }
}

/// One screen, joined from everything that knows something about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    /// The connector, which is niri's name for it.
    pub at: String,
    /// The name its settings are filed under, and why it is that one.
    ///
    /// It is [`ronoctua_niri::Output::naming`]: the monitor's description
    /// when that can only be this monitor, and the connector otherwise.
    pub naming: Naming,
    /// `"<make> <model> <serial>"`, always — for SHOWING, and every part of
    /// it may be the word `Unknown`, which is itself the truth about a
    /// monitor that says nothing. It is not the filing key: a monitor with no
    /// serial shares this string with every other unit of its model.
    pub description: String,
    /// Who made it, or the word `Unknown`.
    pub make: String,
    /// What it is called, or the word `Unknown`.
    pub model: String,
    /// Its serial, when the EDID carries one.
    pub serial: Option<String>,
    /// The glass, in millimetres.
    pub millimetres: Option<(u32, u32)>,
    /// Every mode it offers.
    pub modes: Vec<Mode>,
    /// Which of them is running. See [`ronoctua_niri::Output::current_mode`]
    /// — `None` does not mean dark.
    pub current: Option<usize>,
    /// Where it sits and at what scale. `None` while it is off.
    pub logical: Option<Logical>,
    /// Whether the hardware can vary its refresh rate.
    pub vrr_supported: bool,
    /// Whether it is varying it now.
    pub vrr_enabled: bool,
    /// Whether it is built into the machine.
    pub internal: bool,
    /// Its brightness, when anything here can move it.
    pub light: Option<Light>,
    /// Where the kernel keeps the two things niri cannot answer.
    pub wiring: connector::Wiring,
    /// What somebody chose for it.
    pub wanted: Wanted,
}

impl Screen {
    /// The name this screen's settings are filed under.
    #[must_use]
    pub fn filed_as(&self) -> &str {
        self.naming.name()
    }

    /// The mode that is running.
    #[must_use]
    pub fn mode(&self) -> Option<Mode> {
        self.modes.get(self.current?).copied()
    }

    /// The diagonal in inches, when the monitor said how big it is.
    ///
    /// It is what makes a scale a decision rather than a number: 1.0 on a
    /// 13-inch 4K panel is unreadable, and nothing else on the screen says so.
    #[must_use]
    pub fn inches(&self) -> Option<f32> {
        let (w, h) = self.millimetres?;
        if w == 0 || h == 0 {
            return None;
        }
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (w as f32, h as f32);
        Some(w.hypot(h) / 25.4)
    }

    /// The turn the picture has now.
    ///
    /// It is the PICTURE's, not the setting's: niri adds the panel's own
    /// orientation, and forces one on the nested backend. See
    /// [`ronoctua_niri::Logical::transform`].
    #[must_use]
    pub fn turn(&self) -> Transform {
        self.logical.map_or(Transform::Normal, |l| l.transform)
    }

    /// Whether the compositor is drawing on it at all.
    #[must_use]
    pub const fn lit(&self) -> bool {
        self.logical.is_some()
    }
}

/// Everything, as of one reading.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    /// Every screen the compositor can see, in connector order.
    pub screens: Vec<Screen>,
    /// The connector the keyboard is on.
    pub focused: Option<String>,
    /// Whether the compositor answered at all. False is a machine where the
    /// shell is running without niri, and the view says so instead of
    /// drawing an empty list that looks like "no monitors".
    pub compositor: bool,
    /// Whether the last configuration niri read failed to parse.
    ///
    /// It is how a fragment of ours that will not parse becomes a sentence
    /// rather than a change that silently did nothing.
    pub config_failed: bool,
    /// Whether OUR OWN `display.json` failed to parse.
    ///
    /// Not the same fact as [`Snapshot::config_failed`], which is niri's. When
    /// this is true the reading fell back to the default, so every screen in
    /// the list draws as though nothing had ever been chosen for it — the
    /// person's settings LOOK lost. They are not: the file is intact and this
    /// crate refuses to write over it. But a view that shows the loss and not
    /// the reason is the same silent failure the niri flag beside it exists to
    /// prevent, so it is said out loud on the same terms.
    pub settings_failed: bool,
}

impl Snapshot {
    /// One screen, by connector.
    #[must_use]
    pub fn screen(&self, at: &str) -> Option<&Screen> {
        self.screens.iter().find(|s| s.at == at)
    }

    /// The screen the keyboard is on, or the only one there is.
    ///
    /// The fallback matters: between a monitor going away and the next taking
    /// focus, niri answers `None`, and a brightness key pressed in that
    /// second should still dim the screen the person is looking at.
    #[must_use]
    pub fn focused_screen(&self) -> Option<&Screen> {
        if let Some(at) = &self.focused
            && let Some(s) = self.screen(at)
        {
            return Some(s);
        }
        match self.screens.as_slice() {
            [only] => Some(only),
            _ => None,
        }
    }
}

/// Something to do to the screens.
#[derive(Debug, Clone, PartialEq)]
pub enum Order {
    /// Read everything again. `deep` also asks the monitors themselves,
    /// which costs a fifth of a second each and is not worth doing on every
    /// hotplug.
    Look {
        /// Whether to ask the monitors over DDC/CI as well.
        deep: bool,
    },
    /// Apply now and remember nothing — the preview.
    Try {
        /// The connector.
        at: String,
        /// What to ask niri for.
        ask: Ask,
    },
    /// Write it down, and then say what it means.
    ///
    /// FORGETTING IS THIS ORDER with `Wanted::default()`: every field of a
    /// `Wanted` means "let the compositor decide" by default, so there is
    /// nothing a separate `Forget` could do that this does not. There was
    /// one, it was byte-for-byte this arm with the default substituted, and
    /// nothing ever sent it.
    ///
    /// The file is what survives a restart; the asks that follow it are what
    /// make the running compositor agree with it — including undoing a
    /// preview, which the reload will NOT do on its own. See the module's
    /// header.
    Keep {
        /// The connector.
        at: String,
        /// Everything chosen for that monitor.
        wanted: Box<Wanted>,
    },
    /// Put back what is written down, dropping every preview.
    ///
    /// It writes nothing. It says, over the IPC and knob by knob, what the
    /// file already says — which is the only thing that actually discards a
    /// transient change.
    Undo,
    /// Move a screen's brightness by a step, in percent.
    ///
    /// # Why a STEP and not a percentage to land on
    ///
    /// Because a percentage only addresses a value a percentage can name, and
    /// a laptop panel routinely offers seven, ten or fifteen raw steps. On a
    /// fifteen-step panel at raw 1, minus five percent lands back on raw 1:
    /// the key does nothing, repeatedly and silently, and the reading
    /// afterwards reports the same number so nothing on screen says why.
    /// [`backlight::stepped_raw`] is the arithmetic that always moves, and it
    /// needs the direction rather than a destination.
    ///
    /// Below the floor it CLAMPS rather than refusing: a screen taken to
    /// nothing is one you cannot find the key on to undo it.
    Bright {
        /// The connector, or `None` for whichever screen has the focus —
        /// which is what the keyboard's own brightness keys mean, and what a
        /// row in the view never means, because the row names its screen.
        at: Option<String>,
        /// How far, in percent, and which way.
        by: i16,
    },
    /// How big the text is, in percent of the design's base size.
    ///
    /// IT BELONGS TO NO ONE SCREEN, which is why it is an order of this crate
    /// and not of a monitor: the shell draws one size across every screen,
    /// because the footer's height is the same on all of them. A per-screen
    /// size would be a per-screen footer height, and then every buffer would
    /// reflow when the focus moved between two monitors — which
    /// `docs/ARCHITECTURE.md` records as already decided.
    ///
    /// It is written to the file and nothing else: the doors hear the file.
    Text(u16),
    /// The compositor read its configuration — niri's `ConfigLoaded`, which
    /// only the shell receives, forwarded here.
    ///
    /// It is the ONLY way this crate can learn that the fragment it wrote was
    /// read, and whether it parsed. Without it a fragment that will not parse
    /// is a change that silently did nothing, which is the whole class of
    /// defect this crate exists to remove.
    Reloaded {
        /// Whether niri failed to read it.
        failed: bool,
    },
}

/// A refusal, with the words that go beside it.
///
/// It replaces a `last: Refusal` field on the thread. That field had to be
/// set by hand before every fallible call, which is a discipline rather than
/// a mechanism — and the two places that forgot would have reported `Other`
/// for a refusal that had a name and a fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// Which refusal it is.
    pub why: Refusal,
    /// What to put on the message line.
    pub words: String,
}

impl Refused {
    /// A refusal with a name.
    fn of(why: Refusal, words: impl Into<String>) -> Self {
        Self {
            why,
            words: words.into(),
        }
    }
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.words)
    }
}

impl std::error::Error for Refused {}

/// ANYTHING THAT WAS NOT NAMED IS `Other`, with its own chain as the words.
/// It is what makes `?` usable on the ordinary errors underneath — an
/// unreadable file, a socket that went away — without each of them having to
/// be classified at the point it happens.
impl From<anyhow::Error> for Refused {
    fn from(e: anyhow::Error) -> Self {
        Self::of(Refusal::Other, format!("{e:#}"))
    }
}

/// Why something could not be done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// There is no compositor to ask.
    NoCompositor,
    /// No screen by that name — it was unplugged mid-gesture.
    NoSuchScreen,
    /// This screen's brightness cannot be moved from here.
    NoLight,
    /// The attribute is there and this process may not write it. It is the
    /// udev rule, and it is the one refusal with a fix somebody can carry out.
    NotWritable,
    /// There is nowhere to write a preference.
    NoPlaceToWrite,
    /// Anything else, with its own words alongside.
    Other,
}

/// What the watcher says.
#[derive(Debug, Clone)]
pub enum Event {
    /// A fresh reading. It follows every order, because the answer to an
    /// order is not proof that it happened.
    Snapshot(Box<Snapshot>),
    /// An order finished — with why not, when it did not.
    Done {
        /// What was asked.
        order: Order,
        /// Why it did not happen.
        refused: Option<Refused>,
    },
}

/// What the display thread says about its own progress.
///
/// TWO WORDS AND NOT ONE, because `Handle::finish` has to tell "still working"
/// from "finished" — and a channel that only closes cannot: a thread that
/// drained its queue and a thread that PANICKED both drop the sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Beat {
    /// One order is done, and the thread is going round again.
    Did,
    /// The queue is drained and the thread is leaving.
    Done,
}

/// The way to send orders to the thread.
///
/// NOT `Clone`, since it grew the thread: two of these would be two callers
/// each believing they could wait for the thread to finish, and only one of
/// them could. Nothing clones it — a door holds one.
#[derive(Debug)]
pub struct Handle {
    orders: mpsc::Sender<Order>,
    /// The thread's last act, so a caller can wait for what it has just sent.
    ///
    /// See [`Handle::finish`]. Without it a door that quits leaves the thread
    /// with a queued order and no time to carry it out — and the process ends
    /// before it is scheduled.
    ///
    /// A CHANNEL AND NOT A `JoinHandle`, because a join cannot be given a
    /// deadline and this one needs one: `Niri::request` has no read timeout on
    /// the ordinary path, so a compositor that accepts a connection and never
    /// answers would hold the thread — and a join would hold the DOOR, which
    /// is a window that will not close. `recv_timeout` gives up instead.
    over: mpsc::Receiver<Beat>,
}

impl Handle {
    /// Sends one. A thread that has gone away is a warning, not a panic:
    /// the desktop keeps running without the screens being adjustable.
    pub fn send(&self, order: Order) {
        if let Err(e) = self.orders.send(order) {
            tracing::warn!(error = %e, "there is no display thread to take the order");
        }
    }

    /// Waits for everything this door has sent, and lets the thread go.
    ///
    /// `last` is an order to queue behind the rest first — `Order::Undo` when
    /// a preview is still standing, and `None` when the door has already
    /// answered its own question and only needs the answer carried out.
    ///
    /// # Why a door needs this to close
    ///
    /// Because a preview is a TRANSIENT change over niri's IPC, and the only
    /// thing that takes one down is this thread saying the file's own numbers
    /// back — `Order::Undo`. A door that set `quit` and returned dropped the
    /// sender, ended `main`, and the process took the thread with it: the
    /// queued `Undo` was never carried out, and the change stood until niri
    /// restarted. WORSE than not undoing it, because `previewed` is this
    /// thread's own set — a door opened afterwards has an empty one, so its
    /// `Undo` sends nothing and there is no way back from inside the shell
    /// at all.
    ///
    /// The channel delivers what is queued before it reports the sender gone,
    /// so sending and then joining is enough; the join is what the process
    /// exiting would otherwise cut short.
    pub fn finish(self, last: Option<Order>) {
        // THREE SECONDS, and the number is what the work can honestly take:
        // the thread may be part-way through a DEEP reading when this arrives
        // (a fifth of a second per port, and a laptop with a dock offers a
        // row of them), and the
        // `Undo` behind it is six IPC calls. Past that the compositor is not
        // answering, and waiting longer only holds the door shut.
        const PATIENCE: std::time::Duration = std::time::Duration::from_secs(3);
        if let Some(order) = last {
            self.send(order);
        }
        // THE SENDER GOES FIRST: the loop ends when the channel is empty AND
        // disconnected, so without this the wait below is for a thread that
        // has been told to do one more thing and never to stop.
        drop(self.orders);
        // AND THE TWO WAYS THIS FAILS ARE NOT THE SAME THING.
        //
        // `Disconnected` comes back IMMEDIATELY when the thread is gone —
        // which, if it panicked, is what happens: the sender is dropped during
        // the unwind. Saying "did not finish in time" there blames the clock
        // for a crash, and sends whoever reads the journal looking for a slow
        // monitor. A panicking thread is the one case where waiting was never
        // going to help.
        // AND THE DEADLINE IS PER BEAT, NOT PER QUEUE. Every order the thread
        // finishes resets it, so a slow thread that is MAKING PROGRESS is
        // waited for and only a stuck one is given up on. The budget used to
        // be for the whole queue: the thread's first act is a deep reading
        // that walks in `BREATH` sleeps, an `Undo` costs its own reading plus
        // six IPC calls, and the watch takes another after every order — so on
        // a docked machine it could expire on a thread doing exactly what it
        // was asked, and expiring kills it wherever it is.
        loop {
            match self.over.recv_timeout(PATIENCE) {
                // One more order done. Go round, and give it another PATIENCE.
                Ok(Beat::Did) => {}
                // The queue is drained. This is the ordinary end.
                Ok(Beat::Done) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    tracing::warn!("the screens stopped answering; a change may still be standing");
                    break;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    tracing::warn!("the screens thread is gone; a change may still be standing");
                    break;
                }
            }
        }
    }
}

/// Starts watching the screens.
///
/// ONE thread, and not the three `ronoctua-power` has, because there is
/// nothing to subscribe to: niri publishes no event about outputs at all. The
/// signal comes from Wayland, which the shell already receives — it turns
/// into an [`Order::Look`].
///
/// # Errors
///
/// The thread could not be started.
pub fn watch<F>(on_event: F) -> Result<Handle>
where
    F: Fn(Event) + Send + Sync + 'static,
{
    watch_at(ronoctua_memory::Places::default(), on_event)
}

/// The same, with the roots to read given — see [`Eyes::at`].
///
/// # Errors
///
/// The thread could not be started.
pub fn watch_at<F>(places: ronoctua_memory::Places, on_event: F) -> Result<Handle>
where
    F: Fn(Event) + Send + Sync + 'static,
{
    let (orders, inbox) = mpsc::channel();
    let (over, waited) = mpsc::channel();
    std::thread::Builder::new()
        .name("display".to_owned())
        .spawn(move || {
            let mut eyes = Eyes::at(places);
            on_event(Event::Snapshot(Box::new(eyes.look(true))));
            for order in inbox {
                let refused = eyes.obey(&order).err();
                // THE READING COMES FIRST. Whoever is drawing has to have the
                // new truth before it is told the order finished, or it draws
                // the refusal beside the old numbers.
                on_event(Event::Snapshot(Box::new(eyes.look(order.deep()))));
                on_event(Event::Done { order, refused });
                // AND A BEAT PER ORDER, so the deadline in `Handle::finish`
                // measures PROGRESS instead of the whole queue. One deep
                // reading on a docked machine walks in `BREATH` sleeps, an
                // `Undo` costs its own reading plus six IPC calls, and the
                // watch takes another after every order — a budget for the
                // sum of those is a budget that expires on a thread that is
                // working perfectly well, and expiring kills it wherever it
                // is: possibly with three of one screen's six knobs restored,
                // which is the state `say_what_the_file_says` says at length
                // must never exist.
                let _ = over.send(Beat::Did);
            }
            // THE LAST ACT, and it is what `Handle::finish` waits for. Said
            // out loud rather than left to the channel closing, because a
            // panic closes it too and the two need different words.
            let _ = over.send(Beat::Done);
        })
        .context("could not start the display thread")?;
    Ok(Handle {
        orders,
        over: waited,
    })
}

impl Order {
    /// Whether carrying this out is worth asking the monitors again.
    const fn deep(&self) -> bool {
        match self {
            Self::Look { deep } => *deep,
            // A BRIGHTNESS ORDER MUST NOT, and the first version of this had
            // it backwards. A monitor takes up to a second to move its own
            // backlight; asking it 150 ms after the write gives the number
            // from BEFORE, which then overwrites the remembered one — so the
            // row snapped back to the old value, which is the very thing the
            // deep reading was added to prevent. A panel gains nothing
            // either: `light_of` reads a backlight on every look, deep or not.
            _ => false,
        }
    }
}

/// The thread's own memory.
struct Eyes {
    /// What was chosen, read from disk once and written back on every change.
    prefs: Preferences,
    /// What each monitor answered about its own brightness.
    ///
    /// Keyed by the BUS **and** the name the monitor is filed under, because
    /// `/dev/i2c-N` belongs to the connector and not to what is plugged into
    /// it: a monitor swapped into the same port would otherwise inherit the
    /// previous one's reading, including its silence.
    ///
    /// `None` is one that was asked and said nothing. It is remembered so
    /// that an ordinary reading does not ask again — at a fifth of a second
    /// each, over as many ports as the machine offers — while a DEEP reading
    /// does,
    /// because silence is not always final. See [`Eyes::light_of`].
    monitors: BTreeMap<(PathBuf, String), Option<ddc::Reading>>,
    /// The screens a preview is standing on, since the last time anything
    /// said out loud what the file means.
    previewed: BTreeSet<String>,
    /// Whether the last configuration niri read failed to parse. It survives
    /// between readings because a reading cannot tell — only the event can.
    config_failed: bool,
    /// Whether the last read of `display.json` fell back to the default.
    settings_failed: bool,
    /// Whether the size of the text in `self.prefs` is one an order of ours
    /// just set, rather than one read from the file.
    ///
    /// `write` carries over only what this thread owns, and the text is
    /// normally somebody else's — so without this an `Order::Text` would
    /// re-read the old number over the new one and write that back.
    wrote_the_text: bool,
    /// The same, for the screens — and it was missing, which cost a setting.
    ///
    /// `write` used to carry the screens over ALWAYS, on the argument that
    /// they are what this thread owns. They are not: two doors run two of
    /// these, and each holds whatever the last order's reading left. An
    /// `Order::Text` takes no reading of its own, so stepping the size of the
    /// text in one door wrote that door's stale `screens` back over the file —
    /// deleting a scale the OTHER door had just kept, from `display.json` and
    /// from the fragment both. The `display.json` watch usually gets a fresh
    /// reading in first; with the watch down (`toolkits::watch` answers `None`
    /// and nothing else refreshes) nothing did.
    /// The screens an order of OURS changed, each with what this door was
    /// LAST SHOWN for it — the base of a three-way merge.
    ///
    /// It was a `bool` meaning "the screens are mine now", and that flag wrote
    /// this door's whole map over the file. Two doors run two of these, so it
    /// cost the other door's work in two different ways, and one of them
    /// survived the round that introduced the flag:
    ///
    /// - A DIFFERENT SCREEN. The other door keeps a scale on `DP-1`; this one
    ///   finishes a `Keep` for `HDMI-1` whose reading began before that write,
    ///   and `DP-1`'s scale is gone — from `display.json` and, because the
    ///   fragment is derived from the same map, from what niri loads too.
    /// - THE SAME SCREEN. The record travelling in the order is a whole
    ///   `Wanted`, cloned from the door's own snapshot and one field changed.
    ///   Every OTHER field is that snapshot's, so keeping a rotation puts the
    ///   scale back to whatever this door last saw.
    ///
    /// Keyed by the name the screen is FILED under, holding the base, both
    /// halves are answerable: only these screens are laid over the file, and
    /// for each one only the fields this door actually moved.
    wrote_the_screens: std::collections::BTreeMap<String, fragment::Wanted>,
    /// The roots a reading walks — see [`Eyes::at`].
    places: ronoctua_memory::Places,
}

/// What the file becomes: what is on DISK, with only what an order of ours
/// just changed laid over it.
///
/// Separated from [`Eyes::write`] so it can be tested without a disk — the
/// rule it carries is the one that cost a setting, and it is four lines of
/// arithmetic over two records.
fn merged(
    on_disk: prefs::Preferences,
    mine: &prefs::Preferences,
    text: bool,
    screens: &std::collections::BTreeMap<String, fragment::Wanted>,
) -> prefs::Preferences {
    let mut whole = on_disk;
    for (at, base) in screens {
        // THREE RECORDS, FIELD BY FIELD: what this door was shown (`base`),
        // what it means now (`mine`), and what is on disk this instant
        // (`theirs`, which may hold another door's work done since).
        //
        // A field this door did not move is a field it has no opinion about,
        // and the opinion it would otherwise impose is a stale reading. So
        // `mine` wins only where it DIFFERS from the base; everywhere else the
        // disk does. Forgetting a screen still clears every field, because
        // then every field differs from the base.
        let theirs = whole.of(at);
        // THROUGH `set`, NOT `insert`. `Preferences::set` REMOVES a key whose
        // record says nothing, and its own comment gives the reason: a map
        // that only grows becomes a file listing every monitor ever plugged
        // into this machine, each saying nothing. Reaching into `screens`
        // directly walks past that, so forgetting a screen left an empty
        // record behind instead of taking the entry out.
        whole.set(at, one_of_each(base, &mine.of(at), &theirs));
    }
    if text {
        whole.text = mine.text;
    }
    whole
}

/// One screen's record, merged field by field: mine where I moved it, theirs
/// where I did not.
fn one_of_each(
    base: &fragment::Wanted,
    mine: &fragment::Wanted,
    theirs: &fragment::Wanted,
) -> fragment::Wanted {
    /// Mine if I moved it away from the base; otherwise whatever is on disk.
    ///
    /// A field this door did not move is a field it has no opinion about, and
    /// the opinion it would otherwise impose is a reading that may be older
    /// than the other door's work.
    macro_rules! mine_if_moved {
        ($field:ident) => {
            if mine.$field == base.$field {
                theirs.$field
            } else {
                mine.$field
            }
        };
    }
    // WRITTEN OUT, NOT DERIVED. A `serde_json` round trip or a blanket
    // `..theirs.clone()` would cover a new field silently, and silently is the
    // wrong way to cover it: a field added to `Wanted` and not added here is
    // one this merge drops on every write, which is the defect the whole
    // function exists against. The compiler names it instead — this struct
    // literal has no `..rest`, so it will not build without it.
    fragment::Wanted {
        off: mine_if_moved!(off),
        mode: mine_if_moved!(mode),
        scale: mine_if_moved!(scale),
        transform: mine_if_moved!(transform),
        position: mine_if_moved!(position),
        vary: mine_if_moved!(vary),
        first: mine_if_moved!(first),
    }
}

/// The asks that make the running compositor agree with one written record.
///
/// SIX, ALWAYS, and every one is sent even where nothing was chosen: "not
/// chosen" is the instruction `Automatic`, not the absence of an instruction.
/// Sending only what changed would leave a previewed knob standing, because a
/// reload does not take it down — see the module's header.
///
/// `focus-at-startup` is not among them and cannot be: niri has no action for
/// it. It is a configuration-only knob and takes effect on the next start,
/// which is what its name says.
fn asks_for(w: &Wanted) -> [Ask; 6] {
    // AN OFF SCREEN IS ASKED FOR NOTHING ELSE, and this is not a shortcut: it
    // is what `fragment::kdl_of` writes, for the reason recorded there — an
    // off screen has no picture, so a mode or a place for it describes
    // nothing. Sending them anyway would make the running compositor hold
    // knobs the file does not carry, and a restart would drop them: the two
    // would disagree, which is the one thing this crate promises they cannot.
    let dark = Wanted {
        off: true,
        ..Wanted::default()
    };
    let w = if w.off { &dark } else { w };
    [
        if w.off { Ask::Off } else { Ask::On },
        Ask::Mode(w.mode.map(|m| ModeAsked {
            width: m.width,
            height: m.height,
            refresh: m.rate,
        })),
        // THROUGH THE SAME SIEVE the file goes through. They agreed before
        // only because niri happens to clamp to the same two numbers, which
        // is a coincidence and not an agreement.
        Ask::Scale(fragment::readable_scale(w.scale)),
        Ask::Transform(w.transform),
        Ask::Position(w.position.map(|p| (p.x, p.y))),
        match w.vary {
            Some(fragment::Vary::Always) => Ask::Vrr {
                on: true,
                on_demand: false,
            },
            Some(fragment::Vary::OnDemand) => Ask::Vrr {
                on: true,
                on_demand: true,
            },
            None => Ask::Vrr {
                on: false,
                on_demand: false,
            },
        },
    ]
}

impl Eyes {
    /// Everything this thread remembers, with the roots to read given.
    ///
    /// # Why the roots are a parameter
    ///
    /// Because a reading walks `/sys/class/drm` and then speaks DDC/CI on
    /// every `/dev/i2c-N` a connector names — three tries fifty milliseconds
    /// apart on one that stays silent. A test that starts this thread on the
    /// real roots is a test that pokes whatever monitors are plugged into the
    /// machine running it, and AGENTS.md's own rule is to iterate with `cargo
    /// test`. Given a root with no `class/drm` under it, `connector::read`
    /// takes its early return and nothing is asked of any hardware.
    ///
    /// It is the shape `connector::read_in` and `backlight::read_in` already
    /// have, one level up.
    fn at(places: ronoctua_memory::Places) -> Self {
        Self {
            places,
            prefs: prefs::store().read(),
            monitors: BTreeMap::new(),
            previewed: BTreeSet::new(),
            config_failed: false,
            settings_failed: false,
            wrote_the_text: false,
            wrote_the_screens: BTreeMap::new(),
        }
    }

    /// Everything, joined.
    fn look(&mut self, deep: bool) -> Snapshot {
        // THE PREFERENCES ARE RE-READ, and this is not caution. `display.json`
        // has a SECOND WRITER — the person, with an editor, which is a thing
        // this repository's files are meant to allow — and it carries a field
        // this thread never touches: the size of the text. Reading it once at
        // start and writing the whole file back on the next `Keep` put that
        // field back as it was, silently undoing an edit made since.
        //
        // It is the defect `Queue::took_from_disk` records one file over, and
        // the cure is the same: do not hold what somebody else may change.
        // One small JSON read per reading, and a reading happens on an order
        // or a hotplug — never per frame.
        // ONE READ, TWO ANSWERS. `parsed` says whether the file was actually
        // understood; `read` cannot, because it gives the same empty value for
        // "no file yet" and for "a file I could not parse". The first is every
        // machine's first morning and the second is somebody's settings
        // apparently gone, and the list has to be able to tell them apart.
        let understood = prefs::store().parsed();
        self.settings_failed = understood.is_none();
        self.prefs = understood.unwrap_or_default();
        let wiring = connector::read(&self.places);

        let (outputs, focused, compositor) = match Niri::connect() {
            Ok(mut n) => {
                let outs = n.outputs().unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "niri did not answer about its outputs");
                    BTreeMap::new()
                });
                let focused = n.focused_output().ok().flatten().map(|o| o.name);
                (outs, focused, true)
            }
            Err(e) => {
                // NOT AN ERROR. The shell runs under niri, but the tests and
                // the tools do not, and an empty list is honest there.
                tracing::debug!(error = %e, "no compositor to ask about the screens");
                (BTreeMap::new(), None, false)
            }
        };

        // A BUS IS ONLY WORTH REMEMBERING WHILE A MONITOR IS ON IT. Measured:
        // an empty DisplayPort connector costs 270 ms to ask, and a laptop
        // with a docking station offers a row of them — which is seconds of
        // nothing on every reading.
        let live: BTreeSet<(PathBuf, String)> = outputs
            .values()
            .filter_map(|o| {
                let bus = wiring.get(&o.name)?.bus.as_ref()?;
                Some((bus.clone(), o.naming().name().to_owned()))
            })
            .collect();
        self.monitors.retain(|key, _| live.contains(key));

        let screens = outputs
            .values()
            .map(|o| {
                let wiring = wiring
                    .get(&o.name)
                    .cloned()
                    .unwrap_or_else(|| connector::Wiring {
                        // A CONNECTOR THE SYSFS WALK DID NOT SEE still knows what
                        // KIND it is from its name alone. Without this every
                        // screen in a VM and in the nested loop reads external,
                        // and the row says the wrong thing about the machine it
                        // is running on.
                        internal: connector::is_internal(&o.name),
                        ..connector::Wiring::default()
                    });
                self.screen_of(o, wiring, deep)
            })
            .collect();

        Snapshot {
            screens,
            focused,
            compositor,
            config_failed: self.config_failed,
            settings_failed: self.settings_failed,
        }
    }

    fn screen_of(&mut self, o: &Output, wiring: connector::Wiring, deep: bool) -> Screen {
        let naming = o.naming();
        Screen {
            at: o.name.clone(),
            wanted: self.prefs.of(naming.name()),
            light: self.light_of(&wiring, naming.name(), deep),
            naming,
            description: o.identity(),
            make: o.make.clone(),
            model: o.model.clone(),
            serial: o.serial.clone(),
            millimetres: o.physical_size,
            modes: o.modes.clone(),
            current: o.current_mode,
            logical: o.logical,
            vrr_supported: o.vrr_supported,
            vrr_enabled: o.vrr_enabled,
            internal: wiring.internal,
            wiring,
        }
    }

    fn light_of(
        &mut self,
        wiring: &connector::Wiring,
        filed_as: &str,
        deep: bool,
    ) -> Option<Light> {
        if let Some(dir) = &wiring.backlight
            && let Some(p) = backlight::read_in(dir)
        {
            return Some(Light::Panel(p));
        }
        let bus = wiring.bus.as_ref()?;
        let key = (bus.clone(), filed_as.to_owned());
        // A MONITOR THAT SAID NOTHING IS NOT ASKED AGAIN — except on a DEEP
        // reading, which is what `deep` now means and its only meaning: ask
        // again. It matters because silence is not always final: DDC/CI is a
        // thing people switch on in the monitor's own menu, and a monitor in
        // standby says nothing and says plenty once awake. Without a way back
        // the only cure would be restarting the shell.
        //
        // It is affordable because nothing takes a deep reading casually: the
        // first one, and whatever gesture the view offers for "ask again".
        if deep || !self.monitors.contains_key(&key) {
            let answer = ddc::Bus::open(bus)
                .and_then(|mut b| b.get(ddc::LUMINANCE))
                .map_err(|e| tracing::debug!(bus = %bus.display(), error = %e, "no DDC/CI here"))
                .ok();
            self.monitors.insert(key.clone(), answer);
        }
        self.monitors
            .get(&key)
            .copied()
            .flatten()
            .map(|r| Light::Monitor(r.percent()))
    }

    fn obey(&mut self, order: &Order) -> Result<(), Refused> {
        match order {
            Order::Look { .. } => Ok(()),
            Order::Reloaded { failed } => {
                self.config_failed = *failed;
                Ok(())
            }
            Order::Text(percent) => {
                self.prefs.text = ronoctua_tokens::text::allowed(*percent);
                self.wrote_the_text = true;
                self.write()
            }
            Order::Try { at, ask } => {
                self.ask_niri(at, *ask)?;
                self.previewed.insert(at.clone());
                Ok(())
            }
            Order::Keep { at, wanted } => {
                let filed_as = self.filed_as(at)?;
                // THE BASE IS TAKEN BEFORE THE SET, and it is what this door
                // was last shown for that screen. `write` merges against it,
                // so the fields this order did not move are left to whatever
                // is on disk — which may be the other door's work, done since
                // this door last looked. `entry` and not `insert`: two orders
                // for one screen between two writes must keep the FIRST base,
                // or the second would take this door's own half-written record
                // as the thing to compare against and every field would read
                // as unmoved.
                let base = self.prefs.of(&filed_as);
                self.wrote_the_screens
                    .entry(filed_as.clone())
                    .or_insert(base);
                self.prefs.set(&filed_as, (**wanted).clone());
                self.write()?;
                self.say_what_the_file_says(std::slice::from_ref(at))
            }
            Order::Undo => {
                let standing: Vec<String> = self.previewed.iter().cloned().collect();
                self.say_what_the_file_says(&standing)
            }
            // ONE READING, and the one `watch` takes afterwards is the
            // other. A held brightness key repeats, and a reading walks
            // `/sys/class/drm` and asks the compositor twice — so taking a
            // third for the same press was a cost on the one path that
            // repeats.
            Order::Bright { at, by } => {
                let now = self.look(false);
                let at = match at {
                    Some(named) => named.clone(),
                    None => now
                        .focused_screen()
                        .ok_or_else(|| {
                            Refused::of(Refusal::NoSuchScreen, "there is no screen in focus")
                        })?
                        .at
                        .clone(),
                };
                self.brighten(&now, &at, *by)
            }
        }
    }

    /// Tells the compositor, knob by knob, what the file now says.
    ///
    /// It is what actually discards a preview: a reload will not, because
    /// niri preserves transient output changes whenever the configuration it
    /// re-reads is unchanged from the last disk load.
    fn say_what_the_file_says(&mut self, screens: &[String]) -> Result<(), Refused> {
        if screens.is_empty() {
            return Ok(());
        }
        let now = self.look(false);
        // ONE CONNECTION FOR ALL OF IT. Six asks through `ask_niri` would be
        // six connections and six `Outputs` queries — eighteen requests for
        // one `Keep` — and every one of those queries would be asking again
        // what the snapshot above already proves.
        let mut niri =
            Niri::connect().map_err(|e| Refused::of(Refusal::NoCompositor, format!("{e:#}")))?;

        // EVERY SCREEN IS SETTLED EVEN IF ONE REFUSES. Stopping at the first
        // would leave that screen half put back — some knobs restored and
        // others still previewed, with nothing recording which — and would
        // skip the ones after it entirely. The refusal is carried out and
        // reported at the end.
        let mut trouble: Option<Refused> = None;
        for at in screens {
            let Some(screen) = now.screen(at) else {
                // Unplugged mid-gesture. Nothing is standing on a screen that
                // is gone, so it stops being remembered as previewed — which
                // is what keeps that set from growing for the life of the
                // process.
                self.previewed.remove(at);
                continue;
            };
            let wanted = self.prefs.of(screen.filed_as());
            let mut settled = true;
            for ask in asks_for(&wanted) {
                // THE ANSWER IS READ, and `OutputWasMissing` is a failure
                // here even though it is not an error anywhere. The snapshot
                // above proved the screen was there; between it and the last
                // of six asks it can go away, and then niri KEEPS every one
                // of them for a screen that is gone. Counting that as settled
                // would report success for nothing applied, and would leave
                // the junk section behind — the hazard the `Try` path has a
                // guard against, coming back through the path where the guard
                // was deliberately removed as redundant.
                match niri.output_action(at, ask) {
                    Ok(ronoctua_niri::Change::Applied) => {}
                    Ok(ronoctua_niri::Change::OutputWasMissing) => {
                        settled = false;
                        trouble.get_or_insert_with(|| {
                            Refused::of(
                                Refusal::NoSuchScreen,
                                format!("{at} went away while it was being put back"),
                            )
                        });
                    }
                    // WITH ONE CONNECTION HELD OPEN, an error here is the
                    // socket and not a refusal about this screen. Calling it
                    // `Other` would throw away the one refusal in this crate
                    // that names its own fix.
                    Err(e) => {
                        settled = false;
                        trouble.get_or_insert_with(|| {
                            Refused::of(Refusal::NoCompositor, format!("{e:#}"))
                        });
                    }
                }
            }
            if settled {
                self.previewed.remove(at);
            }
        }
        trouble.map_or(Ok(()), Err)
    }

    fn filed_as(&mut self, at: &str) -> Result<String, Refused> {
        let now = self.look(false);
        if !now.compositor {
            return Err(Refused::of(
                Refusal::NoCompositor,
                "there is no compositor to ask about the screens",
            ));
        }
        now.screen(at)
            .map(|s| s.filed_as().to_owned())
            .ok_or_else(|| {
                Refused::of(
                    Refusal::NoSuchScreen,
                    format!("there is no screen called {at} any more"),
                )
            })
    }

    fn ask_niri(&mut self, at: &str, ask: Ask) -> Result<(), Refused> {
        let mut n =
            Niri::connect().map_err(|e| Refused::of(Refusal::NoCompositor, format!("{e:#}")))?;
        // ASK FIRST WHETHER IT IS THERE. For a name it does not know, niri
        // pushes a fresh output section into its RUNNING configuration and
        // keeps it — so a connector unplugged mid-gesture would leave junk
        // behind that outlives the gesture.
        let outputs = n
            .outputs()
            .map_err(|e| Refused::of(Refusal::NoCompositor, format!("{e:#}")))?;
        if !outputs.contains_key(at) {
            return Err(Refused::of(
                Refusal::NoSuchScreen,
                format!("there is no screen called {at} any more"),
            ));
        }
        // THE ANSWER IS NOT PROOF THAT IT WORKED — `Applied` comes back for
        // a mode the monitor does not have, and what confirms that is the
        // reading which follows. But it IS proof of which screen was touched,
        // and the guard above can go stale in the round trip between them.
        // NAMED, and not left to the catch-all. It is the same socket the
        // settle path holds, already connected and already answered once by
        // `outputs()` above — so a failure here is the compositor going away,
        // and two adjacent doors calling one failure by two names is what
        // `Refused` exists to prevent.
        let answer = n
            .output_action(at, ask)
            .map_err(|e| Refused::of(Refusal::NoCompositor, format!("{e:#}")))?;
        if answer == ronoctua_niri::Change::OutputWasMissing {
            return Err(Refused::of(
                Refusal::NoSuchScreen,
                format!("there is no screen called {at} any more"),
            ));
        }
        Ok(())
    }

    fn brighten(&mut self, now: &Snapshot, at: &str, by: i16) -> Result<(), Refused> {
        let screen = now.screen(at).ok_or_else(|| {
            Refused::of(
                Refusal::NoSuchScreen,
                format!("there is no screen called {at} any more"),
            )
        })?;
        let filed_as = screen.filed_as().to_owned();
        let gone = |what: &str| Refused::of(Refusal::NoSuchScreen, format!("the {what} went away"));

        match screen.light.as_ref() {
            Some(Light::Panel(p)) => {
                if !p.writable {
                    return Err(Refused::of(
                        Refusal::NotWritable,
                        "this panel's brightness belongs to root. The udev rule of \
                         `ronoctua.display` is what opens it to the group.",
                    ));
                }
                let dir = screen
                    .wiring
                    .backlight
                    .clone()
                    .ok_or_else(|| gone("backlight"))?;
                // AND A STEP THAT CANNOT MOVE IS NOT A WRITE. `stepped_raw`
                // clamps, so at either end of the range it answers where the
                // panel already is — and writing that was one sysfs write per
                // keyboard repeat for a number that does not change. The text
                // row, the varying refresh and the place all refuse at their
                // ends; this knob did not.
                let want = backlight::stepped_raw(p.now, p.max, by);
                if want == p.now {
                    return Ok(());
                }
                backlight::write_in(&dir, want)?;
            }
            Some(Light::Monitor(_)) => {
                let bus = screen.wiring.bus.clone().ok_or_else(|| gone("bus"))?;
                let key = (bus.clone(), filed_as);
                let was = self
                    .monitors
                    .get(&key)
                    .copied()
                    .flatten()
                    .ok_or_else(|| gone("monitor's answer"))?;
                // ITS OWN SCALE, NOT A PERCENTAGE. A monitor is entitled to
                // any ceiling and several use 255: writing 55 there is 22% of
                // the picture, and the row would say 55 while the screen went
                // dark.
                let raw = backlight::stepped_raw(u32::from(was.now), u32::from(was.max), by);
                let raw = u16::try_from(raw).unwrap_or(was.max);
                // THE SAME AT THIS END, and it costs more here: a DDC/CI
                // exchange is two I2C transactions and a wait, per repeat.
                if raw == was.now {
                    return Ok(());
                }
                ddc::Bus::open(&bus)?.set(ddc::LUMINANCE, raw)?;
                // A monitor takes up to a second to move its own backlight,
                // so what it would answer right now is the old number. The
                // remembered value is the one just asked for; the next deep
                // reading replaces it with the monitor's own.
                self.monitors.insert(
                    key,
                    Some(ddc::Reading {
                        now: raw,
                        max: was.max,
                    }),
                );
            }
            None => {
                return Err(Refused::of(
                    Refusal::NoLight,
                    "this screen's brightness is on its own buttons",
                ));
            }
        }
        Ok(())
    }

    /// Writes both halves: the compositor's, and ours.
    ///
    /// THE COMPOSITOR'S FIRST, and the order is a choice between two bad
    /// halves of a failure. Ours first would store a preference that was
    /// never applied, and it would then take effect at the next successful
    /// write — a setting appearing by itself, days later, with nothing
    /// connecting it to the gesture. This way the picture changes, the
    /// refusal says the saving failed, and the two agree with each other.
    /// In practice both fail together: one directory, one disk.
    fn write(&mut self) -> Result<(), Refused> {
        let nowhere = || {
            Refused::of(
                Refusal::NoPlaceToWrite,
                "there is nowhere to write a preference: no configuration directory",
            )
        };
        let dir = ronoctua_store::config_dir().ok_or_else(nowhere)?;
        // THE RECONCILIATION COMES FIRST, because the fragment is derived from
        // what it settles. A reading happens on an ORDER; the file can change
        // between one and the next, and writing back what the last reading saw
        // undoes an edit made in the meantime. `took_from_disk`, which this
        // crate's comment names, reconciles at the moment of WRITING, and that
        // is the half that was missing.
        //
        // AND ONLY WHAT AN ORDER OF OURS JUST CHANGED is carried over — see
        // `wrote_the_text` and `wrote_the_screens`. Everything else in the file
        // belongs to whoever wrote it, and "the screens are ours" was not true:
        // there are two of these threads.
        // AND A FILE THAT WILL NOT PARSE IS NOT WRITTEN OVER.
        //
        // `Store::read` answers the default for a file it cannot understand —
        // right for a reader, fatal for a writer. Taken here it would make
        // `on_disk.screens` empty, and then the very next order writes that
        // emptiness back: `display.json` becomes `{"screens":{},"text":N}` and
        // the fragment, which is DERIVED from the same map, becomes its header
        // alone. niri reloads it and every kept mode, scale, rotation and
        // position reverts — from one press of the text-size key, on a file
        // this crate's own doc invites a person to edit by hand.
        //
        // Refusing costs the order and keeps the file, which is the trade the
        // store's promise names: one feature, not the contents.
        let Some(on_disk) = prefs::store().parsed() else {
            return Err(Refused::of(
                Refusal::NoPlaceToWrite,
                format!(
                    "{} will not parse, and writing would replace it. Fix or \
                     remove the file.",
                    prefs::FILE
                ),
            ));
        };
        self.prefs = merged(
            on_disk,
            &self.prefs,
            self.wrote_the_text,
            &self.wrote_the_screens,
        );
        self.wrote_the_text = false;
        self.wrote_the_screens.clear();
        // AND THE FRAGMENT IS WRITTEN ONLY WHEN ITS BYTES MOVE. Every write
        // through `write_atomically` is a rename, which is what niri's include
        // watcher sees — so rewriting the same bytes was a full configuration
        // reload, and each reload is a `ConfigLoaded` that both doors turn into
        // a fresh reading (a walk of `/sys/class/drm` and two connections to
        // niri, in each). Stepping the size of the text has nothing to do with
        // the compositor and paid that per keystroke.
        //
        // COMPARED AND NOT REASONED ABOUT: the fragment is a few hundred bytes
        // and reading it back is exact, which a flag would not be — a hand
        // edit of `display.json` has to reach the fragment too.
        let kdl = fragment::kdl_of(&self.prefs.screens);
        let same = std::fs::read(dir.join(fragment::FILE)).is_ok_and(|had| had == kdl.as_bytes());
        if !same {
            ronoctua_store::write_atomically(&dir, fragment::FILE, kdl.as_bytes())
                .map_err(|e| Refused::of(Refusal::NoPlaceToWrite, e.to_string()))?;
        }
        prefs::store()
            .write(&self.prefs)
            .map_err(|e| Refused::of(Refusal::NoPlaceToWrite, e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A screen the reader could report — INVENTED, every field of it.
    ///
    /// The make, the model and the PHYSICAL SIZE are hardware identity, and
    /// this repository is public (invariants 2 and 13). The millimetres are
    /// the trap: a capture keeps its make and model in plain sight, and they
    /// get noticed, while a pair of numbers reads as arithmetic and stays.
    /// A panel's size names its model as surely as the model does.
    ///
    /// So they are round numbers that no panel has, chosen the way
    /// `ronoctua_niri`'s fixture chooses them and for the same written reason.
    fn screen(at: &str, logical: Option<Logical>) -> Screen {
        Screen {
            at: at.to_owned(),
            naming: Naming::Description(format!("Make Model {at}")),
            description: format!("Make Model {at}"),
            make: "Make".to_owned(),
            model: "Model".to_owned(),
            serial: Some(at.to_owned()),
            millimetres: Some((300, 200)),
            modes: vec![Mode {
                width: 1920,
                height: 1200,
                refresh_rate: 59_951,
                is_preferred: true,
            }],
            current: Some(0),
            logical,
            vrr_supported: false,
            vrr_enabled: false,
            internal: true,
            light: None,
            wiring: connector::Wiring::default(),
            wanted: Wanted::default(),
        }
    }

    fn place() -> Logical {
        Logical {
            x: 0,
            y: 0,
            width: 1920,
            height: 1200,
            scale: 1.0,
            transform: Transform::Normal,
        }
    }

    #[test]
    fn cancelling_says_every_knob_and_not_only_the_changed_one() {
        // THE DEFECT THIS REPLACES: cancelling used to be "rewrite the file
        // and let the reload drop the preview". niri does not drop it — when
        // the configuration it re-reads is unchanged from the last disk load
        // it takes the other branch and puts the transient changes BACK. So a
        // preview of a rotation survived Escape, for ever.
        //
        // The fix is to say what the file means, knob by knob, and "nothing
        // chosen" has to be said too: it is the instruction `Automatic`.
        let quiet = asks_for(&Wanted::default());
        assert_eq!(
            quiet,
            [
                Ask::On,
                Ask::Mode(None),
                Ask::Scale(None),
                Ask::Transform(Transform::Normal),
                Ask::Position(None),
                Ask::Vrr {
                    on: false,
                    on_demand: false
                },
            ]
        );

        // And a record that says something says all of it.
        let full = Wanted {
            off: false,
            mode: Some(fragment::Chosen {
                width: 1920,
                height: 1200,
                rate: Some(59.951),
            }),
            scale: Some(1.5),
            transform: Transform::R270,
            position: Some(fragment::Spot { x: -1920, y: 120 }),
            vary: Some(fragment::Vary::OnDemand),
            first: true,
        };
        let said = asks_for(&full);
        assert_eq!(
            said[1],
            Ask::Mode(Some(ModeAsked {
                width: 1920,
                height: 1200,
                refresh: Some(59.951)
            }))
        );
        assert_eq!(said[2], Ask::Scale(Some(1.5)));
        assert_eq!(said[3], Ask::Transform(Transform::R270));
        assert_eq!(said[4], Ask::Position(Some((-1920, 120))));
        assert_eq!(
            said[5],
            Ask::Vrr {
                on: true,
                on_demand: true
            }
        );
        // `first` is `focus-at-startup`, which niri has no action for. It is
        // in the file and NOT in this list, and that is not an omission.
        assert_eq!(said.len(), 6);

        // Off is the whole of what an off screen is asked for at index 0.
        assert_eq!(
            asks_for(&Wanted {
                off: true,
                ..Wanted::default()
            })[0],
            Ask::Off
        );
        // And "always varying" is not the same ask as "on demand".
        assert_eq!(
            asks_for(&Wanted {
                vary: Some(fragment::Vary::Always),
                ..Wanted::default()
            })[5],
            Ask::Vrr {
                on: true,
                on_demand: false
            }
        );
    }

    #[test]
    fn a_brightness_is_written_in_the_screens_own_units() {
        // THE DEFECT THIS REPLACES: a percentage was written straight into
        // the monitor's VCP value. A monitor is entitled to any ceiling and
        // several use 255 — so "55%" darkened the picture to 22% while the
        // row went on saying 55, and the next reading jumped.
        assert_eq!(backlight::raw_of(50, 255), 128);
        assert_eq!(backlight::raw_of(100, 255), 255);
        assert_eq!(backlight::raw_of(50, 100), 50);
        assert_eq!(backlight::raw_of(50, 65535), 32768);

        // AND IT NEVER REACHES NOTHING. The floor was enforced for a panel
        // and not for a monitor, so the one gesture a row offers — go to
        // zero — could take an external screen dark, with the key to undo it
        // on a screen nobody can read.
        for max in [10u32, 100, 255, 65535] {
            assert!(backlight::raw_of(0, max) >= backlight::floor_raw(max));
            assert!(backlight::raw_of(0, max) >= 1);
            assert!(backlight::stepped_raw(max, max, -100) >= 1);
        }

        // A step moves, even where a percentage cannot address the value.
        assert!(backlight::stepped_raw(3, 15, 5) > 3);
        assert!(backlight::stepped_raw(3, 15, -5) < 3);
    }

    #[test]
    fn the_diagonal_comes_out_in_inches() {
        // A DIAGONAL IS WHAT MAKES A SCALE JUDGEABLE — without it the number
        // has nothing to be a number OF. The arithmetic is what is under
        // test, and the panel it runs on is invented: see `screen`, which
        // says why a size is identity and not just a measurement.
        let s = screen("eDP-1", Some(place()));
        let inches = s.inches().unwrap();
        assert!((inches - 14.20).abs() < 0.05, "{inches}");
        // A monitor that says nothing gets no invented size.
        let mut quiet = screen("DP-1", Some(place()));
        quiet.millimetres = None;
        assert!(quiet.inches().is_none());
        // And one that says zero is not divided by.
        quiet.millimetres = Some((0, 0));
        assert!(quiet.inches().is_none());
    }

    #[test]
    fn the_only_screen_is_the_focused_one_even_when_niri_says_nothing() {
        // Between a monitor going away and the next taking focus, niri
        // answers `None`. A brightness key pressed in that second still has
        // to dim the screen the person is looking at.
        let one = Snapshot {
            screens: vec![screen("eDP-1", Some(place()))],
            focused: None,
            compositor: true,
            config_failed: false,
            settings_failed: false,
        };
        assert_eq!(one.focused_screen().map(|s| s.at.as_str()), Some("eDP-1"));
        // With two, guessing would dim the wrong one.
        let two = Snapshot {
            screens: vec![
                screen("eDP-1", Some(place())),
                screen("DP-1", Some(place())),
            ],
            ..one
        };
        assert!(two.focused_screen().is_none());
        // And what niri names wins over the fallback.
        let named = Snapshot {
            focused: Some("DP-1".to_owned()),
            ..two
        };
        assert_eq!(named.focused_screen().map(|s| s.at.as_str()), Some("DP-1"));
    }

    #[test]
    fn a_screen_that_is_off_is_not_lit_and_has_no_place() {
        let dark = screen("DP-1", None);
        assert!(!dark.lit());
        assert_eq!(dark.turn(), Transform::Normal);
        // But it still says what it could do.
        assert_eq!(dark.modes.len(), 1);
    }

    #[test]
    fn a_panel_that_cannot_be_written_says_so_before_the_key_is_pressed() {
        // The row has to carry it. Finding out at the moment of the gesture
        // is a key that does nothing, which is the defect this replaces.
        let stuck = Light::Panel(backlight::Panel {
            at: "amdgpu_bl1".to_owned(),
            now: 100,
            max: 255,
            writable: false,
        });
        assert!(!stuck.movable());
        assert_eq!(stuck.percent(), 39);
        let free = Light::Monitor(42);
        assert!(free.movable());
        assert_eq!(free.percent(), 42);
    }

    #[test]
    fn only_an_explicit_deep_look_asks_the_monitors_again() {
        // `deep` HAS ONE MEANING: ask the monitors themselves. It costs a
        // fifth of a second per monitor, so only a reading that asked for it
        // gets it.
        assert!(Order::Look { deep: true }.deep());
        assert!(!Order::Look { deep: false }.deep());

        // AND A BRIGHTNESS ORDER IS NOT ONE, which is the opposite of what
        // this test used to assert. A monitor takes up to a second to move
        // its own backlight; the deep reading that followed a write landed
        // about 150 ms later and got the number from BEFORE, which then
        // overwrote the value just asked for — so the row snapped back to the
        // old brightness, the exact defect the deep reading was added to
        // prevent. A panel gains nothing from it either: `light_of` reads a
        // backlight on every look, deep or not.
        assert!(!Order::Bright { at: None, by: 5 }.deep());
        assert!(
            !Order::Bright {
                at: Some("DP-1".to_owned()),
                by: -5
            }
            .deep()
        );

        assert!(!Order::Undo.deep());
        assert!(!Order::Reloaded { failed: true }.deep());
        assert!(
            !Order::Try {
                at: "DP-1".to_owned(),
                ask: Ask::On
            }
            .deep()
        );
    }

    #[test]
    fn an_off_screen_is_asked_for_nothing_but_off() {
        // THE FILE AND THE VOICE HAVE TO SAY THE SAME THING. `kdl_of` writes
        // `off` and nothing else, on the grounds that an off screen has no
        // picture to describe. Sending a mode and a place along with `Off`
        // would leave the compositor holding knobs the file does not carry,
        // and a restart would drop them — the two disagreeing, which is the
        // one thing this crate promises cannot happen.
        let loud = Wanted {
            off: true,
            mode: Some(fragment::Chosen {
                width: 800,
                height: 600,
                rate: None,
            }),
            scale: Some(3.0),
            transform: Transform::R90,
            position: Some(fragment::Spot { x: 77, y: 88 }),
            vary: Some(fragment::Vary::Always),
            first: true,
        };
        assert_eq!(
            asks_for(&loud),
            [
                Ask::Off,
                Ask::Mode(None),
                Ask::Scale(None),
                Ask::Transform(Transform::Normal),
                Ask::Position(None),
                Ask::Vrr {
                    on: false,
                    on_demand: false
                },
            ]
        );
        // And what the fragment writes for it agrees, node for node.
        let written = fragment::kdl_of(&std::collections::BTreeMap::from([("X".to_owned(), loud)]));
        // SCOPED TO THE BLOCK, not the whole file: the header is prose, and
        // asserting over it would fire the day that prose gains one of these
        // words for an unrelated reason.
        let body = &written[written.find("output ").unwrap()..];
        assert!(body.contains("    off\n"), "{body}");
        for knob in ["mode", "scale", "transform", "position", "variable"] {
            assert!(!body.contains(knob), "{knob} in: {body}");
        }
    }

    #[test]
    fn a_scale_that_is_not_a_number_is_never_asked_for_either() {
        // The file sieves it; the voice has to sieve it too, or the two say
        // different things about the same record.
        let mad = Wanted {
            scale: Some(f64::NAN),
            ..Wanted::default()
        };
        assert_eq!(asks_for(&mad)[2], Ask::Scale(None));
        let big = Wanted {
            scale: Some(400.0),
            ..Wanted::default()
        };
        assert_eq!(asks_for(&big)[2], Ask::Scale(Some(10.0)));
    }

    #[test]
    fn a_reload_that_failed_is_remembered_across_readings() {
        // A reading cannot tell: niri answers about its outputs whether or
        // not the last config parsed. Only the event says, and it arrives
        // once — so it has to survive until the next one.
        let mut eyes = Eyes {
            places: ronoctua_memory::Places::default(),
            prefs: Preferences::default(),
            monitors: BTreeMap::new(),
            previewed: BTreeSet::new(),
            config_failed: false,
            settings_failed: false,
            wrote_the_text: false,
            wrote_the_screens: BTreeMap::new(),
        };
        eyes.obey(&Order::Reloaded { failed: true }).unwrap();
        assert!(eyes.config_failed);
        eyes.obey(&Order::Look { deep: false }).unwrap();
        assert!(eyes.config_failed, "a plain look forgot the failure");
        eyes.obey(&Order::Reloaded { failed: false }).unwrap();
        assert!(!eyes.config_failed);
    }

    #[test]
    fn finishing_waits_for_what_it_sent_and_gives_up_in_the_end() {
        // `finish` IS WHAT KEEPS A PREVIEW FROM SURVIVING THE PROCESS. A door
        // that quits with a question standing has to get its `Undo` carried
        // out, and the channel delivers what is queued before it reports the
        // sender gone — so sending and then waiting is enough. What the wait
        // must not do is last for ever: `Niri::request` has no read timeout,
        // and a join would turn a compositor that never answers into a window
        // that will not close.
        let counted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = std::sync::Arc::clone(&counted);
        // ROOTS WITH NO `class/drm` UNDER THEM, so the reading this thread
        // takes before anything else asks nothing of any hardware. Without it
        // this test speaks DDC/CI to whatever is plugged into the machine
        // running it — and if the reading overruns `finish`'s deadline, it
        // fails naming the queued order rather than the cause.
        let nowhere = ronoctua_memory::Places {
            sysfs: std::env::temp_dir().join("ronoctua-display-no-sys"),
            ..ronoctua_memory::Places::default()
        };
        let h = watch_at(nowhere, move |e| {
            if matches!(e, Event::Done { .. }) {
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        })
        .unwrap();
        h.finish(Some(Order::Look { deep: false }));
        assert_eq!(
            counted.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the order queued behind `finish` was never carried out",
        );
    }

    #[test]
    fn a_door_writes_back_only_what_it_changed() {
        // TWO DOORS RUN TWO OF THESE, and each holds whatever its last
        // reading left. `write` carried the screens over ALWAYS, on the
        // argument that they are what this thread owns — so stepping the size
        // of the text in the window wrote the window's STALE screens back over
        // the file, deleting a scale the footer had just kept. An `Order::Text`
        // takes no reading of its own, which is what makes the copy stale.
        let mut on_disk = Preferences::default();
        on_disk.screens.insert(
            "DP-1".to_owned(),
            crate::fragment::Wanted {
                scale: Some(1.5),
                ..crate::fragment::Wanted::default()
            },
        );
        on_disk.text = 100;

        // THE STALE COPY: read before the other door kept anything.
        let mine = Preferences {
            text: 125,
            ..Preferences::default()
        };

        let after = merged(on_disk.clone(), &mine, true, &BTreeMap::new());
        assert_eq!(after.text, 125, "the size this order set was not written");
        assert_eq!(
            after.screens.get("DP-1").and_then(|w| w.scale),
            Some(1.5),
            "a door that only changed the text erased another door's screen",
        );

        // AND THE OTHER WAY ROUND: a door that kept a screen writes it, and
        // leaves a size somebody changed in the meantime alone.
        // A DIFFERENT SIZE FROM THE ONE ON DISK, or the assertion below passes
        // with the `text` guard deleted: both sides being 100 proves nothing
        // about which one was chosen.
        let mut kept = Preferences {
            text: 150,
            ..Preferences::default()
        };
        kept.screens.insert(
            "eDP-1".to_owned(),
            crate::fragment::Wanted {
                scale: Some(2.0),
                ..crate::fragment::Wanted::default()
            },
        );
        let touched = BTreeMap::from([("eDP-1".to_owned(), crate::fragment::Wanted::default())]);
        let after = merged(on_disk, &kept, false, &touched);
        assert_eq!(after.text, 100, "the text on disk was overwritten");
        assert_eq!(after.screens.get("eDP-1").and_then(|w| w.scale), Some(2.0));
        // AND THE SCREEN NOBODY IN THIS ORDER TOUCHED SURVIVES.
        //
        // This assertion used to say the opposite, in as many words — "the
        // screens are written whole, so this door's set is the set" — and it
        // was pinning a defect rather than a rule. Writing the map whole means
        // a `Keep` for one screen deletes every setting the other door made
        // for every OTHER screen, from `display.json` and from the fragment
        // that is derived from it.
        assert_eq!(
            after.screens.get("DP-1").and_then(|w| w.scale),
            Some(1.5),
            "keeping one screen erased another door's work on a different one",
        );
    }

    #[test]
    fn a_door_keeps_the_knob_it_moved_and_not_the_ones_it_did_not() {
        // THE SAME SCREEN, TWO DOORS. The record travelling in `Order::Keep`
        // is a whole `Wanted`, cloned from the door's own snapshot with one
        // field changed — so every OTHER field carries whatever that snapshot
        // held, which is what the door was shown when it last looked.
        //
        // The story: both doors show `DP-1`. The footer keeps `scale 2.0`. The
        // window, whose snapshot still says `scale 1.0`, then rotates the same
        // screen. Writing its record whole would put the scale back to 1.0 —
        // a setting nobody asked to change, undone by a door that was asked
        // about something else.
        let base = crate::fragment::Wanted {
            scale: Some(1.0),
            ..crate::fragment::Wanted::default()
        };
        // WHAT IS ON DISK NOW: the footer's scale, which this door never saw.
        let mut on_disk = Preferences::default();
        on_disk.screens.insert(
            "DP-1".to_owned(),
            crate::fragment::Wanted {
                scale: Some(2.0),
                ..base
            },
        );
        // WHAT THIS DOOR MEANS: its stale scale, and the turn it just chose.
        let mut mine = Preferences::default();
        mine.screens.insert(
            "DP-1".to_owned(),
            crate::fragment::Wanted {
                transform: Transform::R90,
                ..base
            },
        );

        let touched = BTreeMap::from([("DP-1".to_owned(), base)]);
        let after = merged(on_disk, &mine, false, &touched);
        let w = after.screens.get("DP-1").expect("the screen was dropped");
        assert_eq!(
            w.transform,
            Transform::R90,
            "the knob this door actually moved was not written",
        );
        assert_eq!(
            w.scale,
            Some(2.0),
            "a door that rotated a screen put back the scale it last saw, \
             undoing the other door's work on a knob nobody asked it about",
        );
    }

    #[test]
    fn forgetting_a_screen_still_clears_every_knob() {
        // THE MERGE MUST NOT MAKE FORGETTING IMPOSSIBLE. `Order::Keep` with a
        // default `Wanted` IS forgetting — the crate's own doc says so — and
        // under a field-by-field merge that works for exactly one reason:
        // every field then DIFFERS from the base, so every field is "moved"
        // and mine wins throughout. If it did not, `d` would clear the knobs
        // the door had seen and silently keep the ones it had not.
        let base = crate::fragment::Wanted {
            scale: Some(1.5),
            transform: Transform::R90,
            first: true,
            ..crate::fragment::Wanted::default()
        };
        let mut on_disk = Preferences::default();
        on_disk.screens.insert("DP-1".to_owned(), base.clone());
        let mut mine = Preferences::default();
        mine.screens
            .insert("DP-1".to_owned(), crate::fragment::Wanted::default());

        let touched = BTreeMap::from([("DP-1".to_owned(), base)]);
        let after = merged(on_disk, &mine, false, &touched);
        // AND THE ENTRY GOES WITH THEM. This assertion used to demand the
        // opposite — that the key survive holding a default `Wanted` — which
        // pinned a `screens.insert` that walked past `Preferences::set`. That
        // setter REMOVES a key whose record says nothing, and says why: a map
        // that only grows becomes a file listing every monitor ever plugged
        // into this machine, each saying nothing.
        assert!(
            !after.screens.contains_key("DP-1"),
            "forgetting left an empty record behind: {:?}",
            after.screens,
        );
    }

    #[test]
    fn the_running_mode_is_the_one_the_index_names() {
        let s = screen("eDP-1", Some(place()));
        assert_eq!(s.mode().map(|m| m.refresh_rate), Some(59_951));
        // An index niri could not fill in is not a panic.
        let mut interlaced = s.clone();
        interlaced.current = None;
        assert!(interlaced.mode().is_none());
        // Nor is one out of range, which is what a mode list that changed
        // under a stale index would give.
        let mut stale = s;
        stale.current = Some(9);
        assert!(stale.mode().is_none());
    }

    #[test]
    fn a_settings_file_that_will_not_parse_is_said_out_loud() {
        // THE READING FALLS BACK TO THE DEFAULT, so every screen draws as if
        // nothing had ever been chosen for it. That is the person's settings
        // apparently gone — and the file is intact, and this crate refuses to
        // write over it. Showing the loss without the reason is the silent
        // failure `config_failed` exists to prevent, one file over.
        //
        // `parsed` is what tells the two empty answers apart: no file yet
        // (every machine's first morning) against a file nobody can read.
        let dir = std::env::temp_dir().join("ronoctua-prefs-unreadable");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store: ronoctua_store::Store<Preferences> =
            ronoctua_store::Store::at(Some(dir.clone()), prefs::FILE);

        // NO FILE is not a failure.
        assert!(store.parsed().is_some(), "a fresh machine reads as broken");

        // A BROKEN ONE is, and `read` cannot tell you so.
        std::fs::write(dir.join(prefs::FILE), b"{ not json").unwrap();
        assert!(store.parsed().is_none());
        assert_eq!(
            store.read().screens.len(),
            0,
            "the fallback is what makes the settings look lost",
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finishing_waits_while_the_thread_is_still_making_progress() {
        // THE DEADLINE MEASURES PROGRESS, NOT THE WHOLE QUEUE.
        //
        // It used to be one `recv_timeout` for the entire drain, and the drain
        // is not cheap: the thread opens with a DEEP reading that walks in
        // `BREATH` sleeps, an `Undo` costs its own reading plus six IPC calls,
        // and the watch takes another after every order. On a docked machine
        // that sum can pass the budget while the thread is doing exactly what
        // it was asked — and the process then exits and kills it wherever it
        // stands, possibly with some of one screen's knobs restored and the
        // rest still previewed.
        //
        // Proved by the clock: three beats spaced out, and `finish` has to
        // still be there for the last one. The single-`recv` version returned
        // on the FIRST and would come back in a third of the time.
        let (beats, over) = mpsc::channel();
        let (orders, _inbox) = mpsc::channel::<Order>();
        let step = std::time::Duration::from_millis(120);
        std::thread::spawn(move || {
            for _ in 0..2 {
                std::thread::sleep(step);
                let _ = beats.send(Beat::Did);
            }
            std::thread::sleep(step);
            let _ = beats.send(Beat::Done);
        });

        let began = std::time::Instant::now();
        Handle { orders, over }.finish(None);
        let waited = began.elapsed();
        assert!(
            waited >= step * 2,
            "`finish` came back after the first beat instead of waiting for the \
             queue to drain: {waited:?}",
        );
    }
}
