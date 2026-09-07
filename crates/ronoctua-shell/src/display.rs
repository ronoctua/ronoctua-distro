//! The screens — `LEADER s d`.
//!
//! TWO DOORS, and nothing here knows what a door is: a door asks for
//! [`View::rows`] and paints `Row.fields`, and [`crate::display_words`] holds
//! the sentences so the two cannot say a refusal two ways. The footer's is
//! `LEADER s d d`; the window's is `s d w`, in `crate::display_window`.
//!
//! The molds are next door and both are followed on purpose: `bluetooth.rs`
//! for a view with LEVELS (a field says where one is, a `match` in [`rows`]
//! draws it, `Esc` goes back one), and `power.rs` for a list of label and
//! value where `←`/`→` change the value under the cursor.
//!
//! # What is different, and it is one thing
//!
//! A CHANGE TO A SCREEN CAN MAKE THE SCREEN UNREADABLE. Choose a mode the
//! monitor cannot show, or a rotation on the wrong monitor, and the answer is
//! a black rectangle — and the key that would undo it is on a screen nobody
//! can read. So a change of that kind is applied PROVISIONALLY and puts
//! itself back after [`PATIENCE`] unless somebody says to keep it.
//!
//! FIVE KNOBS ARE OF THAT KIND — the resolution, the refresh rate, the scale,
//! the turn, and switching a screen off. The other FIVE are written down as
//! they are asked for, and each says why where it is: a place cannot make a
//! picture unreadable, a varying refresh rate costs battery and not sight, a
//! brightness has its own floor, the size of the text moves every door at once
//! and is undone by the same key that moved it, and which screen the session
//! opens on changes nothing anybody can see until the next session. Everything
//! else in this file is an ordinary list.
//!
//! The question draws "put it back" FIRST and the cursor starts on it, which
//! is the rule the Bluetooth pairing and the disk formatting already follow:
//! a question that appears by itself answers NO to `RET`.

use std::time::{Duration, Instant};

use ronoctua_display::{Screen, Snapshot};
use ronoctua_niri::{Ask, Transform};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::files::{Field, Hit as FilesHit};

/// How long a change that could blind the screen stands before it goes back.
///
/// Fifteen seconds, and the number is about a person and not a machine: it is
/// long enough to read a sentence on a screen that may be flickering or badly
/// scaled and find one key, and short enough that waiting it out is a real
/// way to answer — which is the whole point, because somebody looking at a
/// black rectangle cannot answer any other way.
pub const PATIENCE: Duration = Duration::from_secs(15);

/// The keys this view answers to; the hint line is GENERATED from it.
///
/// Bare letters are free here — nothing is typed in this view. They act on
/// three different things and the difference matters: `a` on the ROW under the
/// cursor, `d` on the SCREEN the level is about, `m` on the level itself. That
/// is why `key` gates the three differently, and why `Where` has a variant for
/// each shape rather than one for "a letter".
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "display-down", Where::Anywhere),
    ("<up>", "display-up", Where::Anywhere),
    ("<left>", "display-less", Where::WhileNothingIsAsked),
    ("<right>", "display-more", Where::WhileNothingIsAsked),
    ("RET", "display-enter", Where::Anywhere),
    ("a", "display-automatic", Where::OnAKnob),
    ("d", "display-forget", Where::OnAMonitor),
    ("m", "display-arrange", Where::OnTheList),
];

/// Where in the view a key is worth teaching.
///
/// The hint line is the scarce thing: a screen that lists a letter which does
/// nothing here is a screen that has to be read twice. It is
/// `bluetooth::Where` with this view's levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// Only on the list of screens.
    OnTheList,
    /// Only inside one screen's own sheet.
    OnAMonitor,
    /// Wherever a row names a knob that can be handed back — the sheet and
    /// the arrangement, whose rows are one per screen.
    ///
    /// FOR `a`, WHICH WORKS ON BOTH. `automatic` resolves the place row's
    /// screen from the row's OWN CONNECTOR rather than from the level
    /// (`keep_for`, which takes one) precisely so that it does, and the table
    /// went on filing it under the sheet — a key that works and is taught
    /// nowhere, which is the counterpart of the two that were taught and did
    /// nothing.
    OnAKnob,
    /// Every level, but not while a question is standing.
    ///
    /// FOR `←`/`→`, WHICH THE QUESTION DOES NOT USE. They were `Anywhere`,
    /// with a comment saying they move "the answer under the cursor in the
    /// question" — and they do not: the question's rows are `PutItBack` and
    /// `KeepIt`, and `turn` has no arm for either, so both keys fall to
    /// `Errand::Nothing`. The hint line on the one screen this view exists to
    /// rescue taught two keys that do nothing.
    ///
    /// The radio reaches the same effect with an `_ if asking => false` arm
    /// rather than a variant, because none of ITS keys is dead under a
    /// question. Here two are, and naming them is what keeps the arm honest.
    WhileNothingIsAsked,
    // THERE IS NO `OnARow`, AND THERE WAS. `←`/`→` move something on every
    // LEVEL — the value under the cursor on a sheet, the screen under it on
    // the map — so a case that named some of those was a case that hid the
    // map's arrows from the hint line.
}

impl Where {
    const fn shown(self, level: &Level, asking: bool) -> bool {
        match self {
            Self::Anywhere => true,
            // A STANDING QUESTION IS THE WHOLE SCREEN, so it teaches only
            // what answers it — `↑`, `↓` and `RET`. A letter that acts on a
            // screen is hidden, because the screen it would act on is the one
            // being asked about; and so are `←`/`→`, which answer nothing.
            _ if asking => false,
            Self::WhileNothingIsAsked => true,
            Self::OnTheList => matches!(level, Level::List),
            Self::OnAMonitor => matches!(level, Level::Monitor(_)),
            Self::OnAKnob => matches!(level, Level::Monitor(_) | Level::Arrange),
        }
    }
}

/// Which screen of the view one is on.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Level {
    /// Every screen, one row each, and what belongs to all of them.
    #[default]
    List,
    /// One screen and everything about it, by connector.
    Monitor(String),
    /// Where the screens sit relative to each other.
    Arrange,
}

/// A key, already reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row, skipping headings.
    Up,
    /// Down one row.
    Down,
    /// The value under the cursor, one step back.
    Less,
    /// The value under the cursor, one step on.
    More,
    /// Open, apply, or answer.
    Enter,
    /// Back one level, or out.
    Escape,
    /// Give the SETTING under the cursor back to the compositor's own choice.
    Automatic,
    /// Forget everything chosen for this screen.
    Forget,
    /// Open the arrangement.
    Arrange,
}

/// What the row under the cursor is about.
///
/// `Eq` for the reason `power::Target` gives: the cursor is restored by TARGET
/// and not by index, because a list of screens reorders under it when a
/// monitor is plugged in. NOT `Copy`, which `power::Target` is — two of these
/// carry a connector, and a `String` is not copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Nothing to do here.
    Nothing,
    /// A screen's own sheet.
    Screen(String),
    /// The arrangement.
    Arrange,
    /// Whether the compositor drives this screen at all.
    Power,
    /// The resolution, among the sizes the monitor offers.
    Resolution,
    /// The refresh rate, among the ones this resolution offers.
    Rate,
    /// The scale.
    Scale,
    /// How the picture is turned.
    Turn,
    /// Whether the refresh rate may vary.
    Vary,
    /// The brightness.
    Light,
    /// Whether the session opens focused here.
    First,
    /// The size of the text, which belongs to no one screen.
    Text,
    /// Where this screen sits, in the arrangement.
    Place(String),
    /// Put the change back.
    PutItBack,
    /// Keep it.
    KeepIt,
}

/// One line of the view.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// What to draw, already placed in cells.
    pub fields: Vec<Field>,
    /// Whether the cursor is on it.
    pub focused: bool,
    /// A title or a blank: the cursor skips it.
    pub heading: bool,
    /// What it is about.
    pub target: Target,
    /// Drawn reachable and REFUSING, with the reason in its own value.
    ///
    /// ELEVEN things set it: the last lit screen, a screen the compositor is
    /// not driving that nothing here turned off, a dark screen, a screen niri
    /// reports no mode for, a size with no second size, a rate with no second
    /// rate, a panel with no variable refresh, a light that belongs to root, a
    /// machine with no reading thread, a door that cannot see `display.json`
    /// change, and a machine with fewer than two lit screens. One of them is
    /// about a mode.
    pub disabled: bool,
}

/// What a standing question is about, and what answering it does.
///
/// # Why there are two natures and not one
///
/// Because two different things can go wrong and they need opposite defaults.
///
/// A change to the PICTURE can leave a screen nobody can read, so it is
/// applied first and taken back by itself if nobody answers: the risk is
/// being unable to say anything, and the safe answer is the one that arrives
/// with no keypress at all.
///
/// FORGETTING is not that. It cannot blind a screen — every default is the
/// compositor's own choice, which is the state the machine boots into — but
/// it throws away every decision somebody made about that monitor, and a
/// countdown that ANSWERS ITSELF would be the wrong shape for it entirely.
/// So nothing happens until somebody says so, and the question stands until
/// they do.
#[derive(Debug, Clone, PartialEq)]
pub enum Deciding {
    /// Applied already; it goes back on its own. The answers are "put it
    /// back" and "keep it", and the clock runs.
    Preview(Ask),
    /// Not applied, and nothing is written unless the answer is yes. The
    /// answers are "leave it" and "forget it", and no clock runs.
    ///
    /// NO PAYLOAD: what a yes writes is `Wanted::default()`, which is what
    /// "forget it" MEANS — every field's default is "let the compositor
    /// decide". It carried a `Box<Wanted>` that was `Box::default()` at its
    /// one construction site, which is a parameter with one value.
    Whether,
}

/// A change standing on its own, waiting to be kept or to go back.
///
/// NOT `Eq`: it carries the [`Ask`], and a scale is an `f64`.
#[derive(Debug, Clone, PartialEq)]
pub struct Asking {
    /// The connector it was made on.
    pub at: String,
    /// A text key naming what was changed.
    pub what: &'static str,
    /// When it was applied, on the MONOTONIC clock.
    ///
    /// Not the wall clock, which was what this used to be. `SystemTime`'s
    /// `duration_since` answers `Err` when the reading it is given is EARLIER
    /// than the mark — which is what a backwards step gives — and the
    /// `is_ok_and` around it then read that as "not over yet". A laptop that
    /// resumes and lets `systemd-timesyncd` pull the clock back is the exact
    /// machine this feature is for, and the outcome was a countdown frozen at
    /// fifteen seconds with a change that could have blinded the screen
    /// standing for ever. `Instant` cannot go backwards.
    pub since: Instant,
    /// WHAT is being decided, and of which of the two natures.
    ///
    /// Not the text key, which only names it for the sentence. "Keep it" has
    /// to write down the ONE thing that changed and leave every other knob
    /// where it was: a question about the scale that also wrote the mode and
    /// the position would opt somebody out of niri's own choice of both,
    /// silently, having asked about neither. And it has to write down what was
    /// ASKED, not only what is on the glass: stepping off the bottom of the
    /// scale ladder asks for `Scale(None)` — "you choose" — and reading the
    /// number niri then chose would write that number down instead, so the
    /// preview and the thing kept would be different changes.
    pub deciding: Deciding,
    /// Where the cursor was when the question went up, by INDEX.
    ///
    /// The companion of `was`, and not a replacement: a target is the strong
    /// answer and survives the rows reordering, but `Target::Nothing` names
    /// every fact at once so it is no answer at all. `d` is reachable from a
    /// fact — it acts on the screen, not the row — and answering it then put
    /// the cursor on the question's own row 3.
    pub stood_at: usize,
    /// The row the cursor was on when the question went up.
    ///
    /// A question REPLACES the sheet, so answering it has to put the cursor
    /// back where the hand left it: nudge the scale, put it back, and the next
    /// arrow has to move the scale again. Without this the cursor came out of
    /// the question at the top of the sheet, four rows above the knob that had
    /// just been turned — read off the screen, not off a test.
    pub was: Option<Target>,
}

impl Asking {
    /// Whether this question answers itself when nobody does.
    #[must_use]
    pub const fn on_the_clock(&self) -> bool {
        matches!(self.deciding, Deciding::Preview(_))
    }

    /// How long is left, in whole seconds. Zero once it is over.
    #[must_use]
    pub fn left(&self, now: Instant) -> u64 {
        let gone = now.saturating_duration_since(self.since);
        PATIENCE.saturating_sub(gone).as_secs()
    }

    /// Whether it has run out.
    ///
    /// FALSE FOR A QUESTION WITH NO CLOCK, whatever the time says: a question
    /// that is waiting for a person is not a question that expires.
    #[must_use]
    pub fn over(&self, now: Instant) -> bool {
        self.on_the_clock() && now.saturating_duration_since(self.since) >= PATIENCE
    }
}

/// What the view wants done.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// Nothing.
    Nothing,
    /// Put this on the clipboard — what `RET` on a fact does everywhere.
    Copy(String),
    /// Close the view.
    Close,
    /// Ask the screens for something.
    Order(Box<ronoctua_display::Order>),
    /// Several changes, in the order they were made.
    ///
    /// For the one gesture that is about more than one screen: making a
    /// monitor the one the session opens on has to take that away from
    /// whichever had it.
    Orders(Vec<ronoctua_display::Order>),
    /// The screen a standing question was about is not there any more.
    ///
    /// It carries the connector so the door can name it, and whether the
    /// question that died had APPLIED anything — a preview leaves a transient
    /// change on the other screens that `Undo` is what discards, and a
    /// confirmation leaves nothing at all.
    ///
    /// The question used to vanish with no word: somebody answered "keep it"
    /// on a monitor that had been unplugged and the screen simply went back to
    /// the sheet, nothing written and nothing said. The flag reaches the
    /// SENTENCE too — see [`crate::display_words::went_away`], which has two.
    ///
    /// AND ONLY "KEEP IT" RAISES IT. "Put it back" and the clock running out
    /// both ask for the state the file already describes, which is what they
    /// get whether the screen is there or not: the answer is the picture
    /// coming back, and it comes back. "Keep it" is the one answer a vanished
    /// screen makes impossible, so it is the one that has to say so.
    Gone(String, bool),
}

/// The view's own state. Everything else is read from the snapshot.
#[derive(Debug, Clone, Default)]
pub struct View {
    level: Level,
    cursor: usize,
    /// What the cursor was ON, so it can be found again after the list
    /// reorders — a monitor plugged in changes the order under it.
    at: Option<Target>,
    asking: Option<Asking>,
    /// Whether the reading thread failed to start at all.
    ///
    /// Set once, by the door that opens the view. It is what tells the two
    /// silences apart — see `list`.
    no_thread: bool,
    /// Whether a reading has arrived yet.
    ///
    /// # THREE SILENCES, and the list had sentences for two
    ///
    /// "no reading thread", "no compositor" and "nothing plugged in" each got
    /// their own sentence, because each is a different fact with a different
    /// answer. The third one — NOT ASKED YET — had none, so it borrowed the
    /// compositor's: a `Snapshot::default()` has `compositor: false`.
    ///
    /// The window opens on one. Its reader's first act is a DEEP reading (a
    /// walk of `/sys/class/drm`, two niri connections, and a DDC/CI exchange
    /// per monitor — three tries at 50 ms apart for one that does not answer),
    /// while the compositor's `configure` arrives in milliseconds and the door
    /// paints. So the screens' own window opened, on a working machine, saying
    /// there was no compositor, and held that for a few hundred milliseconds.
    /// THE FOOTER IS THE OTHER HALF OF THIS, and getting it wrong is how the
    /// first version of this field made things worse rather than better: its
    /// reader starts at login and every hotplug refreshes it, so the snapshot
    /// on hand when somebody opens the view is CURRENT — and a fresh `View`
    /// saying otherwise drew this sentence as row 1 over a complete list, with
    /// the cursor on a heading and nothing lit at all, for as long as the
    /// deep reading took. `Footer::open_display` says so from what it already
    /// holds; the window's door has no such snapshot and waits.
    looked: bool,
    /// Whether this door's watch on `display.json` could not be installed.
    ///
    /// # Why that has to stop the text size, and only it
    ///
    /// Because the step is computed from the size this PROCESS is drawing at,
    /// and that number only moves when the watch fires (`toolkits::reload`).
    /// A door whose watch failed keeps the number it started with for ever,
    /// so its next press asks for a size derived from a baseline the file
    /// left behind — and `Order::Text` is the one order that takes no reading
    /// of its own, so nothing downstream can catch it. With two doors open
    /// that is somebody's chosen size jumping backwards, once per press.
    ///
    /// THE SCREENS ARE AT RISK IN A DIFFERENT WAY, and the sentence that used
    /// to stand here — "`Order::Keep` re-reads the file before it writes, so a
    /// blind door still keeps a fresh map" — is true about the MAP and wrong
    /// about the RECORD. `keep_for` clones this door's own `s.wanted` and
    /// changes one field, so the other six carry whatever this door was last
    /// shown; a fresh map does not help when the value put into it is stale.
    ///
    /// What answers it is on the other side, in `ronoctua_display`: a `Keep`
    /// is merged field by field against what this door was shown, so a knob it
    /// did not move is left to the disk. That makes this a RACE rather than a
    /// loss — and it is why `blind` gates only the size, which is computed
    /// before the order leaves and has no such merge behind it.
    blind: bool,
    /// The text size a key asked for, until the size itself catches up.
    ///
    /// # Why the row cannot just read the size
    ///
    /// Because the size is a PROCESS-WIDE atomic that only moves after a
    /// round trip: the key writes the file, inotify wakes the footer,
    /// `toolkits::reload` sets the atomic. Two presses inside that trip both
    /// computed their step from the same starting number, so the second one
    /// asked for the size the first had already asked for and nothing
    /// happened — holding the arrow moved the text one step.
    ///
    /// `power::View` carries a `pending` for exactly this, and for the same
    /// reason.
    text_pending: Option<u16>,
    /// The place a key asked for, and the screen it was about, until the
    /// reading catches up.
    ///
    /// # The same round trip the text has, for the same reason
    ///
    /// A place is written to `display.json`, said over the IPC, and only comes
    /// back in the next `Snapshot`. Two presses inside that trip both stepped
    /// from the same starting number, so the second asked for the number the
    /// first had already asked for — and `keep_for`'s equality guard, which
    /// compares against the equally stale `Wanted`, did not catch it: each
    /// repeat of a held arrow was a rewrite of the file and six IPC calls for
    /// a monitor that moved once per round trip instead of once per press.
    ///
    /// AND IT IS WHERE THE STEP COMES FROM. Before this the step was taken
    /// from `Screen::logical` — the PICTURE — while the row showed the
    /// SETTING, which is the split `place_said`, `scale_said` and `turn_said`
    /// were each written to close, left open on the one row that has both.
    place_pending: Option<(String, i32, i32)>,
}

/// Where a row's text begins and how much of the line it may use.
///
/// The same three lines the network, power and Bluetooth views use — and this
/// IS the fourth copy of them, which is worth saying plainly rather than
/// implying it is not. Where a shared one belongs is beside
/// `chrome::field_pair`, with the rest of the row arithmetic; moving it there
/// is a change to three views this deliverable did not touch, so what is done
/// here is to copy the answer rather than invent a different one.
#[must_use]
pub fn extent(columns: u16) -> (u16, u16) {
    let left = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
    let end = columns.saturating_sub(crate::chrome::FIELDS_LEAD + tokens::space::FILES_EDGERIGHT);
    (left, end.saturating_sub(left))
}

/// The width `key` builds its rows at.
///
/// A key must mean the same thing whatever the window's size, so the rows the
/// keyboard walks are built at one width and never at the door's. It is
/// `power.rs`'s rule and its test comes with it.
const DECIDING_WIDTH: u16 = 80;

struct Builder<'a> {
    out: Vec<Row>,
    left: u16,
    room: u16,
    cursor: usize,
    texts: &'a ronoctua_i18n::Texts,
}

impl Builder<'_> {
    fn piece(at: u16, text: String, color: tokens::Color) -> Field {
        Field {
            at,
            text,
            color,
            hit: FilesHit::Nothing,
            explain: None,
        }
    }

    fn heading(&mut self, key: &str) {
        // AIR FIRST, and never before the first title — `chrome::AIR`.
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                focused: false,
                heading: true,
                target: Target::Nothing,
                disabled: false,
            });
        }
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                self.texts.t(key),
                tokens::color::TEXT_GROUP,
            )],
            focused: false,
            heading: true,
            target: Target::Nothing,
            disabled: false,
        });
    }

    /// A title that is not a text key — a monitor's own name.
    fn heading_said(&mut self, text: String) {
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                focused: false,
                heading: true,
                target: Target::Nothing,
                disabled: false,
            });
        }
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                layout::elide_end(&text, self.room),
                tokens::color::TEXT_GROUP,
            )],
            focused: false,
            heading: true,
            target: Target::Nothing,
            disabled: false,
        });
    }

    /// A sentence the cursor SKIPS — prose about the screen, not a row.
    ///
    /// `heading: true` is what "the cursor skips it" is spelt as here; it is
    /// the field's own definition. It exists for the question, whose two
    /// sentences the cursor could walk onto: `↑` from "put it back" landed on
    /// the countdown, it lit in the focus colour so it looked chosen, and
    /// `RET` there copied the countdown to the clipboard while the clock went
    /// on running. On the one screen this view exists to rescue, that is the
    /// wrong place for a cursor to be able to go.
    ///
    /// AND EVERY SENTENCE IN THIS VIEW IS ONE. There used to be a second way
    /// of drawing prose that did NOT skip, and six sites reached for it —
    /// among them the list, which is the first thing the view draws. On a
    /// machine whose own fragment does not parse, the FIRST FRAME lit "The
    /// compositor could not read its configuration…" in the focus colour as
    /// though somebody had chosen it, `↑` from the first screen came back to
    /// it, and `RET` copied it. The rule is the arrangement's instruction's:
    /// a sentence is never a row.
    fn told(&mut self, text: String) {
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                layout::elide_end(&text, self.room),
                tokens::color::TEXT_DIM,
            )],
            focused: false,
            heading: true,
            target: Target::Nothing,
            disabled: false,
        });
    }

    /// [`Builder::told`], from a key.
    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.told(text);
    }

    /// A label and a value with nothing to do on `RET`.
    fn fact(&mut self, key: &str, value: String) {
        let label = self.texts.t(key);
        let focused = self.cursor == self.out.len();
        self.out.push(Row {
            fields: crate::chrome::field_pair(
                self.left,
                self.room,
                &label,
                &value,
                if focused {
                    tokens::color::STATE_ACCENT
                } else {
                    tokens::color::TEXT_LABEL_FACT
                },
                if focused {
                    tokens::color::STATE_ACCENT
                } else {
                    tokens::color::TEXT_READ_STRONG
                },
            ),
            focused,
            heading: false,
            target: Target::Nothing,
            disabled: false,
        });
    }

    /// A row something acts on. `disabled` draws it reachable and refusing.
    ///
    /// THE ROW THAT ACTS, and `fact` is the one that does not — the difference
    /// this view draws is a pair of colours, and it is the whole of what tells
    /// a person which lines answer a key. There used to be a third case here,
    /// `target == Target::Nothing`, drawn in the fact's colours; every one of
    /// the caller's targets is a real one, so the arm never ran and would have
    /// passed every test with its body deleted. A row with nothing to act on
    /// goes through `fact`.
    fn pair(&mut self, label: String, value: String, target: Target, disabled: bool) {
        debug_assert_ne!(
            target,
            Target::Nothing,
            "a row with nothing to act on is a `fact`, not a `pair`",
        );
        let focused = self.cursor == self.out.len();
        let (label_colour, value_colour) = match (focused, disabled) {
            (true, _) => (tokens::color::STATE_ACCENT, tokens::color::STATE_ACCENT),
            (false, true) => (tokens::color::TEXT_DIM, tokens::color::TEXT_DIM),
            (false, false) => (tokens::color::TEXT_LABEL_ACTS, tokens::color::TEXT_PRIMARY),
        };
        self.out.push(Row {
            fields: crate::chrome::field_pair(
                self.left,
                self.room,
                &label,
                &value,
                label_colour,
                value_colour,
            ),
            focused,
            heading: false,
            target,
            disabled,
        });
    }
}

impl View {
    /// A view on the list.
    #[must_use]
    pub fn fresh() -> Self {
        Self {
            // ONE, not zero: the first row of every level is a title.
            cursor: 1,
            ..Self::default()
        }
    }

    /// Which row the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The change standing on its own, if there is one.
    #[must_use]
    pub const fn asking(&self) -> Option<&Asking> {
        self.asking.as_ref()
    }

    /// The clock ran out on a provisional change: put it back.
    ///
    /// Called by whoever holds the second-by-second clock. It is not `key`'s
    /// business, because the whole point is that it happens with NOBODY
    /// pressing anything — a person looking at a screen they cannot read.
    pub fn ran_out(&mut self, now: Instant) -> Errand {
        let Some(asking) = self.asking.as_ref() else {
            return Errand::Nothing;
        };
        if !asking.over(now) {
            return Errand::Nothing;
        }
        self.answered();
        Errand::Order(Box::new(ronoctua_display::Order::Undo))
    }

    /// The keys this level teaches, as pieces for the hint line.
    ///
    /// # It builds the rows, and that is the point
    ///
    /// Three gates here started as tests over the SNAPSHOT — is there a
    /// screen, is it lit, is anything written down — and each of them was one
    /// state away from teaching a key nothing answers. The rows already know:
    /// a row that is a `heading` cannot be stood on and a row that is
    /// `disabled` refuses, and `key` reads exactly those two fields before it
    /// acts. So `RET`, `↑` and `↓` are asked of the rows themselves.
    ///
    /// AT `DECIDING_WIDTH`, like `key`, and for the reason written there: a
    /// key means the same thing whatever the window's size, and
    /// `the_rows_are_the_same_things_at_every_width` is what keeps that true.
    /// The cost is one row set per frame beside the one being drawn; the doors
    /// already build two.
    #[must_use]
    pub fn hint(
        &self,
        now: &Snapshot,
        moment: Instant,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        let asking = self.asking.is_some();
        let rows = self.rows(now, DECIDING_WIDTH, moment, texts);
        // A LEVEL WITH NOTHING TO ACT ON TEACHES NOTHING TO ACT WITH. `RET` is
        // refused on a heading and on a `disabled` row, so a level whose every
        // row is one of those answers it nowhere — an orphan sheet and an
        // arrangement of one screen are both two headings, and the line above
        // them taught three keys and answered none.
        let answers = rows.iter().any(|r| !r.heading && !r.disabled);
        // AND ONE ROW IS NOWHERE TO WALK. `walk` stops at both ends, so with a
        // single standable row `↑` and `↓` are two more keys that do nothing.
        let walkable = rows.iter().filter(|r| !r.heading).count() > 1;
        // WHAT WORKS HERE, AND ONLY THAT. `Where` matches the LEVEL, and a
        // level outlives the thing it is about: a sheet survives its monitor
        // being unplugged, and the list of one screen has an arrangement key
        // that opens nothing. Both were taught anyway. The radio's own `Where`
        // carries the owner's photograph of exactly this.
        //
        // The snapshot is what tells them apart, and `Where::shown` is a
        // `const fn` that cannot see one — so the second filter is here.
        let screen = self.here(now);
        // AND SOMETHING HAS TO BE TURNABLE for `←`/`→` to be worth teaching.
        // `WhileNothingIsAsked` was added to hide them under a question and
        // covered only that: on an orphan sheet, on an arrangement whose
        // second screen went away, and on a list with no reading thread, the
        // only rows are prose and both arrows fall to `Errand::Nothing` — a
        // laptop leaving a dock is the everyday one.
        // AND SOMETHING HAS TO BE TURNABLE for `←`/`→` to be worth teaching —
        // ASKED OF THE ROWS, exactly as `answers` and `walkable` are.
        //
        // It used to be a `match` on the level that named the conditions again
        // in its own words, and a parallel list of conditions is a list that
        // drifts. It did, twice, in opposite directions:
        //
        // - The list's arm forgot `blind` when `blind` was added, and taught
        //   the arrows on a row that refused them.
        // - The sheet's arm asked `lit || wanted.off || light.movable()` and
        //   so hid the arrows on a docked laptop with the lid shut — where the
        //   "opens focused here" row is deliberately still live, because it is
        //   disabled on `wanted.off` and NOT on dark. `→` there rewrote
        //   `display.json` and the fragment, taught nowhere.
        //
        // Deriving it from the rows cannot drift: a row that takes an arrow is
        // one whose target `turn` has an arm for, and it is `rows` that decides
        // what is `disabled`. The one list left to keep in step is `turns`,
        // which sits beside `turn` and names the same arms.
        let turnable = rows
            .iter()
            .any(|r| !r.heading && !r.disabled && Self::turns(&r.target));
        let works_here = |w: Where| match w {
            Where::WhileNothingIsAsked => turnable,
            // `d` acts on the SCREEN this level is about — so a level whose
            // screen is gone teaches it nothing, and neither does a screen
            // with nothing chosen for it, which is every screen on a machine
            // nobody has configured. That second half is the far commoner
            // one: a fresh laptop taught `d forget` and the key did nothing.
            Where::OnAMonitor => screen.is_some_and(|s| !s.wanted.says_nothing()),
            // `a` acts on the KNOB under the cursor, and the arrangement's
            // rows are knobs too — so it works on both levels, and the test
            // has to be the one for the level one is ON. Asking `on a screen
            // OR there is an arrangement` was neither: on an orphan sheet
            // with two other monitors lit it taught `a` over a level whose
            // only row is a sentence.
            //
            // AND A DARK SCREEN HAS NO KNOB TO HAND BACK, which is `automatic`'s
            // own first line: on a screen niri is not driving it refuses on
            // every row of the sheet, so `screen.is_some()` taught a letter
            // that does nothing on the one screen the view exists to rescue.
            //
            // NOR HAS A SCREEN NOBODY HAS CHOSEN ANYTHING FOR, which is the
            // far commoner half — the state of every machine on its first
            // morning. `a` HANDS BACK, and each of its four arms reduces to
            // "is this knob written down": with a default `Wanted` all four
            // answer nothing, on every row. It is the same defect `d` was
            // given `!says_nothing()` for, one letter over; `hands_back` is
            // the sharper test, because `d` is about the whole record and this
            // is about the four knobs `automatic` can reach.
            Where::OnAKnob => match &self.level {
                Level::Monitor(_) => screen.is_some_and(|s| s.lit() && hands_back(s)),
                // THE ARRANGEMENT'S ROWS ARE PLACES, one per lit screen, so
                // what `a` can hand back there is a place somebody chose.
                Level::Arrange => {
                    can_be_arranged(now)
                        && now
                            .screens
                            .iter()
                            .any(|s| s.lit() && s.wanted.position.is_some())
                }
                Level::List => false,
            },
            // And `m` opens a level that would have nothing in it.
            Where::OnTheList => can_be_arranged(now),
            _ => true,
        };
        // AND THE THREE THAT `Where::Anywhere` COVERS ARE ASKED OF THE ROWS.
        // `Where` is about the LEVEL and these three are about whether the
        // level has anything on it, which no `Where` can say.
        let works_now = |id: &str| match id {
            "display-enter" => answers,
            "display-up" | "display-down" => walkable,
            _ => true,
        };
        let here: Vec<(&str, &str)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(&self.level, asking))
            .filter(|(_, _, w)| works_here(*w))
            .filter(|(_, id, _)| works_now(id))
            .map(|(k, id, _)| (*k, *id))
            .collect();
        // `RET` MEANS THREE THINGS AND THE LINE SAID ONE. The table's word for
        // it is "open", which is true on the list and false everywhere else:
        // on a sheet it copies the value under the cursor, and on a question
        // it answers.
        //
        // `None` FOR THE UNMOVED MEANING, which is what the register is FOR:
        // `hint_of_with` draws any `Some` in `Register::Alternate`, and a key
        // drawn in "this means something else here" on every level says
        // nothing at all. The notification manager returns `None` the same way.
        // A LEVEL WITH NOTHING TO TEACH TEACHES THE WAY OUT. Two of them
        // exist — a sheet whose monitor went away, and an arrangement whose
        // second screen did — and on both the line came out EMPTY, which the
        // footer then drops entirely: a person read two sentences under a
        // blank strip, with `Esc` the only key that worked and no view in
        // this system teaching it.
        //
        // NOT IN `OF_THE_VIEW`, and deliberately: `Esc` works on every level
        // of every view, and a table row for it would put it on every line in
        // the shell — the radio, the network and the power view all leave it
        // out for that reason. It is drawn HERE, where the alternative is
        // nothing at all.
        //
        // AND A LEVEL WHERE THE ONLY KEYS ARE THE TWO THAT MOVE IS THE SAME
        // CASE, which `is_empty` alone does not see. `walkable` counts rows,
        // and two DISABLED rows are two walkable rows — so a list whose reader
        // thread never started teaches `down · up` over a pair of dim rows
        // that refuse everything, with `Esc` still the only key that works and
        // still the only one not on the line. Moving a cursor is not acting.
        let only_moving = here
            .iter()
            .all(|(_, id)| matches!(*id, "display-up" | "display-down"));
        if here.is_empty() || only_moving {
            // THE MOVEMENT KEYS STAY when there are any: they do work, and a
            // line that dropped them would be teaching less than is true.
            let mut out = here.clone();
            out.push(("ESC", ronoctua_i18n::key::DISPLAY_BACK));
            return crate::chrome::hint_of_with(&out, |_| None, texts);
        }
        let level = &self.level;
        crate::chrome::hint_of_with(
            &here,
            move |id| match id {
                // THE ARROWS MEAN LEFT AND RIGHT ON THE ARRANGEMENT. Every row
                // of that level is a monitor and the arrows slide it sideways
                // — the level's own instruction says so in prose — while the
                // line above it said "less" and "more", which is what they
                // mean on the four knobs of a sheet.
                //
                // THE ARRANGEMENT AND NOT THE PLACE ROW, though the place row
                // on a sheet does the same thing: this line is about a LEVEL,
                // and a sheet's other rows are knobs. A word that changed as
                // the cursor moved down a sheet would be a hint line that
                // flickers between two meanings, and the file manager's
                // `strong_meaning` — which does change with the focus — moves
                // with a SECTION and not with a row.
                //
                // ONE `match` AND NOT AN EARLY RETURN. The arrows' arms were
                // written as a `return` in front of the `RET` arm, which took
                // the arrangement out of the answer below: the level whose
                // every row is a place went back to the table's word, "open",
                // and `RET` there copies.
                "display-less" if matches!(level, Level::Arrange) => {
                    Some(ronoctua_i18n::key::DISPLAY_LEFTWARDS)
                }
                "display-more" if matches!(level, Level::Arrange) => {
                    Some(ronoctua_i18n::key::DISPLAY_RIGHTWARDS)
                }
                "display-enter" if asking => Some(ronoctua_i18n::key::DISPLAY_ENTER_ANSWERS),
                // THE LIST KEEPS THE TABLE'S OWN WORD, "open", and does not
                // name the copy. Two of its rows are values and `RET` on a
                // value copies it — but that is the fallback EVERY view in
                // this system has for a row that is not a door, and no view
                // names it on a level whose point is opening. What the
                // register is for is a meaning that MOVED.
                //
                // AND A LIST WITH NOTHING TO OPEN IS NOT THAT LEVEL. With no
                // reading thread, no compositor, or nothing plugged in, there
                // is no screen row and the arrangement row refuses — so the
                // only `RET` left is the copy, and "open" was a promise about
                // a row that is not drawn.
                "display-enter" if matches!(level, Level::List) && !now.screens.is_empty() => None,
                "display-enter" => Some(ronoctua_i18n::key::DISPLAY_ENTER_COPIES),
                _ => None,
            },
            texts,
        )
    }

    /// Lets go of a text size that was asked for and refused.
    ///
    /// `text_landed` drops it when the file says it arrived; this is the other
    /// end, and without it a refusal left the row showing a percentage the
    /// shell is not drawing at until the view was closed and reopened.
    pub const fn text_never_landed(&mut self) {
        self.text_pending = None;
    }

    /// The same, for a place that was asked for and refused.
    ///
    /// Without it the row would go on showing a place the file never took,
    /// for the life of the view, and the next arrow would step from it.
    ///
    /// BY THE ORDER'S OWN CONTENTS, and it took a round to get there: every
    /// knob that writes straight down emits an `Order::Keep` — the power
    /// toggle, the varying refresh, the session's screen, forgetting — so a
    /// refusal for any of those dropped a place that was still legitimately in
    /// flight. It is `never_mind`'s rule one order over: a refusal answers the
    /// change it was about and no other.
    pub fn place_never_landed(&mut self, at: &str, wanted: &ronoctua_display::fragment::Wanted) {
        let refused = wanted.position.map(|p| (p.x, p.y));
        if self
            .place_pending
            .as_ref()
            .is_some_and(|&(ref who, x, y)| who == at && refused == Some((x, y)))
        {
            self.place_pending = None;
        }
    }

    /// Says that there is no reading thread, so the view can explain itself.
    pub const fn without_a_reader(&mut self) {
        self.no_thread = true;
    }

    /// Says that a reading has arrived — see [`View::looked`].
    pub const fn saw_a_reading(&mut self) {
        self.looked = true;
    }

    /// Says that this door cannot see `display.json` change — see [`View::blind`].
    pub const fn without_a_watch(&mut self) {
        self.blind = true;
    }

    /// Drops a text size that has arrived — the file said what the key asked.
    ///
    /// Called from `settle`, which every path runs: a reading arriving, a key
    /// being pressed, the clock ticking. Nothing else has to remember to.
    fn text_landed(&mut self) {
        if self.text_pending == Some(tokens::text::percent()) {
            self.text_pending = None;
        }
    }

    /// Drops a place that has arrived — the reading says what the key asked.
    ///
    /// The place's half of `text_landed`, and called from the same place, for
    /// the same reason: every path runs `settle`.
    fn place_landed(&mut self, now: &Snapshot) {
        let Some((at, x, y)) = self.place_pending.as_ref() else {
            return;
        };
        // AND A SCREEN THAT WENT AWAY LETS GO TOO. A pending place for a
        // monitor that is not there any more can never land, and it would be
        // drawn on the next one to take that connector.
        let landed = now
            .screen(at)
            .is_none_or(|s| s.wanted.position.is_some_and(|p| (p.x, p.y) == (*x, *y)));
        if landed {
            self.place_pending = None;
        }
    }

    /// Puts the cursor back on what it was on, after the rows changed under it.
    ///
    /// BY TARGET AND NOT BY INDEX. A monitor plugged in or unplugged changes
    /// the order of the list while somebody is standing in it, and an index
    /// would move the cursor to whatever took that place — which is how a key
    /// meant for one screen reaches another.
    pub fn settle(&mut self, now: &Snapshot, rows: &[Row]) {
        self.text_landed();
        self.place_landed(now);
        if rows.is_empty() {
            self.cursor = 0;
            return;
        }
        if let Some(want) = self.at.clone()
            && let Some(n) = rows.iter().position(|r| r.target == want)
        {
            self.cursor = n;
            return;
        }
        // AND WITHOUT A NAME, BY POSITION — the power view's rule, which is
        // what `remember` falls back to. Read `remember` for why a cursor
        // sometimes has no name to be found by.
        self.cursor = self.cursor.min(rows.len() - 1);
        if rows[self.cursor].heading {
            self.cursor = first_row(rows);
        }
    }

    /// Writes down what the cursor is on, when that is a thing with a name.
    ///
    /// `Target::Nothing` IS NOT A NAME. It is what every fact, every note and
    /// every sentence carries, so a cursor remembered by it is found again at
    /// the FIRST of them: walking one row down and pressing anything put the
    /// cursor back on the sheet's first fact, and the rows below — the scale,
    /// the turn, the place — could not be reached at all. Measured on the
    /// screen before it was measured in a test: six `Down`s in a row drew two
    /// pictures, alternating.
    ///
    /// So a row that no target names is kept by POSITION, which is what
    /// `settle` does when there is nothing to look for. That is weaker — a
    /// reading that reorders the rows moves it — and it is the honest weaker
    /// thing, because a fact has no identity for the stronger one to use.
    fn remember(&mut self, target: &Target) {
        self.at = (*target != Target::Nothing).then(|| target.clone());
    }

    /// The mouse aims; the keys act. It only moves the cursor.
    pub fn clicked(&mut self, n: usize, rows: &[Row]) {
        let Some(row) = rows.get(n) else {
            return;
        };
        if row.heading {
            return;
        }
        self.cursor = n;
        self.remember(&row.target);
    }

    /// A key.
    pub fn key(
        &mut self,
        press: Press,
        now: &Snapshot,
        moment: Instant,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let rows = self.rows(now, DECIDING_WIDTH, moment, texts);
        self.settle(now, &rows);
        let target = rows
            .get(self.cursor)
            .map_or(Target::Nothing, |r| r.target.clone());
        let disabled = rows.get(self.cursor).is_some_and(|r| r.disabled);
        // A TITLE IS NOT A ROW, and the cursor can be on one: `first_row`
        // answers 0 when a level has no row the cursor may stand on, and two
        // levels have none — the arrangement of fewer than two screens, and a
        // sheet whose monitor went away. `RET` there copied the TITLE to the
        // clipboard, which is the same defect prose rows were taken off the
        // cursor for, one row up.
        let on_a_title = rows.get(self.cursor).is_some_and(|r| r.heading);

        let errand = match press {
            Press::Up | Press::Down => {
                self.walk(press == Press::Down, &rows);
                Errand::Nothing
            }
            Press::Escape => self.back(),
            Press::Less | Press::More if disabled => Errand::Nothing,
            Press::Less | Press::More => self.turn(&target, press == Press::More, now, moment),
            Press::Enter if disabled || on_a_title => Errand::Nothing,
            Press::Enter => self.enter(&target, now, &rows),
            // NOTHING NEW WHILE A QUESTION STANDS. `d` used to overwrite the
            // standing one with its own, and a provisional change replaced
            // that way can never go back: the new question has no clock, so
            // `ran_out` never fires, and both its answers return without an
            // `Undo`. A screen turned sideways provisionally and then `d`
            // stayed sideways for ever, with nothing on it saying so.
            //
            // The hint already hid both letters while asking (`Where::shown`)
            // — this is the keyboard agreeing with the hint.
            Press::Automatic | Press::Forget if self.asking.is_some() => Errand::Nothing,
            // `a` IS NOT GATED BY THE ROW'S REFUSAL, and it was. `disabled`
            // means the ARROWS have nowhere to go — one mode, one rate, a
            // dark screen — and `a` means something else entirely: hand the
            // knob back to the compositor. The two are only the same when
            // nothing is written down, and when something is, the row that
            // refuses the arrows is exactly the row `a` is for.
            //
            // Pick 1280x720 on a monitor that offers one rate for it, keep it,
            // and the Rate row is drawn refusing with a mode pinned behind it
            // — `a` there was swallowed, silently. What decides now is
            // `would_move`, in `provisional`: a hand-back that would change
            // nothing raises nothing.
            Press::Automatic => self.automatic(&target, now, moment),
            // `d` IS GATED BY THE LEVEL, because that is what IT acts on: the
            // whole screen, whatever row the cursor happens to be resting on.
            // Gating it by the row meant the letter worked from a fact and was
            // swallowed on the row below it — on a one-screen laptop, where
            // "Driven" is drawn refusing, `d` did nothing there and everything
            // one row down, with no sentence either way.
            Press::Forget if !matches!(self.level, Level::Monitor(_)) => Errand::Nothing,
            Press::Forget => self.forget(now, moment),
            // AND `m` REFUSES WHERE THE ROW REFUSES. It only checked the
            // level, so on a machine with one screen the letter reached a
            // level whose own first sentence is that there is nothing there,
            // while `RET` on the row for it was drawn refusing.
            Press::Arrange => {
                if matches!(self.level, Level::List) && can_be_arranged(now) {
                    self.go_to(Level::Arrange);
                }
                Errand::Nothing
            }
        };
        // AND IT SETTLES ON THE WAY OUT TOO.
        //
        // A key that changes WHAT THE ROWS ARE — entering a sheet, putting a
        // question up, answering one — leaves `cursor` pointing into the list
        // it just left, and the frame the door draws immediately after this
        // call is drawn with that index. Settling only on the way in made the
        // picture one keypress behind: answering the question lit a row four
        // above the knob the hand had just turned, and it only jumped back
        // when something else was pressed.
        let rows = self.rows(now, DECIDING_WIDTH, moment, texts);
        self.settle(now, &rows);
        errand
    }

    /// One row up or down, skipping the titles.
    fn walk(&mut self, forward: bool, rows: &[Row]) {
        let mut n = self.cursor;
        loop {
            let next = if forward {
                if n + 1 >= rows.len() {
                    break;
                }
                n + 1
            } else {
                if n == 0 {
                    break;
                }
                n - 1
            };
            n = next;
            if !rows[n].heading {
                self.cursor = n;
                self.remember(&rows[n].target);
                return;
            }
        }
    }

    /// Takes the standing question down and gives the cursor its row back.
    ///
    /// THE FIVE WAYS OUT ARE ONE FUNCTION: `Esc`, "put it back", "keep it",
    /// the clock running out with nobody there, and a refusal arriving for
    /// the change the question was about (`never_mind`). Each used to clear
    /// `asking` on its own, and the fifth is why: it was added a round after
    /// this sentence was written, and had this been five places it would have
    /// been a fifth place to forget the cursor in.
    fn answered(&mut self) -> Option<Asking> {
        let asking = self.asking.take()?;
        self.at = asking.was.clone();
        // AND THE INDEX WHEN THERE IS NO NAME. `settle` falls back to the
        // cursor's position when `at` is `None`, and the position it would
        // fall back to is the question's own row — see `Asking::stood_at`.
        if asking.was.is_none() {
            self.cursor = asking.stood_at;
        }
        Some(asking)
    }

    /// Takes a standing question down because the change never happened.
    ///
    /// NOT AN ANSWER: nothing is undone, because there is nothing to undo —
    /// the preview was refused.
    ///
    /// It restores what the cursor was ON — by target where the row had one,
    /// by index where it did not (`Asking::stood_at`) — but it has no ROWS, so
    /// the caller is what settles that against the list that comes back. See
    /// the `Done` arm of `surface::display_event`, and its twin in the window
    /// door.
    ///
    /// ANSWERS WHETHER IT TOOK ONE DOWN, which is whether the rows changed.
    /// Both doors settled the cursor and pulled the page to it on every
    /// refusal — including the ones that matched no question, which are most
    /// of them: `Esc` on a question and then a wheel down the sheet, and the
    /// refusal arriving for the already-answered `Try` yanked the page back.
    /// A DAEMON'S ARRIVAL DOES NOT FOLLOW THE CURSOR is the rule (CLAUDE.md
    /// states it in Portuguese; this is what it says), and it is the one
    /// the comment beside those calls cites.
    #[must_use]
    pub fn never_mind(&mut self, at: &str, ask: Ask) -> bool {
        // AND ONLY THE QUESTION IT BELONGS TO. A refusal arrives after a round
        // trip, and by then the question it was about may be gone and another
        // one up: `→`, `Esc`, `→` again, and the FIRST refusal would clear the
        // second question with no `Undo` — a change that WAS applied standing
        // for ever with nothing saying so, which is the end state `back` and
        // `PutItBack` are arranged to prevent.
        //
        // THE CONNECTOR IS NOT ENOUGH, and the comment here used to argue that
        // it was: "a question takes the whole region, so the second cannot be
        // raised until the first is answered". True, and beside the point —
        // answering a question does not wait for its `Order::Try` to come
        // back, and the two clocks are independent. The sequence above is two
        // questions about the SAME screen inside one round trip, which is the
        // case the argument said could not happen.
        //
        // So the ASK ties them too, and an order carries it: `→` on the scale
        // twice from either side of an `Esc` asks for two different numbers.
        // WHAT IS LEFT is two questions asking for the identical thing on the
        // identical screen, where the first is refused and the second is not —
        // which needs a compositor that fails once and then succeeds. There
        // the second question comes down without its `Undo`; nothing this
        // module can see tells the two apart, and saying so is better than a
        // guard that reads as though it did.
        if self
            .asking
            .as_ref()
            .is_some_and(|a| a.at == at && a.deciding == Deciding::Preview(ask))
        {
            self.answered();
            return true;
        }
        false
    }

    /// `Esc`: the question first, then one level, then out.
    fn back(&mut self) -> Errand {
        // A STANDING QUESTION IS WHAT `Esc` ANSWERS, and it answers NO — the
        // rule the pairing question and the format buffer already follow. A
        // person who cannot read the screen presses it to get the old one
        // back, and that has to be what it does.
        match self.answered() {
            // See `Target::PutItBack` for why a question with no clock has
            // nothing to undo.
            Some(a) if !a.on_the_clock() => return Errand::Nothing,
            Some(_) => return Errand::Order(Box::new(ronoctua_display::Order::Undo)),
            None => {}
        }
        if matches!(self.level, Level::List) {
            return Errand::Close;
        }
        self.go_to(Level::List);
        Errand::Nothing
    }

    fn go_to(&mut self, level: Level) {
        self.level = level;
        self.cursor = 1;
        self.at = None;
    }

    /// The screen this level is about, if it is about one.
    fn here<'a>(&self, now: &'a Snapshot) -> Option<&'a Screen> {
        match &self.level {
            Level::Monitor(at) => now.screen(at),
            _ => None,
        }
    }
}

/// The first row a cursor may stand on.
fn first_row(rows: &[Row]) -> usize {
    rows.iter().position(|r| !r.heading).unwrap_or(0)
}

// ---------------------------------------------------------------------
// The sentences
// ---------------------------------------------------------------------
//
// Each is a function and not an inline `format!`, for the reason the network
// view's own words module gives: a number that is drawn in two places has to
// be drawn the same way in both, and the decimal mark is the LANGUAGE's.

/// `1920x1200`, with the multiplication sign the design uses.
#[must_use]
pub fn size_said(mode: ronoctua_niri::Mode) -> String {
    format!("{}{}{}", mode.width, tokens::mark::TIMES, mode.height)
}

/// `59,9 Hz`, with the mark the language writes a decimal with.
#[must_use]
pub fn rate_said(mode: ronoctua_niri::Mode, texts: &ronoctua_i18n::Texts) -> String {
    // ONE DECIMAL, because that is what tells 143,9 from 144,0 — and the
    // rounding is `Mode::hertz`'s, never `rate`'s: this is for reading.
    let hz = format!("{:.1}", mode.hertz()).replace('.', &texts.decimal_mark().to_string());
    texts.t_with(ronoctua_i18n::key::DISPLAY_VALUE_RATE, &[("hz", &hz)])
}

/// A scale as a percentage, and — when they differ — the one on the glass.
///
/// # Why two of them, like the turn
///
/// Because they are two different facts and the row used to show one while the
/// arrow moved the other. The SETTING is what `←`/`→` step through and what is
/// written down; the PICTURE is what niri resolved, which is the setting when
/// there is one and its own choice when there is not. `turn_said` was given
/// this treatment a round earlier and the scale did not: it showed the picture
/// alone, so on a panel nobody has configured the row read "100%" over a
/// setting that says "you choose", and a failed configuration reload — which
/// the list already draws a sentence for — could put the two further apart
/// with nothing saying so.
#[must_use]
pub fn scale_said(set: Option<f64>, picture: Option<f64>, texts: &ronoctua_i18n::Texts) -> String {
    let percent = |v: f64| {
        // THE NUMBER IS niri's AND THE PERCENTAGE IS OURS: 1.25 reads as 125%,
        // which is the word a person uses. Rounded to whole percent because a
        // scale of 1.333 is a number nobody typed.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let whole = (v * 100.0).round().max(0.0) as u32;
        texts.t_with(
            ronoctua_i18n::key::DISPLAY_VALUE_PERCENT,
            &[("percent", &whole.to_string())],
        )
    };
    let named = |v: Option<f64>| {
        v.map_or_else(
            || texts.t(ronoctua_i18n::key::DISPLAY_VALUE_AUTOMATIC),
            percent,
        )
    };
    let same = match (set, picture) {
        (None, None) => true,
        (Some(a), Some(b)) => (a - b).abs() < 0.001,
        _ => false,
    };
    if same {
        return named(set);
    }
    texts.t_with(
        ronoctua_i18n::key::DISPLAY_SCALE_AND_PICTURE,
        &[("set", &named(set)), ("picture", &named(picture))],
    )
}

/// How the picture is turned, and — when they differ — how it was ASKED to be.
///
/// # Why two of them
///
/// Because they are two different facts and the row used to show one while the
/// arrow moved the other. `Screen::turn` is the PICTURE's transform, which
/// niri composes from ours and the panel's own (and forces one of its own on
/// the nested backend); `Wanted::transform` is the setting, which is what the
/// arrows step through and what gets written down. On a panel that declares an
/// orientation the row read "a quarter turn" while the ring stood at index 0,
/// so the left arrow did nothing and the right one applied a turn on top of
/// the panel's.
///
/// So the row says the SETTING — the thing the key moves — and adds the
/// picture only when the two are not the same, which on an ordinary monitor is
/// never.
///
/// # And why the key is a `match` and not a `format!`
///
/// It was `texts.t(&format!("display-turn-{}", turn.id()))`, which puts the
/// key outside every guarantee the system has: invariant 9's "a key that does
/// not exist is a compile error" does not reach it, `labels-of-a-view-are-
/// distinct` reads `key::([A-Z0-9_]+)` and so never compared these eight with
/// the rest of the view, and a ninth `Transform` would have drawn
/// `[display-turn-…]` on the screen with every check green. The `match` is
/// exhaustive, so the ninth variant is a build failure instead.
#[must_use]
pub fn turn_said(set: Transform, picture: Transform, texts: &ronoctua_i18n::Texts) -> String {
    let named = |t: Transform| {
        texts.t(match t {
            Transform::Normal => ronoctua_i18n::key::DISPLAY_TURN_NORMAL,
            Transform::R90 => ronoctua_i18n::key::DISPLAY_TURN_ROTATED_90,
            Transform::R180 => ronoctua_i18n::key::DISPLAY_TURN_ROTATED_180,
            Transform::R270 => ronoctua_i18n::key::DISPLAY_TURN_ROTATED_270,
            Transform::Flipped => ronoctua_i18n::key::DISPLAY_TURN_FLIPPED,
            Transform::Flipped90 => ronoctua_i18n::key::DISPLAY_TURN_FLIPPED_90,
            Transform::Flipped180 => ronoctua_i18n::key::DISPLAY_TURN_FLIPPED_180,
            Transform::Flipped270 => ronoctua_i18n::key::DISPLAY_TURN_FLIPPED_270,
        })
    };
    if set == picture {
        return named(set);
    }
    texts.t_with(
        ronoctua_i18n::key::DISPLAY_TURN_AND_PICTURE,
        &[("set", &named(set)), ("picture", &named(picture))],
    )
}

/// Where a screen sits, and — when they differ — where the picture actually is.
///
/// The third of the three that take a SETTING and a PICTURE, for the reason
/// `turn_said` and `scale_said` give: the row used to show one while the arrow
/// moved the other. Here the cost of not doing it was sharper, because the
/// setting is `None` on every machine nobody has placed a monitor on: the
/// `display-value-automatic` branch was DEAD (both callers pass a lit screen's
/// `logical`, which is always `Some`), so the one sentence that says "the
/// compositor places it" was never drawn — and `a` on the row, which hands the
/// place back, did nothing with nothing on screen to say why.
#[must_use]
pub fn place_said(
    set: Option<(i32, i32)>,
    picture: Option<(i32, i32)>,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let named = |at: Option<(i32, i32)>| {
        at.map_or_else(
            || texts.t(ronoctua_i18n::key::DISPLAY_VALUE_AUTOMATIC),
            |(x, y)| {
                texts.t_with(
                    ronoctua_i18n::key::DISPLAY_VALUE_PLACE,
                    &[("x", &x.to_string()), ("y", &y.to_string())],
                )
            },
        )
    };
    if set == picture {
        return named(set);
    }
    texts.t_with(
        ronoctua_i18n::key::DISPLAY_PLACE_AND_PICTURE,
        &[("set", &named(set)), ("picture", &named(picture))],
    )
}

/// Writes one ask into a `Wanted` — the one place that knows how.
///
/// TWO CALLERS: "keep it", which writes down what was previewed, and the
/// power toggle of a DARK screen, which is written straight down because there
/// is no picture to preview. They were one arm and a copy of it for one
/// commit.
fn write_the(w: &mut ronoctua_display::fragment::Wanted, ask: Ask) {
    match ask {
        Ask::On => w.off = false,
        Ask::Off => w.off = true,
        Ask::Mode(None) => w.mode = None,
        // THE MODE THAT WAS ASKED FOR, like every arm around it. Reading it
        // back looks safer and is not: `Screen::current` is an `Option`, so a
        // reading that has not caught up — or one where niri reports no
        // current mode — wrote down the mode the person had just changed AWAY
        // from, or "the compositor chooses".
        Ask::Mode(Some(m)) => {
            w.mode = Some(ronoctua_display::fragment::Chosen {
                width: m.width,
                height: m.height,
                rate: m.refresh,
            });
        }
        Ask::Scale(chosen) => w.scale = chosen,
        // THE ASKED TRANSFORM, never `Screen::turn`: that one is the
        // PICTURE's, with the panel's own orientation already composed into
        // it. Writing it back would add the panel's turn to ours every time.
        Ask::Transform(t) => w.transform = t,
        Ask::Position(spot) => {
            w.position = spot.map(|(x, y)| ronoctua_display::fragment::Spot { x, y });
        }
        Ask::Vrr { on, on_demand } => {
            w.vary = on.then_some(if on_demand {
                ronoctua_display::fragment::Vary::OnDemand
            } else {
                ronoctua_display::fragment::Vary::Always
            });
        }
    }
}

/// Whether asking for this would change anything on this screen.
///
/// Read `provisional`, which is the one caller and the reason it exists.
///
/// THE COMPARISON IS AGAINST WHAT THE ROW SHOWS, which for the picture is the
/// reading and for a "you choose" is the SETTING: `Scale(None)` moves only
/// when a number is written down, however the compositor has since resolved
/// it, because handing back a decision nobody had taken changes nothing.
fn would_move(s: &Screen, ask: Ask) -> bool {
    match ask {
        Ask::On => !s.lit(),
        Ask::Off => s.lit(),
        Ask::Mode(None) => s.wanted.mode.is_some(),
        Ask::Mode(Some(m)) => s.mode().is_none_or(|now| {
            now.width != m.width || now.height != m.height || m.refresh.is_some()
        }),
        Ask::Scale(None) => s.wanted.scale.is_some(),
        Ask::Scale(Some(v)) => s
            .wanted
            .scale
            .or_else(|| s.logical.map(|l| l.scale))
            .is_none_or(|now| (now - v).abs() > 0.001),
        Ask::Transform(t) => t != s.wanted.transform,
        // NEITHER OF THESE IS ASKED ANY MORE, and they are kept because the
        // match is over `Ask` and has to be exhaustive. Moving a monitor
        // sideways cannot leave a screen nobody can read, so the place row
        // writes straight down through `keep_for` — which carries the same
        // rule for the written-down half, by comparing the `Wanted` it built
        // against the one on file. They are the honest comparisons for the day
        // a place question comes back, which is why they say what they mean
        // rather than answering `true`.
        Ask::Position(None) => s.wanted.position.is_some(),
        Ask::Position(Some((x, y))) => s.logical.is_none_or(|l| l.x != x || l.y != y),
        // NOT ASKED PROVISIONALLY EITHER — varying a refresh rate cannot blind
        // a screen. Answering `true` keeps this exhaustive without claiming to
        // know a comparison nobody makes.
        Ask::Vrr { .. } => true,
    }
}

/// Whether `a` has anything to hand back on this screen.
///
/// THE FOUR COMPARISONS `automatic` REACHES, and no others: a mode (which the
/// resolution row and the rate row share), a scale, a turn and a place. `Wanted` carries
/// more than that — the power, the session's screen, the varying refresh — and
/// none of those is a knob `a` touches, which is why this is not
/// `!says_nothing()`.
///
/// It is the same comparison `would_move` makes for each of them, collected:
/// `Ask::Mode(None)` and `Ask::Scale(None)` ask whether one is written down,
/// `Ask::Transform(Normal)` whether the turn is not already the default, and
/// the place goes through `keep_for`'s equality guard.
fn hands_back(s: &Screen) -> bool {
    s.wanted.mode.is_some()
        || s.wanted.scale.is_some()
        || s.wanted.transform != Transform::Normal
        || s.wanted.position.is_some()
}

/// Whether there is an arrangement to speak of.
///
/// ONE RULE, and FIVE places ask it: the arrangement row's own refusal, the
/// `m` key, and the three halves of the hint line that decide whether a level
/// with no arrangement teaches `m`, `a` and the arrows. The row's refusal, the
/// row's value and the level's own sentence each counted differently — all the
/// screens, the lit ones, the lit ones — so two screens with one off gave a
/// row that was reachable, reading "one screen", opening a level whose first
/// sentence is that there is nothing here. The row's VALUE and the level's own
/// sentence still count for themselves, because each wants a different thing
/// from the count; what they share is the question.
#[must_use]
pub fn can_be_arranged(now: &Snapshot) -> bool {
    now.screens.iter().filter(|s| s.lit()).count() >= 2
}

/// A LADDER AND NOT A SLIDER, because a scale is not a continuum to a person:
/// it is "the same", "half again", "twice". niri accepts anything in
/// `0.1..=10` and the ones between these are not sizes anybody asks for by
/// name.
///
/// # Why this is a constant here and the TEXT size is a token
///
/// They scale different things, and the difference is not that one belongs to
/// a protocol — niri takes anything in `0.1..=10`, so these seven rungs are as
/// much a choice as `font.scale`'s three numbers are. It is WHOSE choice.
/// `font.scale` moves the size of OUR design, which is what invariant 9 is
/// about: a value the design system owns and a token is where it lives. This
/// moves the COMPOSITOR's output, and what it does to a pixel is the
/// monitor's business — the list is which of the compositor's numbers are
/// worth offering by name, which is the same kind of decision `power`'s charge
/// presets are, and they sit in the same place.
pub const LADDER: [f64; 7] = [1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];

/// The next scale up or down the ladder. `None` is the compositor's choice,
/// and it sits BELOW the bottom rung: stepping down from the smallest hands
/// the decision back rather than stopping.
#[must_use]
pub fn next_scale(now: Option<f64>, up: bool) -> Option<Option<f64>> {
    let Some(s) = now else {
        // ALREADY AT THE COMPOSITOR'S CHOICE: down is where one already is.
        return up.then(|| Some(LADDER[0]));
    };
    // THE NEXT RUNG PAST WHERE ONE IS STANDING, and not "the rung one is on,
    // plus one". The difference is every value the ladder does not carry, and
    // niri's own choice for a panel is routinely one of those — 1,6 on a
    // 13-inch 4K. `position(…).unwrap_or(0)` filed all of them at the BOTTOM,
    // so "more" from 1,6 answered 1,25, which is less.
    //
    // The epsilon is the same 0,001 the old lookup used, and it is what keeps
    // a value that IS on a rung from finding itself: without it a float that
    // is a hair under 2.0 would answer 2.0 to "more", which does not move.
    const CLOSE: f64 = 0.001;
    // TWO `Option`S, AND THEY MEAN DIFFERENT THINGS. The outer one is "there
    // is a step to take"; the inner one is the scale, where `None` is the
    // compositor's own choice. One `Option` cannot say both, and it used to
    // answer the CURRENT scale at the top rung — a question about a change
    // that had not happened. See `step`, which carries the same fix.
    if up {
        return LADDER.iter().copied().find(|r| *r > s + CLOSE).map(Some);
    }
    // AND BELOW THE BOTTOM RUNG IS THE COMPOSITOR'S CHOICE, which is what
    // makes this a ladder somebody can climb all the way off. Below THAT there
    // is nothing, which is the outer `None`.
    Some(LADDER.iter().copied().rev().find(|r| *r < s - CLOSE))
}

/// The distinct sizes a monitor offers, largest first.
///
/// DISTINCT, because a monitor lists one entry per size AND rate — a 4K panel
/// offers thirty-odd modes and perhaps eight sizes. Walking the modes with an
/// arrow would make somebody press it four times to change the resolution
/// once, passing through rates they did not ask about.
#[must_use]
pub fn sizes_of(screen: &Screen) -> Vec<(u16, u16)> {
    let mut out: Vec<(u16, u16)> = screen.modes.iter().map(|m| (m.width, m.height)).collect();
    out.sort_unstable_by_key(|(w, h)| (std::cmp::Reverse(*w), std::cmp::Reverse(*h)));
    out.dedup();
    out
}

/// The rates that one size offers, highest first.
#[must_use]
pub fn rates_of(screen: &Screen, size: (u16, u16)) -> Vec<ronoctua_niri::Mode> {
    let mut out: Vec<ronoctua_niri::Mode> = screen
        .modes
        .iter()
        .filter(|m| (m.width, m.height) == size)
        .copied()
        .collect();
    out.sort_unstable_by_key(|m| std::cmp::Reverse(m.refresh_rate));
    out
}

/// One step along a list, stopping at both ends.
fn step<T: Clone>(list: &[T], at: Option<usize>, up: bool) -> Option<T> {
    let n = at?;
    // NONE AT BOTH ENDS, and it used to be `None` going back and THE SAME
    // ELEMENT going forward. That asymmetry is not cosmetic here: every caller
    // hands the answer to `provisional`, so at the last rung the arrow applied
    // the value that was already applied, replaced the sheet with a
    // fifteen-second question saying the picture had changed, and sent an
    // `Undo` for it. Exactly the defect `Target::Power` carried, in four more
    // places.
    let next = if up {
        n.checked_sub(1)?
    } else {
        n.checked_add(1).filter(|m| *m < list.len())?
    };
    list.get(next).cloned()
}

impl View {
    /// Every line of the level the view is on.
    #[must_use]
    pub fn rows(
        &self,
        now: &Snapshot,
        columns: u16,
        moment: Instant,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let (left, room) = extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            texts,
        };
        // A STANDING QUESTION IS THE WHOLE SCREEN. It is not a row among the
        // others: whoever is reading it may be reading a picture that is
        // wrong, and the one thing that must be findable is the way back.
        if let Some(asking) = &self.asking {
            self.question(&mut b, asking, moment);
            return b.out;
        }
        match &self.level {
            Level::List => self.list(&mut b, now),
            Level::Arrange => self.arrangement(&mut b, now),
            Level::Monitor(at) => match now.screen(at) {
                Some(s) => self.sheet(&mut b, s, now),
                None => {
                    // THE CONNECTOR IS THE TITLE, which is what a sheet's
                    // title is — a real one carries the monitor's description.
                    // It used to borrow the LIST's title, so two levels read
                    // "Screens" and nothing on the glass said which one you
                    // were on, while `Esc` still went back one.
                    b.heading_said(at.clone());
                    b.note(ronoctua_i18n::key::DISPLAY_WENT_AWAY);
                }
            },
        }
        b.out
    }

    /// The timed question.
    fn question(&self, b: &mut Builder<'_>, asking: &Asking, moment: Instant) {
        // THE TITLE IS THE QUESTION'S OWN. "Keeping this?" is the preview's,
        // and over a question that has changed nothing it asks about
        // something that did not happen.
        b.heading(match asking.deciding {
            Deciding::Preview(_) => ronoctua_i18n::key::DISPLAY_HEADING_KEEPING,
            Deciding::Whether => ronoctua_i18n::key::DISPLAY_HEADING_FORGETTING,
        });
        // TWO ROWS OF PROSE EITHER WAY, so the answers sit on the same line
        // whichever question this is: `provisional` and `forget` both put the
        // cursor on row 3, and a question whose shape moved under them would
        // put it on a sentence. The cursor cannot stand on either — see
        // `Builder::told`, which is now the only way this view draws prose.
        let (back, keep) = match &asking.deciding {
            Deciding::Preview(_) => {
                let what = b.texts.t(asking.what);
                b.told(b.texts.t_with(
                    ronoctua_i18n::key::DISPLAY_ASK,
                    &[("what", &what), ("screen", &asking.at)],
                ));
                b.told(b.texts.t_with(
                    ronoctua_i18n::key::DISPLAY_ASK_SECONDS,
                    &[("seconds", &asking.left(moment).to_string())],
                ));
                (
                    ronoctua_i18n::key::DISPLAY_PUT_IT_BACK,
                    ronoctua_i18n::key::DISPLAY_KEEP_IT,
                )
            }
            // IN THE FUTURE TENSE, because nothing has happened. `display-ask`
            // reads "{ $what } on { $screen }" — a report of a change already
            // applied, which is what the other nature is and this one is not.
            //
            // AND NO COUNTDOWN, WHICH IT SAYS: a question that waits for a
            // person and one that answers itself look the same on the glass
            // otherwise, and only one of them can be walked away from.
            Deciding::Whether => {
                // `asking.what` FOR THIS NATURE TOO. The arm named the key
                // directly while the field carried the same one, so the two
                // could drift — and for a round the field carried a key
                // nothing in the system could render.
                b.told(b.texts.t_with(asking.what, &[("screen", &asking.at)]));
                b.told(b.texts.t(ronoctua_i18n::key::DISPLAY_ASK_NOTHING_YET));
                (
                    ronoctua_i18n::key::DISPLAY_LEAVE_IT,
                    ronoctua_i18n::key::DISPLAY_FORGET_IT,
                )
            }
        };
        // THE WAY BACK IS THE FIRST ROW AND THE CURSOR IS ON IT. A question
        // that appears by itself answers NO to `RET` — the rule the pairing
        // question and the disk formatting already carry, and here it is the
        // difference between a screen somebody can read and one they cannot.
        b.pair(b.texts.t(back), String::new(), Target::PutItBack, false);
        b.pair(b.texts.t(keep), String::new(), Target::KeepIt, false);
    }

    /// Every screen, one row each, and what belongs to all of them.
    fn list(&self, b: &mut Builder<'_>, now: &Snapshot) {
        b.heading(ronoctua_i18n::key::DISPLAY_HEADING_SCREENS);
        if self.no_thread {
            // THE THREAD AND THE COMPOSITOR ARE DIFFERENT FACTS. Both used to
            // say "there is no compositor to ask": the reading thread failing
            // to start (a door that will never show anything) read the same
            // as niri not answering (a door that would work if it did).
            b.note(ronoctua_i18n::key::DISPLAY_NO_THREAD);
        } else if !self.looked {
            // AND SO IS "NOT ASKED YET" — see [`View::looked`]. This one is
            // the ordinary first frame of the window, not a failure at all.
            b.note(ronoctua_i18n::key::DISPLAY_READING);
        } else if !now.compositor {
            b.note(ronoctua_i18n::key::DISPLAY_NO_COMPOSITOR);
        } else if now.screens.is_empty() {
            b.note(ronoctua_i18n::key::DISPLAY_NOTHING_HERE);
        }
        if now.config_failed {
            b.told(b.texts.t(ronoctua_i18n::key::DISPLAY_CONFIG_FAILED));
        }
        // AND OUR OWN FILE, WHICH IS A DIFFERENT FACT. When `display.json`
        // will not parse the reading falls back to the default, so every row
        // below draws as if nothing had ever been chosen — the settings LOOK
        // gone. They are not: the file is intact and the crate refuses to
        // write over it. Without this line the person sees the loss and not
        // the reason, which is the silent failure the line above exists to
        // prevent, one file over.
        if now.settings_failed {
            b.told(b.texts.t(ronoctua_i18n::key::DISPLAY_SETTINGS_FAILED));
        }
        for s in &now.screens {
            let focused = now.focused.as_deref() == Some(s.at.as_str());
            b.pair(
                if focused {
                    b.texts
                        .t_with(ronoctua_i18n::key::DISPLAY_THIS_ONE, &[("name", &s.at)])
                } else {
                    s.at.clone()
                },
                summary_of(s, b.texts),
                Target::Screen(s.at.clone()),
                false,
            );
        }

        b.heading(ronoctua_i18n::key::DISPLAY_HEADING_EVERY);
        b.pair(
            b.texts.t(ronoctua_i18n::key::DISPLAY_ARRANGEMENT),
            arrangement_said(now, b.texts),
            Target::Arrange,
            // LIT ONES, which is what the level itself counts and what its
            // value already said. Counting all of them made the row reachable
            // on a machine with two screens one of which is off: the value
            // read "one screen" and `RET` opened a level saying there was
            // nothing to arrange.
            !can_be_arranged(now),
        );
        b.pair(
            b.texts.t(ronoctua_i18n::key::DISPLAY_TEXT_SIZE),
            if self.no_thread {
                // AND IT SAYS WHY, like every other refusing row: the reason
                // is a note several rows above under a different heading, and
                // a row that refuses without carrying its own is a key that
                // does nothing for a reason nobody can read.
                b.texts.t(ronoctua_i18n::key::DISPLAY_NO_THREAD_SHORT)
            } else if self.blind {
                // AND THE OTHER REFUSAL SAYS WHY TOO, which it did not: the
                // `disabled` flag below tested both reasons and this branch
                // tested one, so the refusal that was ADDED for a reason
                // shipped as a dim row reading like any other value.
                b.texts.t(ronoctua_i18n::key::DISPLAY_NO_WATCH)
            } else {
                // WHAT WAS ASKED FOR while it is on its way — otherwise the
                // row reads the old number for a whole round trip and the key
                // looks as though it did nothing.
                b.texts.t_with(
                    ronoctua_i18n::key::DISPLAY_VALUE_PERCENT,
                    &[(
                        "percent",
                        &self
                            .text_pending
                            .unwrap_or_else(tokens::text::percent)
                            .to_string(),
                    )],
                )
            },
            Target::Text,
            // AND WITH NO READER IT REFUSES. `Order::Text` is what writes the
            // size, and with no thread to send it to the row would take a
            // press, show what was asked for, and hold that number for the
            // life of the view — under a note saying nothing here will change.
            //
            // AND WITH NO WATCH TOO, for the reason `View::blind` carries: the
            // step is computed from a number this process only learns from
            // the watch, so a door that lost it would ask for a size from a
            // baseline that has moved on.
            self.no_thread || self.blind,
        );
    }

    /// One screen and everything about it.
    fn sheet(&self, b: &mut Builder<'_>, s: &Screen, now: &Snapshot) {
        b.heading_said(s.description.clone());
        b.fact(ronoctua_i18n::key::DISPLAY_FACT_CONNECTOR, s.at.clone());
        b.fact(
            ronoctua_i18n::key::DISPLAY_FACT_FILED,
            filed_said(s, b.texts),
        );
        if let Some(inches) = s.inches() {
            let n = format!("{inches:.1}").replace('.', &b.texts.decimal_mark().to_string());
            b.fact(
                ronoctua_i18n::key::DISPLAY_FACT_SIZE,
                b.texts
                    .t_with(ronoctua_i18n::key::DISPLAY_VALUE_INCHES, &[("inches", &n)]),
            );
        }

        b.heading(ronoctua_i18n::key::DISPLAY_HEADING_PICTURE);
        // THE LAST LIT SCREEN CANNOT BE TURNED OFF. Doing it leaves a machine
        // with a compositor drawing nowhere and no way back that does not
        // involve another computer.
        let only_one = s.lit() && now.screens.iter().filter(|o| o.lit()).count() < 2;
        // AND A SCREEN NOTHING HERE TURNED OFF CANNOT BE TURNED ON FROM HERE.
        // The toggle asks "is it lit?" — the PICTURE — and writes `off`, which
        // is a DECISION. On a screen the compositor is not driving for a
        // reason of its own (a shut lid, a cable out) the two disagree: the
        // row read "no", the key was live, and pressing it wrote `off = false`
        // over an `off` that was already false. The file did not change, the
        // screen did not light, and nothing said why. It is the same rule the
        // rest of this section already follows — the row refuses, and its
        // value is the reason.
        let not_ours = !s.lit() && !s.wanted.off;
        b.pair(
            b.texts.t(ronoctua_i18n::key::DISPLAY_ON),
            // AND THE ROW SAYS WHY IT REFUSES. A row drawn dim with a plain
            // "yes" on it is a key that does nothing for a reason nobody can
            // read — the light row and the vary row both carry their reason
            // in the value, and this is the same rule.
            b.texts.t(if only_one {
                ronoctua_i18n::key::DISPLAY_ONLY_ONE_LIT
            } else if not_ours {
                ronoctua_i18n::key::DISPLAY_DARK_NOT_OURS
            } else if s.lit() {
                ronoctua_i18n::key::DISPLAY_VALUE_YES
            } else {
                ronoctua_i18n::key::DISPLAY_VALUE_NO
            }),
            Target::Power,
            only_one || not_ours,
        );
        let running = s.mode();
        // ONLY ONE ON OFFER IS A REASON, and the row says it — see the power
        // row just above for why a refusal without one is a key that does
        // nothing for reasons nobody can read.
        // AND A DARK SCREEN IS ITSELF A REASON. Every row of this section is
        // refused when the compositor is not driving the monitor, and until
        // now four of them said so with a mode or a percentage — a row drawn
        // dim whose value reads like any other value. `dark` is checked first
        // because it is the reason that outranks the rest.
        let dark = !s.lit();
        // AND A SCREEN THE COMPOSITOR REPORTS NO MODE FOR HAS NO MODE ROWS.
        // `Screen::current` is an `Option` and its doc says `None` does not
        // mean dark — so both rows were drawn LIVE, reading "the compositor
        // chooses", and both arrows died silently: the rate has nothing to
        // step through, and the resolution filed the unknown at the top of
        // the list, so one arrow did nothing and the other jumped to the
        // second-largest size whatever was actually running.
        let no_mode = s.lit() && running.is_none();
        let one_size = s.lit() && sizes_of(s).len() < 2;
        let one_rate =
            s.lit() && running.is_some_and(|m| rates_of(s, (m.width, m.height)).len() < 2);
        b.pair(
            b.texts.t(ronoctua_i18n::key::DISPLAY_RESOLUTION),
            if dark {
                b.texts.t(ronoctua_i18n::key::DISPLAY_SCREEN_IS_DARK)
            } else if no_mode {
                b.texts.t(ronoctua_i18n::key::DISPLAY_NO_MODE)
            } else if one_size {
                b.texts.t(ronoctua_i18n::key::DISPLAY_ONLY_ONE_OFFERED)
            } else {
                // `running` IS `Some` HERE: `dark` and `no_mode` between them
                // cover every way it can be `None`, so the fallback below is
                // not a branch — it is the type's shape. Written as a
                // `map_or_else` rather than an `unwrap` because a panic on a
                // reading from another thread is not a trade worth making.
                running.map_or_else(
                    || b.texts.t(ronoctua_i18n::key::DISPLAY_VALUE_AUTOMATIC),
                    size_said,
                )
            },
            Target::Resolution,
            dark || no_mode || one_size,
        );
        b.pair(
            b.texts.t(ronoctua_i18n::key::DISPLAY_RATE),
            if dark {
                b.texts.t(ronoctua_i18n::key::DISPLAY_SCREEN_IS_DARK)
            } else if no_mode {
                b.texts.t(ronoctua_i18n::key::DISPLAY_NO_MODE)
            } else if one_rate {
                b.texts.t(ronoctua_i18n::key::DISPLAY_ONLY_ONE_OFFERED)
            } else {
                // `running` IS `Some` HERE: `dark` and `no_mode` between them
                // cover every way it can be `None`, so the fallback below is
                // not a branch — it is the type's shape. Written as a
                // `map_or_else` rather than an `unwrap` because a panic on a
                // reading from another thread is not a trade worth making.
                running.map_or_else(
                    || b.texts.t(ronoctua_i18n::key::DISPLAY_VALUE_AUTOMATIC),
                    |m| rate_said(m, b.texts),
                )
            },
            Target::Rate,
            dark || no_mode || one_rate,
        );
        b.pair(
            b.texts.t(ronoctua_i18n::key::DISPLAY_SCALE),
            if dark {
                b.texts.t(ronoctua_i18n::key::DISPLAY_SCREEN_IS_DARK)
            } else {
                scale_said(s.wanted.scale, s.logical.map(|l| l.scale), b.texts)
            },
            Target::Scale,
            dark,
        );
        b.pair(
            b.texts.t(ronoctua_i18n::key::DISPLAY_TURN),
            if dark {
                b.texts.t(ronoctua_i18n::key::DISPLAY_SCREEN_IS_DARK)
            } else {
                turn_said(s.wanted.transform, s.turn(), b.texts)
            },
            Target::Turn,
            dark,
        );
        b.pair(
            b.texts.t(ronoctua_i18n::key::DISPLAY_PLACE),
            if dark {
                b.texts.t(ronoctua_i18n::key::DISPLAY_SCREEN_IS_DARK)
            } else {
                // WHAT WAS ASKED FOR COMES FIRST, until the reading catches
                // up — the arrangement's copy of this row was given the rule
                // and the SHEET's was not, so one knob showed two different
                // numbers on two levels while it was in flight. This is the
                // level everybody walks; the other is behind `m`.
                place_said(
                    self.place_pending
                        .as_ref()
                        .filter(|(who, ..)| *who == s.at)
                        .map(|&(_, x, y)| (x, y))
                        .or_else(|| s.wanted.position.map(|p| (p.x, p.y))),
                    s.logical.map(|l| (l.x, l.y)),
                    b.texts,
                )
            },
            Target::Place(s.at.clone()),
            dark,
        );
        b.pair(
            b.texts.t(ronoctua_i18n::key::DISPLAY_VARY),
            if dark {
                b.texts.t(ronoctua_i18n::key::DISPLAY_SCREEN_IS_DARK)
            } else {
                vary_said(s, b.texts)
            },
            Target::Vary,
            // AND A DARK SCREEN HAS NO REFRESH RATE TO VARY. Every other row
            // of this section is `!s.lit()`; this one was not, so on an off
            // screen it was the single row of "The picture" that still
            // answered the arrows — and then the single one drawn dim with a
            // value that reads like any other value.
            dark || !s.vrr_supported,
        );

        b.heading(ronoctua_i18n::key::DISPLAY_HEADING_LIGHT);
        match s.light.as_ref() {
            Some(l) => b.pair(
                b.texts.t(ronoctua_i18n::key::DISPLAY_BRIGHTNESS),
                if l.movable() {
                    b.texts.t_with(
                        ronoctua_i18n::key::DISPLAY_VALUE_PERCENT,
                        &[("percent", &l.percent().to_string())],
                    )
                } else {
                    // THE ROW SAYS IT BEFORE THE KEY IS PRESSED. A panel whose
                    // udev rule is missing reads fine and refuses every write,
                    // and finding that out by pressing is a key that does
                    // nothing.
                    b.texts.t(ronoctua_i18n::key::DISPLAY_LIGHT_IS_ROOTS)
                },
                Target::Light,
                !l.movable(),
            ),
            None => b.note(ronoctua_i18n::key::DISPLAY_LIGHT_ON_ITS_BUTTONS),
        }

        b.heading(ronoctua_i18n::key::DISPLAY_HEADING_SESSION);
        b.pair(
            b.texts.t(ronoctua_i18n::key::DISPLAY_FIRST),
            // THE VALUE FOLLOWS THE SAME TEST THE REFUSAL DOES. It read
            // `dark`, so a lid-shut monitor's row said "the compositor is not
            // drawing on it" over a `focus-at-startup` the file really
            // carries — a setting hidden from the one screen it is about.
            b.texts.t(if s.wanted.off {
                ronoctua_i18n::key::DISPLAY_SCREEN_IS_DARK
            } else if s.wanted.first {
                ronoctua_i18n::key::DISPLAY_VALUE_YES
            } else {
                ronoctua_i18n::key::DISPLAY_VALUE_NO
            }),
            Target::First,
            // AND A SCREEN THIS VIEW TURNED OFF CANNOT BE THE ONE THE SESSION
            // OPENS ON. `fragment::kdl_of` short-circuits a monitor whose
            // `off` is written down before it reaches `focus-at-startup`, so
            // the row would read "yes" over a file that says nothing of the
            // kind.
            //
            // `wanted.off` AND NOT `dark`, which is the picture. A docked
            // laptop with the lid shut is dark with `off == false`, so the
            // file DOES carry `focus-at-startup` for it — the row refused
            // over a setting that was in force, and would not say what it
            // was.
            s.wanted.off,
        );
    }

    /// Where the screens sit relative to each other.
    fn arrangement(&self, b: &mut Builder<'_>, now: &Snapshot) {
        b.heading(ronoctua_i18n::key::DISPLAY_HEADING_ARRANGEMENT);
        let lit: Vec<&Screen> = now.screens.iter().filter(|s| s.lit()).collect();
        if lit.len() < 2 {
            b.note(ronoctua_i18n::key::DISPLAY_ONE_SCREEN_ONLY);
            return;
        }
        for s in &lit {
            let asked = self
                .place_pending
                .as_ref()
                .filter(|(who, ..)| *who == s.at)
                .map(|&(_, x, y)| (x, y));
            b.pair(
                s.at.clone(),
                beside_said(s, asked, &lit, b.texts),
                Target::Place(s.at.clone()),
                false,
            );
        }
        // AN INSTRUCTION IS NOT A ROW: this level has rows the cursor is FOR,
        // and one it can stand on lights up as though it were one of them —
        // and copies itself on `RET`. Every sentence in the view goes through
        // `told` for this reason; the argument was first written here.
        b.told(b.texts.t(ronoctua_i18n::key::DISPLAY_ARRANGE_HOW));
    }
}

/// A screen's row on the list: what it is doing, in one line.
#[must_use]
pub fn summary_of(s: &Screen, texts: &ronoctua_i18n::Texts) -> String {
    if !s.lit() {
        return texts.t(ronoctua_i18n::key::DISPLAY_VALUE_OFF);
    }
    let mut said = Vec::new();
    if let Some(m) = s.mode() {
        said.push(size_said(m));
        said.push(rate_said(m, texts));
    }
    if let Some(l) = s.logical {
        // THE PICTURE'S, TWICE: this is the LIST's one-line summary of a
        // screen, which is about what one is looking at rather than what was
        // chosen for it. The sheet's row is the one that has to tell the two
        // apart, because it is the one the arrows act on.
        said.push(scale_said(Some(l.scale), Some(l.scale), texts));
    }
    if let Some(light) = s.light.as_ref() {
        said.push(texts.t_with(
            ronoctua_i18n::key::DISPLAY_VALUE_PERCENT,
            &[("percent", &light.percent().to_string())],
        ));
    }
    said.join(&format!(" {} ", tokens::mark::SEPARATOR))
}

/// What this screen's settings follow, said plainly.
#[must_use]
pub fn filed_said(s: &Screen, texts: &ronoctua_i18n::Texts) -> String {
    let why = match &s.naming {
        ronoctua_niri::Naming::Description(_) => ronoctua_i18n::key::DISPLAY_FILED_BY_ITSELF,
        ronoctua_niri::Naming::Connector {
            why: ronoctua_niri::Anonymous::Silent,
            ..
        } => ronoctua_i18n::key::DISPLAY_FILED_SILENT,
        ronoctua_niri::Naming::Connector {
            why: ronoctua_niri::Anonymous::NoSerial,
            ..
        } => ronoctua_i18n::key::DISPLAY_FILED_NO_SERIAL,
    };
    texts.t(why)
}

/// Whether the refresh rate may vary, and how.
#[must_use]
pub fn vary_said(s: &Screen, texts: &ronoctua_i18n::Texts) -> String {
    if !s.vrr_supported {
        return texts.t(ronoctua_i18n::key::DISPLAY_VARY_CANNOT);
    }
    texts.t(match s.wanted.vary {
        Some(ronoctua_display::fragment::Vary::Always) => ronoctua_i18n::key::DISPLAY_VARY_ALWAYS,
        Some(ronoctua_display::fragment::Vary::OnDemand) => {
            ronoctua_i18n::key::DISPLAY_VARY_ON_DEMAND
        }
        None => ronoctua_i18n::key::DISPLAY_VALUE_NO,
    })
}

/// How many screens there are, for the row that opens the arrangement.
#[must_use]
pub fn arrangement_said(now: &Snapshot, texts: &ronoctua_i18n::Texts) -> String {
    let lit = now.screens.iter().filter(|s| s.lit()).count();
    // `t_args` AND `Arg::Number`, never `t_with`. Fluent picks a plural
    // variant from the VALUE's type: given the string "1" it looks for a
    // variant literally named `1`, finds none, and falls through to the
    // default — so the row read "1 screens" on the machine every laptop is,
    // with every check green. The crate's own doc records the measurement, and
    // `network.rs` and `notifications.rs` both feed their selectors this way.
    texts.t_args(
        ronoctua_i18n::key::DISPLAY_VALUE_SCREENS,
        &[(
            "n",
            ronoctua_i18n::Arg::Number(i64::try_from(lit).unwrap_or(i64::MAX)),
        )],
    )
}

/// Where one screen sits, said against the one it is next to.
///
/// THE NUMBERS ARE THERE TOO, and the words are not a replacement for them:
/// `x=1920 y=0` is what the file will say and what a person types when they
/// want a gap. What the words add is the answer to the question somebody
/// actually has, which is "which one is on the left".
#[must_use]
pub fn beside_said(
    s: &Screen,
    asked: Option<(i32, i32)>,
    all: &[&Screen],
    texts: &ronoctua_i18n::Texts,
) -> String {
    // THE SETTING AND THE PICTURE, so the arrangement's rows say whether a
    // place is written down — which is what decides whether `a` on one of them
    // has anything to hand back.
    //
    // AND WHAT WAS ASKED FOR COMES FIRST, until the reading catches up — the
    // text row's rule, and the same round trip. Without it a held arrow drew
    // the old number for as long as the trip took, which reads as a key that
    // does not work.
    let place = place_said(
        asked.or_else(|| s.wanted.position.map(|p| (p.x, p.y))),
        s.logical.map(|l| (l.x, l.y)),
        texts,
    );
    let Some(me) = s.logical else {
        return place;
    };
    let neighbour = all
        .iter()
        .filter(|o| o.at != s.at)
        .filter_map(|o| o.logical.map(|l| (o, l)))
        // SATURATING AND UNSIGNED, for the reason the `Target::Place` arm of
        // `turn` gives: a
        // `position x=` is a line somebody may have typed, and the difference
        // of two `i32`s at opposite ends overflows in debug and wraps in
        // release — where the wrap would name the FARTHEST screen the nearest.
        .min_by_key(|(_, l)| l.x.abs_diff(me.x).saturating_add(l.y.abs_diff(me.y)));
    let Some((other, theirs)) = neighbour else {
        return place;
    };
    let sideways = me.x.abs_diff(theirs.x) >= me.y.abs_diff(theirs.y);
    let key = match (sideways, me.x < theirs.x, me.y < theirs.y) {
        (true, true, _) => ronoctua_i18n::key::DISPLAY_LEFT_OF,
        (true, false, _) => ronoctua_i18n::key::DISPLAY_RIGHT_OF,
        (false, _, true) => ronoctua_i18n::key::DISPLAY_ABOVE,
        (false, _, false) => ronoctua_i18n::key::DISPLAY_BELOW,
    };
    let beside = texts.t_with(key, &[("other", &other.at)]);
    format!("{place} {} {beside}", tokens::mark::SEPARATOR)
}

/// How far one arrow moves the brightness, in percent.
///
/// Five, which is `power`'s step for the charge thresholds. The keyboard's own
/// brightness keys are a later deliverable and will read this constant rather
/// than carry one, so that holding the key and holding the arrow arrive at the
/// same numbers — written here because the number is what makes that possible,
/// not because those keys exist yet.
pub const STEP: i16 = 5;

/// How far one arrow moves a screen in the arrangement, in logical pixels.
///
/// A round number and not one pixel: an arrangement is set by dragging a
/// rectangle in every other system, and a key that moves a monitor by one
/// pixel would take two thousand presses to put it beside its neighbour. Ten
/// is a step somebody can hold, and the exact number is typed into the file
/// by whoever needs an exact number.
pub const NUDGE: i32 = 10;

impl View {
    /// `←` / `→` on the row under the cursor.
    /// Whether `←`/`→` mean anything on a row with this target.
    ///
    /// IT NAMES THE ARMS OF [`View::turn`], and it is the only list that has
    /// to be kept in step with it — `hint` asks the ROWS which of them are
    /// enabled, so nothing here repeats a condition that lives there.
    ///
    /// A target missing from this list is an arrow taught nowhere; a target
    /// here that `turn` does not answer is an arrow taught and dead. The
    /// second is what the old, level-shaped version of this produced.
    const fn turns(target: &Target) -> bool {
        matches!(
            target,
            Target::Text
                | Target::Light
                | Target::Power
                | Target::Turn
                | Target::Scale
                | Target::Resolution
                | Target::Rate
                | Target::Vary
                | Target::First
                | Target::Place(_)
        )
    }

    fn turn(&mut self, target: &Target, up: bool, now: &Snapshot, moment: Instant) -> Errand {
        use ronoctua_display::Order;
        match target {
            Target::Text => {
                // FROM WHAT WAS LAST ASKED FOR, when the answer has not come
                // back yet — see `View::text_pending`.
                let from = self.text_pending.unwrap_or_else(tokens::text::percent);
                let want = tokens::text::stepped(from, if up { 1 } else { -1 });
                // AT THE END OF THE RANGE, NOTHING. `stepped` clamps, so at
                // the ceiling the answer is where one already is — and a key
                // held down there wrote `display.json` on every repeat and
                // forced a full reading (a walk of `/sys/class/drm` and two
                // connections to niri) for a value that cannot move.
                if want == from {
                    return Errand::Nothing;
                }
                self.text_pending = Some(want);
                Errand::Order(Box::new(Order::Text(want)))
            }
            Target::Light => {
                let Some(s) = self.here(now) else {
                    return Errand::Nothing;
                };
                // A SCREEN WITH NO LIGHT AT ALL HAS NO ROW TO ACT ON, and one
                // whose light will not move is drawn refusing — the arrow does
                // not reach here either way. Kept as a guard rather than an
                // assumption, because the reading arrives from a thread.
                if s.light.as_ref().is_none_or(|l| !l.movable()) {
                    return Errand::Nothing;
                }
                // NAMED, so the row's own screen moves and not whichever has
                // the focus — inside this view the row says which, and the
                // screen whose sheet is open is not always the focused one.
                // `None` there is the keyboard's meaning, not a row's.
                //
                // A STEP AND NOT A PERCENTAGE TO LAND ON: a laptop panel
                // offers as few as seven raw values, and minus five percent of
                // seven is the same raw value. `Order::Bright` carries the
                // direction and `backlight::stepped_raw` guarantees the move —
                // the row used to compute a percentage here, and on such a
                // panel the arrow did nothing, repeatedly and silently.
                Errand::Order(Box::new(Order::Bright {
                    at: Some(s.at.clone()),
                    by: if up { STEP } else { -STEP },
                }))
            }
            // THE KNOB IS A TOGGLE, so the ask is the state the screen is NOT
            // in. It asked `On` whichever way the row read: on a lit screen
            // that is a no-op, and the question that went up over it said the
            // screen had been turned off. Fifteen seconds later an `Undo`
            // arrived for a change nobody had made.
            Target::Power => {
                let Some(s) = self.here(now) else {
                    return Errand::Nothing;
                };
                // AND THE DIRECTION DECIDES, which it did not for a round:
                // both arrows read `s.lit()` and did the same thing, so on a
                // lit screen `→ more` turned the screen OFF. Every other row
                // in the view moves right for more of the thing; this one is
                // the only one where "more" and "less" are a state rather
                // than a number, and right is lit.
                if up == s.lit() {
                    return Errand::Nothing;
                }
                if s.lit() {
                    return self.provisional(
                        now,
                        moment,
                        Ask::Off,
                        ronoctua_i18n::key::DISPLAY_WHAT_POWER_OFF,
                    );
                }
                // AND LIGHTING A SCREEN IS WRITTEN DOWN, NOT PREVIEWED — this
                // is the one row of the view whose two directions have two
                // natures. There is nothing on a dark screen to compare a
                // preview against, and lighting one cannot blind anybody, so
                // the countdown would be fifteen seconds of a sentence about a
                // picture that is not being drawn.
                //
                // HERE AND NOT INSIDE `provisional`, which is where the rule
                // used to live as a short-circuit. Every other knob's row is
                // `disabled` on a dark screen and `automatic` refuses a dark
                // sheet outright, so that branch could only ever be reached by
                // this one key — and it needed a `what` it never drew, which
                // is a text key nothing in the system could render.
                let at = s.at.clone();
                self.keep_for(&at, now, |w| write_the(w, Ask::On))
            }
            Target::Turn => {
                let Some(s) = self.here(now) else {
                    return Errand::Nothing;
                };
                // NOT A `ring`, WHICH IT WAS CALLED: `step` stops at both
                // ends by contract — the resolution list needs that — so
                // `←` on `normal` does nothing and the eight turns are a
                // list one walks, not a wheel one spins.
                let turns = Transform::ALL;
                let at = turns.iter().position(|t| *t == s.wanted.transform);
                let Some(next) = step(&turns, at.or(Some(0)), !up) else {
                    return Errand::Nothing;
                };
                self.provisional(
                    now,
                    moment,
                    Ask::Transform(next),
                    ronoctua_i18n::key::DISPLAY_WHAT_TURN,
                )
            }
            Target::Scale => {
                let Some(s) = self.here(now) else {
                    return Errand::Nothing;
                };
                // FROM THE SETTING WHEN THERE IS ONE, and from the glass when
                // there is not. The step used to start from `wanted.scale`
                // alone, and on a panel nobody has configured that is `None`
                // while the picture is the 200% niri chose: "more" answered
                // 100%, and the picture halved.
                //
                // The row says both (`scale_said`), so the arrow starting at
                // the setting is what the row's leading value shows — and the
                // parenthesis is there for the case where they differ, which
                // is the case this `or_else` exists for.
                let from = s.wanted.scale.or_else(|| s.logical.map(|l| l.scale));
                let Some(next) = next_scale(from, up) else {
                    return Errand::Nothing;
                };
                self.provisional(
                    now,
                    moment,
                    Ask::Scale(next),
                    ronoctua_i18n::key::DISPLAY_WHAT_SCALE,
                )
            }
            Target::Resolution => {
                let Some(s) = self.here(now) else {
                    return Errand::Nothing;
                };
                let sizes = sizes_of(s);
                let running = s.mode().map(|m| (m.width, m.height));
                let at = running.and_then(|r| sizes.iter().position(|z| *z == r));
                let Some(size) = step(&sizes, at.or(Some(0)), up) else {
                    return Errand::Nothing;
                };
                // THE BEST RATE THAT SIZE OFFERS, and not the one that was
                // running: a rate belongs to a size, and 144 Hz at 4K is a
                // mode most monitors do not have.
                let Some(mode) = rates_of(s, size).first().copied() else {
                    return Errand::Nothing;
                };
                self.provisional(
                    now,
                    moment,
                    Ask::Mode(Some(mode.asked())),
                    ronoctua_i18n::key::DISPLAY_WHAT_RESOLUTION,
                )
            }
            Target::Rate => {
                let Some(s) = self.here(now) else {
                    return Errand::Nothing;
                };
                let Some(running) = s.mode() else {
                    return Errand::Nothing;
                };
                let rates = rates_of(s, (running.width, running.height));
                let at = rates
                    .iter()
                    .position(|m| m.refresh_rate == running.refresh_rate);
                let Some(mode) = step(&rates, at.or(Some(0)), up) else {
                    return Errand::Nothing;
                };
                self.provisional(
                    now,
                    moment,
                    Ask::Mode(Some(mode.asked())),
                    ronoctua_i18n::key::DISPLAY_WHAT_RATE,
                )
            }
            Target::Vary => {
                let Some(s) = self.here(now) else {
                    return Errand::Nothing;
                };
                // THREE STATES AND NO QUESTION. Varying a refresh rate cannot
                // make a screen unreadable; it costs battery, which is a thing
                // one notices later and undoes at leisure.
                //
                // AND THE STATE IS OURS, NOT THE READING'S. `Screen::vrr_enabled`
                // says whether the rate is varying RIGHT NOW, which is not the
                // same question: `on_demand` reads `false` whenever no window
                // has asked for it, so a three-state control driven from the
                // reading would fall out of `on-demand` by itself. `Ask::Vrr`'s
                // own doc carries the whole of it. The cost is that VRR turned
                // on outside our fragment reads here as "no".
                let was = s.wanted.vary;
                let next = match (s.wanted.vary, up) {
                    (None, true) => Some(ronoctua_display::fragment::Vary::OnDemand),
                    (Some(ronoctua_display::fragment::Vary::OnDemand), true) => {
                        Some(ronoctua_display::fragment::Vary::Always)
                    }
                    (Some(ronoctua_display::fragment::Vary::Always), true) => {
                        Some(ronoctua_display::fragment::Vary::Always)
                    }
                    (Some(ronoctua_display::fragment::Vary::Always), false) => {
                        Some(ronoctua_display::fragment::Vary::OnDemand)
                    }
                    (_, false) => None,
                };
                // AT EITHER END, NOTHING. `Order::Keep` is not free: it costs
                // a reading to file the screen by name, a write of the file,
                // and then six IPC calls saying what the file says — per
                // keyboard repeat. The text row already refuses a step that
                // cannot move, and the arrows already do at the ends of their
                // lists; this was the one knob left writing for a no-op.
                if next == was {
                    return Errand::Nothing;
                }
                self.keep(now, |w| w.vary = next)
            }
            Target::First => {
                let Some(s) = self.here(now) else {
                    return Errand::Nothing;
                };
                // AND THE DIRECTION DECIDES HERE TOO. This is the view's other
                // yes/no row, and it was left a toggle when the power row
                // stopped being one — so `→ more` on a screen that already
                // opens the session turned that OFF, and `← less` on one that
                // does not turned it ON and swept the flag off every other
                // screen. Right is yes, on both rows.
                if up == s.wanted.first {
                    return Errand::Nothing;
                }
                let want = up;
                // NO GUARD IS NEEDED HERE and it is worth saying why: a toggle
                // always moves, so `would_move`'s job is done by the `!`.
                // What holding the key costs is one write per repeat, which is
                // what a toggle held down means anywhere.
                // AND ONLY ONE SCREEN CAN BE IT. Nothing downstream enforces
                // that — `kdl_of` writes `focus-at-startup` for every screen
                // whose `Wanted` says so, and niri takes the last — so two
                // screens could both be turned on here and both rows would
                // read "yes" while only one of them was true.
                //
                // Turning one ON turns the others off; turning one OFF leaves
                // the rest alone, because "none of them" is a state the
                // compositor already has an answer for.
                let mine = s.at.clone();
                if want {
                    let others: Vec<Errand> = now
                        .screens
                        .iter()
                        .filter(|o| o.at != mine && o.wanted.first)
                        .map(|o| {
                            let at = o.at.clone();
                            self.keep_for(&at, now, |w| w.first = false)
                        })
                        .collect();
                    // THE ORDERS GO OUT AS ONE ERRAND CANNOT CARRY TWO, so
                    // the others are sent through `Errand::Orders` — the door
                    // hands them over in the order they were made, and the
                    // last one is the screen being turned on.
                    if !others.is_empty() {
                        let mut all: Vec<ronoctua_display::Order> = others
                            .into_iter()
                            .filter_map(|e| match e {
                                Errand::Order(o) => Some(*o),
                                _ => None,
                            })
                            .collect();
                        if let Errand::Order(o) = self.keep(now, move |w| w.first = want) {
                            all.push(*o);
                        }
                        return Errand::Orders(all);
                    }
                }
                self.keep(now, move |w| w.first = want)
            }
            Target::Place(at) => {
                let Some(s) = now.screen(at) else {
                    return Errand::Nothing;
                };
                // FROM WHAT WAS LAST ASKED FOR, then from what is WRITTEN
                // DOWN, and only then from the picture — the same order
                // `place_said` draws them in, and the same round trip
                // `View::place_pending` carries for the reason written there.
                let Some((from_x, from_y)) = self
                    .place_pending
                    .as_ref()
                    .filter(|(who, ..)| who == at)
                    .map(|&(_, x, y)| (x, y))
                    .or_else(|| s.wanted.position.map(|p| (p.x, p.y)))
                    .or_else(|| s.logical.map(|l| (l.x, l.y)))
                else {
                    return Errand::Nothing;
                };
                // SATURATING, because `position x=` is a line somebody may
                // have typed: these files are meant to be edited by hand, and
                // an `i32` at its end overflows in debug and wraps in release.
                let x = if up {
                    from_x.saturating_add(NUDGE)
                } else {
                    from_x.saturating_sub(NUDGE)
                };
                // AT THE END OF THE RANGE, NOTHING — the text row's rule, and
                // here it is what stops a held arrow rewriting the file for a
                // number that cannot move.
                if x == from_x {
                    return Errand::Nothing;
                }
                let at = at.clone();
                self.place_pending = Some((at.clone(), x, from_y));
                self.keep_for(&at, now, move |w| {
                    w.position = Some(ronoctua_display::fragment::Spot { x, y: from_y });
                })
            }
            _ => Errand::Nothing,
        }
    }

    /// Applies a change provisionally and starts the clock on it.
    ///
    /// THE SCREEN IS THE LEVEL'S, and there used to be a second half of this
    /// that took a connector instead — for "a row that names its screen rather
    /// than standing on it", which is what the arrangement's rows are. Its one
    /// caller was this function, handing it the level's own screen; the row
    /// that needed the other half is the place row, and that row stopped being
    /// provisional. What is left of the idea is `keep_for`, which does take a
    /// connector, and which the place row goes through.
    fn provisional(
        &mut self,
        now: &Snapshot,
        moment: Instant,
        ask: Ask,
        what: &'static str,
    ) -> Errand {
        let Some(s) = self.here(now) else {
            return Errand::Nothing;
        };
        // A QUESTION ABOUT NOTHING IS NOT A QUESTION. `a` hands a knob back to
        // the compositor, and on a knob the compositor already chooses that is
        // no change at all — the sheet would be replaced for fifteen seconds
        // by a sentence saying the scale had changed, ending in an `Undo` for
        // a change nobody made. The arrows have the same end at the ends of
        // their lists, and `step`/`next_scale` answer `None` there; this is
        // the same rule for the knobs that have no list.
        if !would_move(s, ask) {
            return Errand::Nothing;
        }
        // A DARK SCREEN NEVER REACHES HERE, and it is worth saying where the
        // rule went: every knob's row is `disabled` when the compositor is not
        // drawing on the monitor, `key` refuses the arrows on a disabled row,
        // and `automatic` refuses a dark sheet before its `match`. The one key
        // that acts on a dark screen is the power toggle, and it writes down
        // rather than previews — in its own arm, where the reason is about
        // that row and not about this function.
        self.asking = Some(Asking {
            at: s.at.clone(),
            what,
            // THE CLOCK `key` WAS HANDED, not a fresh reading of the wall:
            // the whole `key`/`rows`/`ran_out` chain takes an injectable
            // moment so a caller can hold time still, and this one place
            // ignored it — measuring the countdown against a different clock
            // from the one drawing it.
            since: moment,
            deciding: Deciding::Preview(ask),
            stood_at: self.cursor,
            was: self.at.clone(),
        });
        // THE CURSOR GOES TO THE WAY BACK — the pairing question's rule and
        // its reason: a question that opens itself over whatever was on
        // screen must not be answered YES by the next `RET` somebody presses.
        // Here it is sharper still, because the person may be pressing keys
        // on a picture they cannot read.
        //
        // ROW 3: the title, the sentence saying what changed, the one
        // counting down, and then the answer. The number is written out
        // rather than searched for, as `bluetooth::asked` writes 2 — and the
        // test `a_question_puts_the_way_back_first_and_the_cursor_on_it` is
        // what keeps it true when the rows move.
        self.cursor = 3;
        // AND IT STAYS THERE while the seconds tick: `settle` restores by
        // TARGET, and the countdown rebuilds the rows once a second.
        self.at = Some(Target::PutItBack);
        Errand::Order(Box::new(ronoctua_display::Order::Try {
            at: s.at.clone(),
            ask,
        }))
    }

    /// Writes a change down at once — for the knobs that cannot blind a screen.
    fn keep<F>(&mut self, now: &Snapshot, change: F) -> Errand
    where
        F: FnOnce(&mut ronoctua_display::fragment::Wanted),
    {
        let Some(s) = self.here(now) else {
            return Errand::Nothing;
        };
        let at = s.at.clone();
        self.keep_for(&at, now, change)
    }

    /// AND AN ORDER THAT CHANGES NOTHING IS NOT AN ORDER — the written-down
    /// half of "a question about nothing is not a question" (`would_move`,
    /// which is the same rule for the knobs that ask first).
    ///
    /// The rule used to live in `would_move` alone, and was reached only by
    /// the knobs that go through a countdown. When the place row's `a` stopped
    /// being provisional it left that guard behind with nothing in its place:
    /// on the default state of every machine nobody has placed a monitor on,
    /// `a` on Place cleared a position that was already unset and emitted a
    /// `Keep` — a reading, a rewrite of `display.json` and six IPC calls, per
    /// keyboard repeat, redrawing byte for byte. The `Vary` knob's ends and
    /// the brightness at 0 and 100 are the same shape.
    ///
    /// COMPARED AND NOT REASONED ABOUT: `Wanted` is `PartialEq`, so this holds
    /// for a field nobody has written a rule for yet.
    fn keep_for<F>(&mut self, at: &str, now: &Snapshot, change: F) -> Errand
    where
        F: FnOnce(&mut ronoctua_display::fragment::Wanted),
    {
        let Some(s) = now.screen(at) else {
            return Errand::Nothing;
        };
        let mut wanted = s.wanted.clone();
        change(&mut wanted);
        if wanted == s.wanted {
            return Errand::Nothing;
        }
        Errand::Order(Box::new(ronoctua_display::Order::Keep {
            at: at.to_owned(),
            wanted: Box::new(wanted),
        }))
    }

    /// `RET`.
    ///
    /// NO `texts`: it took one and used it only to discard it. Everything
    /// this function produces is either an order or a string already on the
    /// screen, so there is nothing here to localise.
    fn enter(&mut self, target: &Target, now: &Snapshot, rows: &[Row]) -> Errand {
        match target {
            Target::Screen(at) => {
                self.go_to(Level::Monitor(at.clone()));
                Errand::Nothing
            }
            Target::Arrange => {
                self.go_to(Level::Arrange);
                Errand::Nothing
            }
            Target::PutItBack => match self.answered() {
                // NOTHING WAS APPLIED, so there is nothing to put back — and
                // an `Undo` here would not be harmless: it re-sends what the
                // file says over the IPC, which is work and a flicker for a
                // question that changed nothing.
                Some(a) if !a.on_the_clock() => Errand::Nothing,
                _ => Errand::Order(Box::new(ronoctua_display::Order::Undo)),
            },
            Target::KeepIt => {
                let Some(asking) = self.answered() else {
                    return Errand::Nothing;
                };
                // WHICH NATURE IT IS decides what a yes writes: the one with
                // a clock writes the knob that was previewed, and the one
                // without writes the default, which is what forgetting means.
                // THE SCREEN WENT AWAY WHILE THE QUESTION STOOD, and this is
                // checked FIRST, FOR BOTH NATURES. Nothing can be written for
                // a monitor that is not there — but the question disappearing
                // with no word was worse than nothing: the person answered
                // "keep it" and the screen simply went back to the sheet.
                // `Undo` is what puts the rest of them where the file says,
                // and the door says why.
                //
                // The `Whether` arm used to return before this, so forgetting
                // an unplugged monitor wrote a `Keep` for a connector that is
                // not there and got the generic refusal instead.
                if now.screen(&asking.at).is_none() {
                    // NOTHING TO PUT BACK when nothing was applied — the same
                    // rule `Target::PutItBack` states twenty lines above, and
                    // for the same reason: `Undo` re-sends what the file says
                    // over the IPC for every screen that has a preview
                    // standing, which is work and a flicker.
                    let clock = asking.on_the_clock();
                    return Errand::Gone(asking.at, clock);
                }
                let ask = match asking.deciding {
                    Deciding::Preview(ask) => ask,
                    // FORGETTING IS WRITING THE DEFAULT, which is what every
                    // field of `Wanted` means by "let the compositor decide" —
                    // and `say_what_the_file_says` then tells niri all six,
                    // which is what actually puts the screen back.
                    Deciding::Whether => {
                        return Errand::Order(Box::new(ronoctua_display::Order::Keep {
                            at: asking.at,
                            wanted: Box::default(),
                        }));
                    }
                };
                // THE SCREEN IS STILL THERE — the check above returned `Gone`
                // if it was not, for both natures. This one only takes it out
                // of the `Option`.
                let Some(s) = now.screen(&asking.at) else {
                    return Errand::Nothing;
                };
                // AND THE LAST LIT SCREEN IS STILL NOT TURNED OFF.
                //
                // `only_one` disables the row when the question is RAISED, and
                // a question with a clock stands for fifteen seconds. Inside
                // that window the other screen can stop being lit for a reason
                // of its own — a lid shut, DPMS, a cable half out — and none
                // of those is an unplug, so the `Gone` check above does not
                // fire. Answering "keep it" then wrote `off` for the only
                // screen left, which is a compositor drawing nowhere and no
                // way back that does not involve another computer: exactly
                // what `only_one` exists to make impossible.
                //
                // THE ANSWER BECOMES "PUT IT BACK", which is the same `Undo`
                // the safe button and the clock both send — so the screen
                // relights, and the sheet it returns to draws the power row
                // dim with `display-only-one-lit` in its value, which is the
                // reason. There is no `Errand` for "refused" and this does not
                // invent one: the reason already has a place on the screen,
                // and it is the place a person looks after a toggle does not
                // take.
                if matches!(ask, ronoctua_niri::Ask::Off)
                    && now.screens.iter().filter(|o| o.lit()).count() < 2
                {
                    return Errand::Order(Box::new(ronoctua_display::Order::Undo));
                }
                // ONE KNOB, THE ONE THAT WAS ASKED ABOUT. Writing the scale,
                // the position and the mode on every keep — which is what
                // this did — opts somebody out of niri's own choice of all
                // three the first time they nudge any one of them, and
                // nothing on the screen says so.
                //
                // AND WHAT WAS ASKED, not only what is on the glass. The two
                // differ in the case the ladder is built around: stepping off
                // the bottom asks `Scale(None)` — "you choose" — and the
                // reading afterwards is the number niri chose. Writing that
                // number down would keep a different change from the one that
                // was previewed and confirmed.
                //
                let mut wanted = s.wanted.clone();
                write_the(&mut wanted, ask);
                Errand::Order(Box::new(ronoctua_display::Order::Keep {
                    at: asking.at,
                    wanted: Box::new(wanted),
                }))
            }
            // EVERYTHING ELSE IS A VALUE, and a value is moved with the
            // arrows. `RET` on one copies it, like any other fact — which is
            // also what `Target::Nothing` does, so the two arms that used to
            // spell this out separately are one.
            _ => rows
                .get(self.cursor)
                .and_then(|r| crate::chrome::information_of(&r.fields))
                .map_or(Errand::Nothing, Errand::Copy),
        }
    }

    /// `a`: hand the KNOB under the cursor back to the compositor — and ask.
    ///
    /// The knob, and not the screen: `d` is the one that is about the whole
    /// screen, which is why the two are gated differently in `key`.
    ///
    /// IT USED TO WRITE STRAIGHT AWAY. This module's own header says a change
    /// that can leave a screen unreadable is applied provisionally, and this
    /// is such a change: somebody who chose a mode because the monitor's
    /// PREFERRED one does not display is one keystroke from the mode that
    /// does not display, with nothing to press to get back. It goes through
    /// the same question the arrows do — except on a screen that is dark,
    /// where there is no picture to judge and the whole gesture is refused.
    fn automatic(&mut self, target: &Target, now: &Snapshot, moment: Instant) -> Errand {
        // A DARK SCREEN HAS NO KNOB TO HAND BACK. Every row of its picture is
        // drawn refusing with the sentence that says why, and `a` is not gated
        // by that refusal — deliberately, because `disabled` usually means the
        // ARROWS have nowhere to go and `a` is exactly for the rows where
        // something IS written down. Dark is the one reason that is about the
        // whole screen rather than the row, so it is checked here: without it
        // `a` wrote the setting away and the row redrew identically, because
        // its value is the dark sentence and does not depend on `wanted`.
        //
        // The place row is not on a screen at all when the level is the
        // arrangement, which lists only lit ones — so `here` answering `None`
        // there is not this case.
        if matches!(self.level, Level::Monitor(_)) && self.here(now).is_some_and(|s| !s.lit()) {
            return Errand::Nothing;
        }
        match target {
            // ONE ASK, TWO SENTENCES. Handing the mode back is the same order
            // from either row — a rate belongs to a size — but the question
            // has to name the row the key was pressed on, or `a` on the
            // refresh rate raises a question about the resolution.
            Target::Resolution => self.provisional(
                now,
                moment,
                Ask::Mode(None),
                ronoctua_i18n::key::DISPLAY_WHAT_RESOLUTION,
            ),
            Target::Rate => self.provisional(
                now,
                moment,
                Ask::Mode(None),
                ronoctua_i18n::key::DISPLAY_WHAT_RATE,
            ),
            Target::Scale => self.provisional(
                now,
                moment,
                Ask::Scale(None),
                ronoctua_i18n::key::DISPLAY_WHAT_SCALE,
            ),
            Target::Turn => self.provisional(
                now,
                moment,
                Ask::Transform(Transform::Normal),
                ronoctua_i18n::key::DISPLAY_WHAT_TURN,
            ),
            // WRITTEN DOWN, NOT PREVIEWED — the same nature the arrows on this
            // row already have, and now with the reason written: moving a
            // monitor ten pixels sideways cannot leave a screen nobody can
            // read, which is the only thing the countdown is for. One knob
            // with two natures was one gesture asking and the other deciding,
            // with nothing saying why.
            //
            // BY THE ROW'S OWN CONNECTOR, because this row exists on TWO
            // levels: the sheet, where `here` resolves it, and the
            // arrangement, where `here` answers `None`.
            Target::Place(at) => {
                let at = at.clone();
                // AND WHAT WAS ASKED FOR IS LET GO OF, because handing the
                // place back is asking for no place at all — a pending one
                // would go on being drawn, and the next arrow would step from
                // it.
                //
                // FOR THIS SCREEN ONLY. `place_pending` holds ONE screen, and
                // clearing it whatever it names undoes a nudge still in flight
                // on a DIFFERENT one: hold `→` on `DP-1`, move to `eDP-1`,
                // press `a`, and `DP-1`'s row snaps back to the stale position
                // for the rest of the round trip — so the next arrow there
                // steps from the old number and one nudge is lost. That is the
                // defect `place_pending` was added to fix, reintroduced across
                // screens. It is `never_mind`'s rule again: an answer answers
                // the change it was about and no other.
                if self
                    .place_pending
                    .as_ref()
                    .is_some_and(|(who, _, _)| who == &at)
                {
                    self.place_pending = None;
                }
                self.keep_for(&at, now, |w| w.position = None)
            }
            _ => Errand::Nothing,
        }
    }

    /// `d`: throw away everything chosen for this screen — after a yes.
    ///
    /// A CONFIRMATION AND NOT A PREVIEW, because what is at risk here is not
    /// the picture: every default is the compositor's own choice, which is
    /// the state the machine boots into. What is at risk is the decisions
    /// somebody made, and there is no undoing those from the screen. It was
    /// one bare letter, applied at once. See [`Deciding`].
    fn forget(&mut self, now: &Snapshot, moment: Instant) -> Errand {
        let Some(s) = self.here(now) else {
            return Errand::Nothing;
        };
        // NOTHING TO FORGET IS NOT A QUESTION. A screen nobody has chosen
        // anything for would otherwise put up a question whose yes and whose
        // no do exactly the same thing.
        if s.wanted.says_nothing() {
            return Errand::Nothing;
        }
        let at = s.at.clone();
        self.asking = Some(Asking {
            at,
            // THE SENTENCE THIS QUESTION DRAWS. `question` reads `what` only
            // for the other nature, and this one writes its own two lines —
            // so the field used to carry a key nothing in the system could
            // render, under a comment naming `refused` as its reader.
            // `refused` takes a `Refused` and never sees an `Asking`. A field
            // that names the question names the sentence on the glass.
            what: ronoctua_i18n::key::DISPLAY_ASK_FORGET,
            since: moment,
            deciding: Deciding::Whether,
            stood_at: self.cursor,
            was: self.at.clone(),
        });
        self.cursor = 3;
        self.at = Some(Target::PutItBack);
        Errand::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_display::connector::Wiring;
    use ronoctua_niri::{Logical, Mode, Naming};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn mode(w: u16, h: u16, mhz: u32) -> Mode {
        Mode {
            width: w,
            height: h,
            refresh_rate: mhz,
            is_preferred: false,
        }
    }

    /// A screen the view could be shown — INVENTED, every field of it.
    ///
    /// The make, the model and the PHYSICAL SIZE are hardware identity, and
    /// this repository is public (invariants 2 and 13). The millimetres are
    /// the trap: a capture keeps its make and model in plain sight, and they
    /// get noticed, while a pair of numbers reads as arithmetic and stays.
    /// A panel's size names its model as surely as the model does.
    ///
    /// So they are round numbers that no panel has, chosen the way
    /// `ronoctua_niri`'s fixture chooses them and for the same written reason.
    /// Nothing in this module reads them: they are here because `Screen` has
    /// the field.
    fn screen(at: &str, lit: bool) -> Screen {
        Screen {
            at: at.to_owned(),
            naming: Naming::Description(format!("Make Model {at}")),
            description: format!("Make Model {at}"),
            make: "Make".to_owned(),
            model: "Model".to_owned(),
            serial: Some(at.to_owned()),
            millimetres: Some((300, 200)),
            modes: vec![
                mode(1920, 1200, 59_951),
                mode(1920, 1200, 143_856),
                mode(1280, 720, 60_000),
            ],
            current: Some(0),
            logical: lit.then_some(Logical {
                x: 0,
                y: 0,
                width: 1920,
                height: 1200,
                scale: 1.0,
                transform: Transform::Normal,
            }),
            vrr_supported: true,
            vrr_enabled: false,
            internal: true,
            light: None,
            wiring: Wiring::default(),
            wanted: ronoctua_display::fragment::Wanted::default(),
        }
    }

    fn snapshot(screens: Vec<Screen>) -> Snapshot {
        Snapshot {
            focused: screens.first().map(|s| s.at.clone()),
            screens,
            compositor: true,
            config_failed: false,
            settings_failed: false,
        }
    }

    #[test]
    fn the_rows_are_the_same_things_at_every_width() {
        // A KEY MEANS THE SAME THING WHATEVER THE WINDOW'S SIZE. The rows the
        // keyboard walks are built at one width, and if the WIDTH changed
        // which rows exist, a key pressed in a narrow window would act on a
        // different line than the same key in a wide one. `power.rs` carries
        // this test for the same reason.
        let now = snapshot(vec![screen("eDP-1", true), screen("DP-1", true)]);
        let v = View::fresh();
        let moment = Instant::now();
        let narrow = v.rows(&now, 40, moment, &texts());
        let wide = v.rows(&now, 200, moment, &texts());
        assert_eq!(narrow.len(), wide.len());
        for (a, b) in narrow.iter().zip(&wide) {
            assert_eq!(a.target, b.target);
            assert_eq!(a.heading, b.heading);
            assert_eq!(a.disabled, b.disabled);
        }
    }

    #[test]
    fn answering_the_question_gives_the_cursor_its_row_back() {
        // A QUESTION REPLACES THE SHEET, and the hand that turned the scale is
        // still on the scale. Coming out of the question four rows above it
        // means the next arrow moves something else — and on this view the
        // something else is the picture the person is trying to read.
        //
        // All three answers, because each used to clear the question in its
        // own place: `Esc`, "put it back" and "keep it".
        let now = snapshot(vec![screen("eDP-1", true)]);
        let moment = Instant::now();
        for answer in [Press::Escape, Press::Enter, Press::Down] {
            let mut v = View::fresh();
            v.key(Press::Enter, &now, moment, &texts());
            // WALKED TO, NOT COUNTED TO: how many rows come before the scale
            // depends on how much the monitor says about itself — one with no
            // physical size in its EDID has one fact fewer.
            for _ in 0..20 {
                let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
                if rows[v.cursor].target == Target::Scale {
                    break;
                }
                v.key(Press::Down, &now, moment, &texts());
            }
            let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
            assert_eq!(rows[v.cursor].target, Target::Scale, "not on the scale");

            v.key(Press::More, &now, moment, &texts());
            assert!(v.asking.is_some(), "the scale did not put a question up");
            // `Down` walks off "put it back" onto "keep it", so the three
            // answers here are refuse, refuse and KEEP.
            if answer == Press::Down {
                v.key(Press::Down, &now, moment, &texts());
                v.key(Press::Enter, &now, moment, &texts());
            } else {
                v.key(answer, &now, moment, &texts());
            }
            assert!(v.asking.is_none(), "the question stayed up");

            let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
            assert_eq!(
                rows[v.cursor].target,
                Target::Scale,
                "the cursor did not come back to the row that armed it",
            );
        }
    }

    /// Walks the sheet to a row and answers what the view was asked for.
    fn on_the_row(now: &Snapshot, target: &Target) -> View {
        let moment = Instant::now();
        let mut v = View::fresh();
        v.key(Press::Enter, now, moment, &texts());
        for _ in 0..24 {
            let rows = v.rows(now, DECIDING_WIDTH, moment, &texts());
            if rows[v.cursor].target == *target {
                return v;
            }
            v.key(Press::Down, now, moment, &texts());
        }
        panic!("the sheet has no {target:?} to walk to");
    }

    fn asked_for(errand: &Errand) -> Ask {
        match errand {
            Errand::Order(o) => match &**o {
                ronoctua_display::Order::Try { ask, .. } => *ask,
                other => panic!("not a `Try`: {other:?}"),
            },
            other => panic!("no order: {other:?}"),
        }
    }

    fn kept(errand: &Errand) -> ronoctua_display::fragment::Wanted {
        match errand {
            Errand::Order(o) => match &**o {
                ronoctua_display::Order::Keep { wanted, .. } => (**wanted).clone(),
                other => panic!("not a `Keep`: {other:?}"),
            },
            other => panic!("no order: {other:?}"),
        }
    }

    #[test]
    fn a_door_that_cannot_see_the_file_does_not_step_the_text() {
        // THE STEP IS COMPUTED BEFORE THE ORDER LEAVES, from the size this
        // PROCESS is drawing at — a number that only moves when the watch on
        // `display.json` fires. A door whose watch failed keeps its start-up
        // number for ever, so its next press asks for a size derived from a
        // baseline the file has left behind; with two doors open that is
        // somebody's chosen size jumping backwards, once per press.
        let now = snapshot(vec![screen("eDP-1", true)]);
        let moment = Instant::now();
        let mut v = View::fresh();
        v.without_a_watch();
        v.at = Some(Target::Text);
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        v.settle(&now, &rows);
        assert_eq!(rows[v.cursor].target, Target::Text);
        assert!(
            rows[v.cursor].disabled,
            "a step it cannot compute is offered"
        );
        assert_eq!(
            v.key(Press::More, &now, moment, &texts()),
            Errand::Nothing,
            "a blind door stepped the size of the text",
        );

        // AND WITH THE WATCH IT DOES, which is what makes the refusal a
        // refusal rather than a row that never works.
        let mut seeing = View::fresh();
        seeing.at = Some(Target::Text);
        let rows = seeing.rows(&now, DECIDING_WIDTH, moment, &texts());
        seeing.settle(&now, &rows);
        assert!(!rows[seeing.cursor].disabled);
        assert!(matches!(
            seeing.key(Press::More, &now, moment, &texts()),
            Errand::Order(_)
        ));
    }

    #[test]
    fn holding_the_text_arrow_moves_more_than_one_step() {
        // THE SIZE ONLY MOVES AFTER A ROUND TRIP — file, inotify, reload —
        // and the step was computed from the process's own size every time,
        // so two presses inside one trip asked for the same number twice and
        // the second did nothing.
        let now = snapshot(vec![screen("eDP-1", true)]);
        let moment = Instant::now();
        let mut v = View::fresh();
        v.at = Some(Target::Text);
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        v.settle(&now, &rows);
        assert_eq!(rows[v.cursor].target, Target::Text);

        let first = asked_size(&v.key(Press::More, &now, moment, &texts()));
        let second = asked_size(&v.key(Press::More, &now, moment, &texts()));
        assert!(
            second > first,
            "two presses asked for {first} and then {second}",
        );
        // AND THE ROW SHOWS WHAT WAS ASKED, not the size that has not landed.
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        assert!(
            rows[v.cursor]
                .fields
                .iter()
                .any(|f| f.text.contains(&second.to_string())),
            "the row did not say what the key asked for",
        );
    }

    fn asked_size(errand: &Errand) -> u16 {
        match errand {
            Errand::Order(o) => match &**o {
                ronoctua_display::Order::Text(percent) => *percent,
                other => panic!("not a text order: {other:?}"),
            },
            other => panic!("no order: {other:?}"),
        }
    }

    #[test]
    fn handing_a_knob_back_works_where_the_arrows_have_nowhere_to_go() {
        // `a` WAS GATED BY THE ROW'S REFUSAL, and `disabled` means the ARROWS
        // have nowhere to go — one mode, one rate — which is exactly when a
        // mode IS written down and handing it back is the thing to do. The
        // key was swallowed on the one row it was for.
        let mut screens = vec![screen("eDP-1", true)];
        // A size with one rate, pinned: the Rate row then refuses the arrows.
        screens[0].wanted.mode = Some(ronoctua_display::fragment::Chosen {
            width: 1280,
            height: 720,
            rate: Some(60.0),
        });
        screens[0].current = Some(2);
        let now = snapshot(screens);
        let moment = Instant::now();
        let mut v = on_the_row(&now, &Target::Rate);
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        assert!(
            rows[v.cursor].disabled,
            "the row this is about is not refused"
        );
        let out = v.key(Press::Automatic, &now, moment, &texts());
        assert_eq!(asked_for(&out), Ask::Mode(None), "`a` was swallowed");
    }

    #[test]
    fn holding_the_place_arrow_moves_more_than_one_step() {
        // THE SAME ROUND TRIP THE TEXT ROW HAS. A place is written to the file
        // and only comes back in the next reading, and the step used to be
        // taken from `Screen::logical` — the PICTURE — so two presses inside
        // one trip both stepped from the same number and asked for the same
        // place. The second was then refused by `keep_for`'s equality guard,
        // which compares against the equally stale `Wanted`: a held arrow
        // moved a monitor once per round trip instead of once per press.
        let now = snapshot(vec![screen("eDP-1", true), screen("DP-1", true)]);
        let moment = Instant::now();
        let mut v = View::fresh();
        v.go_to(Level::Arrange);
        v.at = Some(Target::Place("eDP-1".to_owned()));
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        v.settle(&now, &rows);
        assert_eq!(rows[v.cursor].target, Target::Place("eDP-1".to_owned()));

        let spot = |errand: &Errand| {
            kept(errand)
                .position
                .map(|p| p.x)
                .expect("no place was written")
        };
        let first = spot(&v.key(Press::More, &now, moment, &texts()));
        let second = spot(&v.key(Press::More, &now, moment, &texts()));
        assert_eq!(
            second - first,
            NUDGE,
            "two presses asked for the same place"
        );

        // AND THE ROW SHOWS WHAT WAS ASKED, not the place that has not landed.
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        let said = rows
            .iter()
            .find(|r| r.target == Target::Place("eDP-1".to_owned()))
            .map(|r| r.fields.iter().map(|f| f.text.clone()).collect::<String>())
            .expect("no place row");
        assert!(said.contains(&second.to_string()), "{said}");

        // AND IT IS LET GO OF WHEN THE READING SAYS IT ARRIVED.
        let mut landed = vec![screen("eDP-1", true), screen("DP-1", true)];
        landed[0].wanted.position = Some(ronoctua_display::fragment::Spot { x: second, y: 0 });
        let landed = snapshot(landed);
        let rows = v.rows(&landed, DECIDING_WIDTH, moment, &texts());
        v.settle(&landed, &rows);
        assert!(v.place_pending.is_none(), "the place never landed");

        // AND A REFUSAL LETS GO OF IT TOO, or the row would hold a place the
        // file does not have for the life of the view — but only the refusal
        // of THAT order. Every knob that writes straight down emits a `Keep`.
        let out = v.key(Press::More, &now, moment, &texts());
        let asked = kept(&out);
        assert!(v.place_pending.is_some());
        v.place_never_landed("DP-1", &asked);
        assert!(
            v.place_pending.is_some(),
            "a refusal about another screen dropped this place",
        );
        v.place_never_landed("eDP-1", &ronoctua_display::fragment::Wanted::default());
        assert!(
            v.place_pending.is_some(),
            "a refusal of another change dropped this place",
        );
        v.place_never_landed("eDP-1", &asked);
        assert!(v.place_pending.is_none());
    }

    #[test]
    fn handing_back_a_place_nobody_chose_writes_nothing() {
        // THE DEFAULT STATE OF EVERY MACHINE nobody has placed a monitor on.
        // `a` on the place row cleared a position that was already unset and
        // emitted a `Keep` anyway — a reading, a rewrite of `display.json` and
        // six IPC calls, once per keyboard repeat, redrawing byte for byte.
        //
        // It had a guard while it was a question: `would_move` refuses one
        // that would not move. The round that made this row write straight
        // down left that behind with nothing in its place, so the rule now
        // lives in `keep_for` and holds for every knob that writes.
        let now = snapshot(vec![screen("eDP-1", true), screen("DP-1", true)]);
        let moment = Instant::now();
        let mut v = on_the_row(&now, &Target::Place("eDP-1".to_owned()));
        assert!(
            now.screen("eDP-1")
                .is_some_and(|s| s.wanted.position.is_none())
        );
        assert_eq!(
            v.key(Press::Automatic, &now, moment, &texts()),
            Errand::Nothing,
            "handing back a place nobody chose wrote the file",
        );

        // AND WITH ONE CHOSEN IT WRITES, which is what makes the refusal above
        // a refusal rather than a key that never works.
        let mut screens = vec![screen("eDP-1", true), screen("DP-1", true)];
        screens[0].wanted.position = Some(ronoctua_display::fragment::Spot { x: 100, y: 0 });
        let placed = snapshot(screens);
        let mut v = on_the_row(&placed, &Target::Place("eDP-1".to_owned()));
        let out = v.key(Press::Automatic, &placed, moment, &texts());
        assert!(
            kept(&out).position.is_none(),
            "the place was not handed back: {out:?}",
        );
    }

    #[test]
    fn the_light_row_says_whether_it_can_be_moved() {
        // NO FIXTURE IN THE WHOLE REPOSITORY HAD A LIGHT. `screen()` builds
        // `light: None`, so `Target::Light`, its refusal and the sentence that
        // explains it were named in no test at all — on the knob a laptop
        // owner touches most.
        let moment = Instant::now();
        let t = texts();
        let row_of = |now: &Snapshot| {
            let mut v = View::fresh();
            v.go_to(Level::Monitor("eDP-1".to_owned()));
            v.rows(now, DECIDING_WIDTH, moment, &t)
                .into_iter()
                .find(|r| r.target == Target::Light)
                .expect("no brightness row")
        };
        let with = |light| {
            let mut screens = vec![screen("eDP-1", true)];
            screens[0].light = Some(light);
            snapshot(screens)
        };

        // A PANEL THIS PROCESS MAY WRITE reads a percentage and answers.
        let open = with(ronoctua_display::Light::Panel(
            ronoctua_display::backlight::Panel {
                at: "intel_backlight".to_owned(),
                now: 40,
                max: 100,
                writable: true,
            },
        ));
        let row = row_of(&open);
        assert!(!row.disabled, "a panel that can be written refuses");
        assert!(
            row.fields.iter().any(|f| f.text.contains("40")),
            "the row does not say where the brightness is: {row:?}",
        );

        // AND ONE THAT BELONGS TO ROOT SAYS SO BEFORE THE KEY IS PRESSED.
        let shut = with(ronoctua_display::Light::Panel(
            ronoctua_display::backlight::Panel {
                at: "intel_backlight".to_owned(),
                now: 40,
                max: 100,
                writable: false,
            },
        ));
        let row = row_of(&shut);
        assert!(
            row.disabled,
            "a panel that refuses every write is drawn live"
        );
        // BY THE HEAD OF THE SENTENCE, because the row ELIDES: a value longer
        // than the line's own room comes back with an ellipsis, and comparing
        // the whole of it would be asserting the width rather than the words.
        let said = t.t(ronoctua_i18n::key::DISPLAY_LIGHT_IS_ROOTS);
        let head: String = said.chars().take(20).collect();
        assert!(
            row.fields.iter().any(|f| f.text.starts_with(&head)),
            "the row refuses without saying why: {row:?}",
        );

        // AND THE ARROWS REACH IT, which is what makes the row worth drawing.
        let mut v = on_the_row(&open, &Target::Light);
        let out = v.key(Press::More, &open, moment, &texts());
        match out {
            Errand::Order(o) => match *o {
                ronoctua_display::Order::Bright { by, .. } => assert!(by > 0),
                other => panic!("not a brightness order: {other:?}"),
            },
            other => panic!("no order: {other:?}"),
        }
        // AND ON THE ONE THAT REFUSES, NOTHING.
        let mut v = on_the_row(&shut, &Target::Light);
        assert_eq!(
            v.key(Press::More, &shut, moment, &texts()),
            Errand::Nothing,
            "a panel that belongs to root took the key anyway",
        );
    }

    #[test]
    fn the_session_opens_on_one_screen_and_the_others_lose_it() {
        // `Errand::Orders` EXISTED IN NO TEST. niri takes the LAST
        // `focus-at-startup` it reads, so two screens both carrying it is a
        // file that says one thing and a session that does another — and the
        // rows would both read "yes". This is the one gesture in the view that
        // is about more than one screen.
        let mut screens = vec![screen("eDP-1", true), screen("DP-1", true)];
        screens[0].wanted.first = true;
        let now = snapshot(screens);
        let moment = Instant::now();
        // THE SHEET OF THE ONE THAT DOES NOT HAVE IT, which `on_the_row`
        // cannot reach: it opens the first screen of the list.
        let mut v = View::fresh();
        v.go_to(Level::Monitor("DP-1".to_owned()));
        v.at = Some(Target::First);
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        v.settle(&now, &rows);
        assert_eq!(rows[v.cursor].target, Target::First);

        // `→` IS YES, and this screen does not have it.
        assert_eq!(
            v.key(Press::Less, &now, moment, &texts()),
            Errand::Nothing,
            "`less` on a screen that does not have it did something",
        );
        let out = v.key(Press::More, &now, moment, &texts());
        let Errand::Orders(orders) = out else {
            panic!("not a batch of orders: {out:?}");
        };
        assert_eq!(orders.len(), 2, "{orders:?}");
        // IN THE ORDER THEY WERE MADE, and the LAST is the one the person
        // asked for — the others are the ones losing it.
        let took: Vec<(String, bool)> = orders
            .iter()
            .map(|o| match o {
                ronoctua_display::Order::Keep { at, wanted } => (at.clone(), wanted.first),
                other => panic!("not a keep: {other:?}"),
            })
            .collect();
        assert_eq!(
            took,
            vec![("eDP-1".to_owned(), false), ("DP-1".to_owned(), true)],
        );

        // AND ASKING FOR THE ONE THAT ALREADY HAS IT WRITES NOTHING.
        let mut v = View::fresh();
        v.go_to(Level::Monitor("eDP-1".to_owned()));
        v.at = Some(Target::First);
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        v.settle(&now, &rows);
        assert_eq!(rows[v.cursor].target, Target::First);
        // AND THE DIRECTION DECIDES, as it does on the power row: right is
        // yes. `→` on the screen that already has it writes nothing; `←` takes
        // it away, and takes it from nobody else.
        assert_eq!(
            v.key(Press::More, &now, moment, &texts()),
            Errand::Nothing,
            "`more` on the screen the session already opens on did something",
        );
        let out = v.key(Press::Less, &now, moment, &texts());
        let Errand::Order(o) = out else {
            panic!("turning the flag off should be one order: {out:?}");
        };
        match *o {
            ronoctua_display::Order::Keep { at, wanted } => {
                assert_eq!(at, "eDP-1");
                assert!(!wanted.first);
            }
            other => panic!("not a keep: {other:?}"),
        }
    }

    #[test]
    fn the_vary_knob_writes_nothing_at_its_ends() {
        // `Order::Keep` costs a reading to file the screen by name, a write,
        // and six IPC calls saying what the file says — per keyboard repeat.
        // Both ends of this three-state knob answered themselves and wrote
        // anyway; every other knob in the view already refuses.
        let mut screens = vec![screen("eDP-1", true)];
        screens[0].vrr_supported = true;
        screens[0].wanted.vary = Some(ronoctua_display::fragment::Vary::Always);
        let now = snapshot(screens);
        let moment = Instant::now();
        let mut v = on_the_row(&now, &Target::Vary);
        assert_eq!(
            v.key(Press::More, &now, moment, &texts()),
            Errand::Nothing,
            "the top of the knob wrote the file",
        );

        let mut screens = vec![screen("eDP-1", true)];
        screens[0].vrr_supported = true;
        screens[0].wanted.vary = None;
        let bottom = snapshot(screens);
        let mut v = on_the_row(&bottom, &Target::Vary);
        assert_eq!(
            v.key(Press::Less, &bottom, moment, &texts()),
            Errand::Nothing,
            "the bottom of the knob wrote the file",
        );
    }

    #[test]
    fn a_screen_the_compositor_reports_no_mode_for_has_no_mode_rows() {
        // `Screen::current` is an `Option` and `None` does not mean dark, so
        // both rows were drawn LIVE reading "the compositor chooses" — and
        // both arrows died silently, the resolution after filing the unknown
        // at the top of its own list.
        let mut screens = vec![screen("eDP-1", true)];
        screens[0].current = None;
        let now = snapshot(screens);
        let moment = Instant::now();
        let mut v = View::fresh();
        v.level = Level::Monitor("eDP-1".to_owned());
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        for target in [Target::Resolution, Target::Rate] {
            let row = rows
                .iter()
                .find(|r| r.target == target)
                .unwrap_or_else(|| panic!("no {target:?} row"));
            assert!(row.disabled, "{target:?} is drawn live with a dead arrow");
        }
    }

    #[test]
    fn the_hint_teaches_only_what_works_where_one_is() {
        // A LEVEL OUTLIVES THE THING IT IS ABOUT: a sheet survives its monitor
        // being unplugged, and a one-screen list has an arrangement key that
        // opens nothing. Both were taught anyway. And the other way about: `a`
        // works on the arrangement and was filed under the sheet, so it worked
        // and was taught nowhere.
        let t = texts();
        let alone = snapshot(vec![screen("eDP-1", true)]);
        let said = |v: &View, now: &Snapshot| {
            v.hint(now, Instant::now(), &t)
                .iter()
                .map(|p| p.text.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let on_the_list = View::fresh();
        assert!(
            !said(&on_the_list, &alone).contains(&t.t(ronoctua_i18n::key::DISPLAY_ARRANGE)),
            "one screen, and the line still teaches the arrangement key",
        );

        let two = snapshot(vec![screen("eDP-1", true), screen("DP-1", true)]);
        assert!(
            said(&on_the_list, &two).contains(&t.t(ronoctua_i18n::key::DISPLAY_ARRANGE)),
            "two screens, and the line does not teach it",
        );

        // AND THE ARRANGEMENT TEACHES `a` WHEN THERE IS A PLACE TO HAND BACK.
        // The fixture has to SAY so: over two default screens `a` refuses on
        // every row, and asserting the letter is taught there was asserting
        // the defect.
        let mut placed = vec![screen("eDP-1", true), screen("DP-1", true)];
        placed[1].wanted.position = Some(ronoctua_display::fragment::Spot { x: 1920, y: 0 });
        let placed = snapshot(placed);
        let mut arranging = View::fresh();
        arranging.level = Level::Arrange;
        assert!(
            said(&arranging, &placed).contains(&t.t(ronoctua_i18n::key::DISPLAY_AUTOMATIC)),
            "`a` works on the arrangement and is taught nowhere",
        );
        assert!(
            !said(&arranging, &two).contains(&t.t(ronoctua_i18n::key::DISPLAY_AUTOMATIC)),
            "nobody has placed a monitor, and the line teaches the key that puts one back",
        );

        // AND A SHEET WHOSE SCREEN WENT AWAY teaches neither letter.
        let mut orphan = View::fresh();
        orphan.level = Level::Monitor("gone".to_owned());
        let line = said(&orphan, &alone);
        assert!(
            !line.contains(&t.t(ronoctua_i18n::key::DISPLAY_AUTOMATIC)),
            "{line}"
        );
        assert!(
            !line.contains(&t.t(ronoctua_i18n::key::DISPLAY_FORGET)),
            "{line}"
        );
    }

    #[test]
    fn a_question_raised_from_a_fact_comes_back_to_that_fact() {
        // `d` ACTS ON THE SCREEN, so it is reachable from a fact — where the
        // cursor has no target to be remembered by (`Target::Nothing` names
        // every fact at once). Answering then left it on the question's own
        // row, four lines from where the hand was.
        let mut screens = vec![screen("eDP-1", true)];
        screens[0].wanted.scale = Some(1.5);
        let now = snapshot(screens);
        let moment = Instant::now();
        let mut v = View::fresh();
        v.key(Press::Enter, &now, moment, &texts());
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        let was = v.cursor;
        assert_eq!(rows[was].target, Target::Nothing, "not standing on a fact");

        v.key(Press::Forget, &now, moment, &texts());
        assert!(v.asking.is_some(), "`d` raised no question from a fact");
        v.key(Press::Escape, &now, moment, &texts());
        assert_eq!(v.cursor, was, "the cursor did not come back to the fact");
    }

    #[test]
    fn a_dark_screen_has_no_knob_to_hand_back() {
        // `a` IS NOT GATED BY THE ROW'S REFUSAL, deliberately — and on a dark
        // screen every picture row is refused for a reason that is about the
        // whole screen. It wrote the setting away and the row redrew
        // identically, because a dark row's value is the sentence saying the
        // compositor is not drawing on it and does not read `wanted`.
        let mut screens = vec![screen("eDP-1", true), screen("DP-1", false)];
        screens[1].wanted.scale = Some(1.5);
        let now = snapshot(screens);
        let moment = Instant::now();
        let mut v = View::fresh();
        v.level = Level::Monitor("DP-1".to_owned());
        v.at = Some(Target::Scale);
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        v.settle(&now, &rows);
        assert_eq!(rows[v.cursor].target, Target::Scale);
        assert_eq!(
            v.key(Press::Automatic, &now, moment, &texts()),
            Errand::Nothing,
            "`a` threw a setting away on a screen nobody is looking at",
        );
        assert!(v.asking.is_none());

        // AND THE LINE DOES NOT TEACH IT EITHER. A key that refuses on every
        // row of a level, named in the line above those rows, is the same
        // defect as an arrow with nowhere to go — and it is worse here,
        // because a dark screen is the screen somebody came to this view to
        // rescue.
        let t = texts();
        let line = v
            .hint(&now, Instant::now(), &t)
            .iter()
            .map(|p| p.text.clone())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            !line.contains(&t.t(ronoctua_i18n::key::DISPLAY_AUTOMATIC)),
            "a dark sheet taught `a`, which refuses on every one of its rows: {line}",
        );
        // AND ON A LIT ONE WITH SOMETHING WRITTEN DOWN IT IS TAUGHT. The
        // second half is not decoration: `a` HANDS BACK, and on a screen
        // nobody has chosen anything for there is nothing to hand — which is
        // every machine on its first morning, and was the state this
        // assertion used to be made in.
        let mut chosen = vec![screen("eDP-1", true), screen("DP-1", false)];
        chosen[0].wanted.scale = Some(1.5);
        let chosen = snapshot(chosen);
        let said = |v: &View, now: &Snapshot| {
            v.hint(now, Instant::now(), &t)
                .iter()
                .map(|p| p.text.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let mut on = View::fresh();
        on.level = Level::Monitor("eDP-1".to_owned());
        let line = said(&on, &chosen);
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_AUTOMATIC)),
            "{line}"
        );
        let line = said(&on, &now);
        assert!(
            !line.contains(&t.t(ronoctua_i18n::key::DISPLAY_AUTOMATIC)),
            "nothing is written down for this screen, and `a` is taught anyway: {line}",
        );
    }

    #[test]
    fn the_arrows_are_not_taught_where_nothing_turns() {
        // `WhileNothingIsAsked` hid them under a question and covered only
        // that. On an orphan sheet the only row is a sentence, and both arrows
        // fall to `Errand::Nothing` — a laptop leaving a dock is the everyday
        // way to get there.
        let t = texts();
        let two = snapshot(vec![screen("eDP-1", true), screen("DP-1", true)]);
        let said = |v: &View, now: &Snapshot| {
            v.hint(now, Instant::now(), &t)
                .iter()
                .map(|p| p.text.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let mut orphan = View::fresh();
        orphan.level = Level::Monitor("gone".to_owned());
        let line = said(&orphan, &two);
        assert!(
            !line.contains(&t.t(ronoctua_i18n::key::DISPLAY_LESS)),
            "{line}"
        );
        assert!(
            !line.contains(&t.t(ronoctua_i18n::key::DISPLAY_MORE)),
            "{line}"
        );

        // AND ON A SHEET THAT IS ABOUT A SCREEN, they are taught.
        let mut here = View::fresh();
        here.level = Level::Monitor("eDP-1".to_owned());
        assert!(said(&here, &two).contains(&t.t(ronoctua_i18n::key::DISPLAY_MORE)));

        // AND ON A DARK SHEET THEY ARE STILL TAUGHT, because one row there
        // still answers — and this assertion used to say the opposite.
        //
        // The claim it made was that on a screen the compositor is not driving
        // "both arrows do nothing on every row". That is true of every PICTURE
        // row and of the power toggle, and false of "opens focused here",
        // which is disabled on `wanted.off` and deliberately NOT on dark: a
        // docked laptop with the lid shut is dark with `off == false`, and the
        // file does carry `focus-at-startup` for it. So `→` there returns an
        // `Order` and rewrites `display.json` and the fragment.
        //
        // It passed because it only asserted the ABSENCE of `less` — and `←`
        // genuinely does nothing, since the row is already `first == false`.
        // One arrow of a pair is not the pair, and the two share one gate.
        let lid = snapshot(vec![screen("eDP-1", true), screen("DP-1", false)]);
        let mut shut = View::fresh();
        shut.level = Level::Monitor("DP-1".to_owned());
        let line = said(&shut, &lid);
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_MORE)),
            "a key that rewrites the file is taught nowhere: {line}"
        );
        // AND IT IS THAT ROW THAT TAKES THEM — asserted by pressing, so the
        // line above cannot go on being right for the wrong reason.
        let mut pressing = View::fresh();
        pressing.level = Level::Monitor("DP-1".to_owned());
        let rows = pressing.rows(&lid, DECIDING_WIDTH, Instant::now(), &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::First)
            .expect("the sheet has no row for what opens focused here");
        pressing.cursor = at;
        // AND THE REMEMBERED TARGET WITH IT. `key` runs `settle` first, and
        // `settle` finds the cursor again by `View::at` — so planting only the
        // index leaves the press landing wherever the view last was, which is
        // how the sibling test below passed while testing nothing.
        pressing.at = Some(Target::First);
        assert!(
            !matches!(
                pressing.key(Press::More, &lid, Instant::now(), &t),
                Errand::Nothing
            ),
            "the row the hint promises does nothing",
        );

        // AND ON ONE THIS VIEW TURNED OFF THEY ARE, because the power row
        // answers them.
        let mut ours = vec![screen("eDP-1", true), screen("DP-1", false)];
        ours[1].wanted.off = true;
        let ours = snapshot(ours);
        assert!(said(&shut, &ours).contains(&t.t(ronoctua_i18n::key::DISPLAY_MORE)));
    }

    #[test]
    fn a_level_with_no_row_does_not_copy_its_own_title() {
        // `first_row` ANSWERS 0 when nothing on a level may hold the cursor,
        // and two levels are like that: an arrangement of fewer than two lit
        // screens, and a sheet whose monitor went away. Row 0 is the title,
        // and `RET` there put it on the clipboard — the same defect prose was
        // taken off the cursor for, one row up.
        let now = snapshot(vec![screen("eDP-1", true)]);
        let moment = Instant::now();

        let mut arranging = View::fresh();
        arranging.go_to(Level::Arrange);
        let rows = arranging.rows(&now, DECIDING_WIDTH, moment, &texts());
        assert!(
            rows.iter().all(|r| r.heading),
            "this level is meant to have no row the cursor can stand on",
        );
        assert_eq!(
            arranging.key(Press::Enter, &now, moment, &texts()),
            Errand::Nothing,
            "`RET` copied the level's own title",
        );

        let mut orphan = View::fresh();
        orphan.go_to(Level::Monitor("HDMI-1".to_owned()));
        assert_eq!(
            orphan.key(Press::Enter, &now, moment, &texts()),
            Errand::Nothing,
            "`RET` copied the title of a sheet whose screen went away",
        );

        // AND A ROW STILL COPIES, which is what makes the two refusals above
        // refusals rather than a key that stopped working.
        let mut sheet = on_the_row(&now, &Target::Scale);
        assert!(matches!(
            sheet.key(Press::Enter, &now, moment, &texts()),
            Errand::Copy(_)
        ));
    }

    #[test]
    fn a_door_that_has_not_read_yet_does_not_blame_the_compositor() {
        // THREE SILENCES HAD SENTENCES AND A FOURTH BORROWED ONE. A
        // `Snapshot::default()` says `compositor: false`, and the window's
        // first frame is drawn against exactly that — so the screens' own
        // window opened, on a working machine, saying there was no compositor
        // to ask, and held it for as long as the first DEEP reading took (a
        // DDC/CI exchange per monitor, three tries fifty milliseconds apart
        // for one that does not answer).
        let t = texts();
        let moment = Instant::now();
        let says = |v: &View, now: &Snapshot, key: &str| {
            let want = t.t(key);
            v.rows(now, DECIDING_WIDTH, moment, &t)
                .iter()
                .any(|r| r.fields.iter().any(|f| f.text == want))
        };
        let nothing = ronoctua_display::Snapshot::default();

        let fresh = View::fresh();
        assert!(says(&fresh, &nothing, ronoctua_i18n::key::DISPLAY_READING));
        assert!(
            !says(&fresh, &nothing, ronoctua_i18n::key::DISPLAY_NO_COMPOSITOR),
            "the first frame blamed the compositor for not having been asked yet",
        );

        // AND AFTER A READING IT SAYS WHAT THE READING SAID.
        let mut read_to = View::fresh();
        read_to.saw_a_reading();
        assert!(says(
            &read_to,
            &nothing,
            ronoctua_i18n::key::DISPLAY_NO_COMPOSITOR
        ));
        assert!(!says(
            &read_to,
            &nothing,
            ronoctua_i18n::key::DISPLAY_READING
        ));
    }

    #[test]
    fn a_level_with_no_row_teaches_no_key_at_all() {
        // THREE KEYS AND NO ANSWER. `Where` is about the LEVEL, and no `Where`
        // can say whether the level has anything on it — so an orphan sheet
        // and an arrangement of one screen (two headings each) went on
        // teaching `RET`, `↑` and `↓`, all three of which `key` refuses. The
        // owner photographed the same shape on the radio.
        let t = texts();
        let moment = Instant::now();
        let one = snapshot(vec![screen("eDP-1", true)]);
        let said = |v: &View, now: &Snapshot| {
            v.hint(now, moment, &t)
                .iter()
                .map(|p| p.text.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let dead = [
            ronoctua_i18n::key::DISPLAY_ENTER,
            ronoctua_i18n::key::DISPLAY_ENTER_COPIES,
            ronoctua_i18n::key::DISPLAY_UP,
            ronoctua_i18n::key::DISPLAY_DOWN,
        ];

        let mut orphan = View::fresh();
        orphan.go_to(Level::Monitor("HDMI-1".to_owned()));
        let line = said(&orphan, &one);
        for key in dead {
            assert!(!line.contains(&t.t(key)), "{line}");
        }
        // AND IT IS NOT BLANK. A level whose every key is dead teaches the one
        // that is not — the way out — because a strip with nothing on it is
        // dropped by the footer entirely, and `Esc` is in no view's table.
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_BACK)),
            "the line came out empty: {line:?}",
        );

        let mut arranging = View::fresh();
        arranging.go_to(Level::Arrange);
        let line = said(&arranging, &one);
        for key in dead {
            assert!(!line.contains(&t.t(key)), "{line}");
        }
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_BACK)),
            "the line came out empty: {line:?}",
        );

        // AND A LIST WHOSE TWO ROWS BOTH REFUSE loses `RET` and KEEPS the
        // arrows. A disabled row is one the cursor may stand on — that is what
        // "drawn reachable and refusing" means, and walking to it is how a
        // person reads the sentence saying why. So `↑`/`↓` are not dead there;
        // `RET` is.
        let mut bare = snapshot(vec![]);
        bare.compositor = false;
        let mut blind = View::fresh();
        blind.without_a_reader();
        let line = said(&blind, &bare);
        assert!(
            !line.contains(&t.t(ronoctua_i18n::key::DISPLAY_ENTER)),
            "{line}"
        );
        assert!(
            !line.contains(&t.t(ronoctua_i18n::key::DISPLAY_ENTER_COPIES)),
            "{line}"
        );
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_DOWN)),
            "{line}"
        );
        // AND IT TEACHES THE WAY OUT, which this block did not assert and the
        // line did not carry. The two neighbouring blocks both check it; this
        // one checked only that `RET` was gone and that the arrows stayed, so
        // the state shipped teaching `down · up` over two rows that refuse
        // everything — a cursor to move, nothing to move it to, and the only
        // key that works taught nowhere. `is_empty` cannot see this case,
        // because two disabled rows are two walkable rows.
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_BACK)),
            "the only working key is missing from the line: {line:?}",
        );

        // AND A LEVEL WITH ROWS TEACHES ALL THREE, which is what makes the
        // three refusals above refusals rather than a line that never speaks.
        let line = said(&View::fresh(), &one);
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_ENTER)),
            "{line}"
        );
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_UP)),
            "{line}"
        );
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_DOWN)),
            "{line}"
        );
    }

    #[test]
    fn the_arrangement_says_what_its_keys_do() {
        // `RET` MEANS THREE THINGS AND THE LINE SAID ONE. The arrangement's
        // rows are all places: `RET` on one copies the value, as it does on
        // any row that is not a door — but the table's own word is "open", and
        // an early `return` written for the arrows took this level out of the
        // answer that would have said otherwise.
        let t = texts();
        let now = snapshot(vec![screen("eDP-1", true), screen("DP-1", true)]);
        let said = |v: &View| {
            v.hint(&now, Instant::now(), &t)
                .iter()
                .map(|p| p.text.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let mut arranging = View::fresh();
        arranging.level = Level::Arrange;
        let line = said(&arranging);
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_ENTER_COPIES)),
            "the arrangement says `RET` opens something: {line}",
        );
        assert!(
            !line.contains(&t.t(ronoctua_i18n::key::DISPLAY_ENTER)),
            "{line}"
        );
        // AND THE ARROWS SAY WHICH WAY, not how much.
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_LEFTWARDS)),
            "{line}"
        );
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_RIGHTWARDS)),
            "{line}"
        );

        // AND THE LIST KEEPS "open", because its rows open.
        let line = said(&View::fresh());
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_ENTER)),
            "{line}"
        );

        // BUT NOT A LIST WITH NOTHING TO OPEN: with no screen row and the
        // arrangement row refusing, the only `RET` left is the copy.
        let mut bare = snapshot(vec![]);
        bare.compositor = false;
        let line = View::fresh()
            .hint(&bare, Instant::now(), &t)
            .iter()
            .map(|p| p.text.clone())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            line.contains(&t.t(ronoctua_i18n::key::DISPLAY_ENTER_COPIES)),
            "a list with nothing on it says `RET` opens something: {line}",
        );
    }

    #[test]
    fn a_refusal_takes_down_only_its_own_question() {
        // A REFUSAL ARRIVES AFTER A ROUND TRIP, and by then the question it
        // was about may be gone and another one up. Clearing the second with
        // no `Undo` leaves a change that WAS applied standing for ever.
        let now = snapshot(vec![screen("eDP-1", true), screen("DP-1", true)]);
        let moment = Instant::now();
        let mut v = on_the_row(&now, &Target::Scale);
        let sent = v.key(Press::More, &now, moment, &texts());
        let asked = asked_for(&sent);
        let standing = v.asking.clone().expect("no question");
        assert_eq!(standing.at, "eDP-1");

        // AND THE ANSWER IS ASSERTED, not discarded. It is what both doors
        // read to decide whether to settle the cursor and pull the page to
        // it — a `never_mind` that answered `true` unconditionally would put
        // back the rule that a daemon's arrival does not follow the cursor,
        // with every test green.
        assert!(
            !v.never_mind("DP-1", asked),
            "a refusal about another screen said it took a question down",
        );
        assert!(
            v.asking.is_some(),
            "a refusal about another screen took this question down",
        );
        // AND NOT A REFUSAL OF ANOTHER ASK ABOUT THIS ONE. `→`, `Esc`, `→`
        // again is two questions about the same connector inside one round
        // trip, and the first refusal used to take the second down — with no
        // `Undo`, so the change that WAS applied stood for ever.
        let other = Ask::Scale(Some(9.0));
        assert_ne!(other, asked);
        assert!(!v.never_mind("eDP-1", other));
        assert!(
            v.asking.is_some(),
            "a refusal of another change took this question down",
        );
        assert!(
            v.never_mind("eDP-1", asked),
            "its own refusal said nothing moved"
        );
        assert!(v.asking.is_none(), "its own refusal did not take it down");
    }

    #[test]
    fn a_letter_does_not_replace_a_standing_question() {
        // `d` OVERWROTE THE QUESTION, and a preview replaced that way could
        // never go back: the forget question has no clock, so `ran_out` never
        // fires, and both its answers return without an `Undo`. A screen
        // turned sideways provisionally and then `d` stayed sideways for ever.
        let mut screens = vec![screen("eDP-1", true)];
        screens[0].wanted.scale = Some(1.5);
        let now = snapshot(screens);
        let moment = Instant::now();
        let mut v = on_the_row(&now, &Target::Turn);
        v.key(Press::More, &now, moment, &texts());
        let armed = v.asking.clone().expect("the turn raised no question");
        assert!(armed.on_the_clock());

        for letter in [Press::Forget, Press::Automatic] {
            v.key(letter, &now, moment, &texts());
            assert_eq!(
                v.asking.as_ref().map(|a| a.deciding.clone()),
                Some(armed.deciding.clone()),
                "{letter:?} replaced the standing question",
            );
        }
        // AND THE ONE THAT WAS THERE STILL ANSWERS ITSELF.
        assert!(matches!(
            v.ran_out(moment + PATIENCE),
            Errand::Order(o) if *o == ronoctua_display::Order::Undo
        ));
    }

    #[test]
    fn forgetting_is_about_the_screen_and_not_the_row_under_the_cursor() {
        // IT WAS GATED BY THE ROW. On a one-screen laptop "Driven" is drawn
        // refusing — the last lit screen cannot be turned off — so `d` did
        // nothing there and everything one row down, with no sentence either
        // way, although it acts on the whole screen whatever the cursor is on.
        let mut screens = vec![screen("eDP-1", true)];
        screens[0].wanted.scale = Some(1.5);
        let now = snapshot(screens);
        let moment = Instant::now();
        let mut v = on_the_row(&now, &Target::Power);
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        assert!(
            rows[v.cursor].disabled,
            "the row this is about is not refused"
        );
        v.key(Press::Forget, &now, moment, &texts());
        assert!(
            matches!(
                v.asking.as_ref().map(|a| &a.deciding),
                Some(Deciding::Whether)
            ),
            "`d` was swallowed by a refusing row",
        );
    }

    #[test]
    fn an_arrow_with_nowhere_to_go_raises_no_question() {
        // A QUESTION ABOUT NOTHING. `step` answered the element already
        // selected at one end and `next_scale` the current scale at the top,
        // so the last rung replaced the sheet for fifteen seconds with a
        // sentence saying the picture had changed, and sent an `Undo` for a
        // change nobody had made — the defect the power row already carried.
        let mut screens = vec![screen("eDP-1", true)];
        screens[0].wanted.transform = *Transform::ALL.last().unwrap();
        screens[0].wanted.scale = Some(LADDER[LADDER.len() - 1]);
        let now = snapshot(screens);
        let moment = Instant::now();

        for target in [Target::Turn, Target::Scale] {
            let mut v = on_the_row(&now, &target);
            let out = v.key(Press::More, &now, moment, &texts());
            assert_eq!(out, Errand::Nothing, "{target:?} asked for something");
            assert!(v.asking.is_none(), "{target:?} raised a question anyway");
        }

        // AND `a` ON A KNOB THE COMPOSITOR ALREADY CHOOSES is the same case
        // without a list: nothing is written down, so handing it back changes
        // nothing.
        let fresh = snapshot(vec![screen("eDP-1", true)]);
        let mut v = on_the_row(&fresh, &Target::Scale);
        assert!(
            fresh
                .screen("eDP-1")
                .is_some_and(|s| s.wanted.scale.is_none())
        );
        let out = v.key(Press::Automatic, &fresh, moment, &texts());
        assert_eq!(out, Errand::Nothing);
        assert!(v.asking.is_none(), "`a` raised a question about nothing");
    }

    #[test]
    fn keeping_a_resolution_keeps_the_one_that_was_asked_for() {
        // IT READ THE MODE BACK instead of keeping the ask, so a "keep it"
        // answered before the reading caught up wrote down the mode the person
        // had just changed AWAY from — and a screen whose `current` is `None`
        // wrote "the compositor chooses".
        let now = snapshot(vec![screen("eDP-1", true)]);
        let moment = Instant::now();
        let mut v = on_the_row(&now, &Target::Resolution);
        // `Less`, BECAUSE THE FIXTURE OPENS ON THE LARGEST: the sizes are
        // listed largest first and `More` at the top now has nowhere to go,
        // which is the fix the test above pins.
        let out = v.key(Press::Less, &now, moment, &texts());
        let Ask::Mode(Some(asked)) = asked_for(&out) else {
            panic!("the resolution did not ask for a mode");
        };
        v.key(Press::Down, &now, moment, &texts());
        // THE READING HAS NOT MOVED, which is the case this is about.
        let out = v.key(Press::Enter, &now, moment, &texts());
        let kept = kept(&out).mode.expect("no mode was kept");
        assert_eq!((kept.width, kept.height), (asked.width, asked.height));
        assert_eq!(kept.rate, asked.refresh);
    }

    #[test]
    fn the_power_row_asks_for_the_state_it_is_not_in() {
        // IT ALWAYS ASKED `On`. On a lit screen that is a no-op, and the
        // question that went up over it read "the screen was turned off" —
        // a sentence about something that had not happened, followed fifteen
        // seconds later by an `Undo` for it. Two lit screens, because the last
        // lit one refuses the key.
        let now = snapshot(vec![screen("eDP-1", true), screen("DP-1", true)]);
        let moment = Instant::now();
        let mut v = on_the_row(&now, &Target::Power);
        // AND THE DIRECTION DECIDES: right is lit. Both arrows read the
        // PICTURE and did the same thing, so on a lit screen the key the line
        // calls "more" turned the screen off.
        assert_eq!(
            v.key(Press::More, &now, moment, &texts()),
            Errand::Nothing,
            "`more` on a screen that is already lit did something",
        );
        assert!(v.asking.is_none());
        let out = v.key(Press::Less, &now, moment, &texts());
        assert_eq!(asked_for(&out), Ask::Off, "a lit screen was asked to light");
        assert_eq!(
            v.asking.as_ref().map(|a| a.what),
            Some(ronoctua_i18n::key::DISPLAY_WHAT_POWER_OFF),
        );

        // A DARK SCREEN THIS VIEW TURNED OFF is the one the key is for, and
        // the fixture has to SAY so: the assertion below used to run over a
        // default `Wanted`, where `off` is already `false` and `!kept(&out).off`
        // passed without the write having done anything at all.
        let mut screens = vec![screen("eDP-1", true), screen("DP-1", false)];
        screens[1].wanted.off = true;
        let dark = snapshot(screens);
        let mut v = View::fresh();
        v.level = Level::Monitor("DP-1".to_owned());
        v.at = Some(Target::Power);
        let rows = v.rows(&dark, DECIDING_WIDTH, moment, &texts());
        v.settle(&now, &rows);
        assert_eq!(rows[v.cursor].target, Target::Power);
        assert!(!rows[v.cursor].disabled, "the row this key is for refuses");
        assert_eq!(
            v.key(Press::Less, &dark, moment, &texts()),
            Errand::Nothing,
            "`less` on a screen that is already dark did something",
        );
        let out = v.key(Press::More, &dark, moment, &texts());
        // AND A DARK SCREEN IS WRITTEN, NOT PREVIEWED. There is nothing on it
        // to compare a preview against, and lighting one cannot blind
        // anybody — so the countdown would be fifteen seconds of a sentence
        // about a picture that is not being drawn.
        assert!(v.asking.is_none(), "a dark screen raised a countdown");
        assert!(!kept(&out).off, "a dark screen was not asked to light");

        // AND A SCREEN NOTHING HERE TURNED OFF REFUSES, saying so. The toggle
        // reads the PICTURE and writes a DECISION, and on a shut lid the two
        // disagree: `off` is already `false`, so the key wrote nothing, the
        // screen stayed dark, and the row read a plain "no".
        let lid = snapshot(vec![screen("eDP-1", true), screen("DP-1", false)]);
        let mut v = View::fresh();
        v.level = Level::Monitor("DP-1".to_owned());
        v.at = Some(Target::Power);
        let rows = v.rows(&lid, DECIDING_WIDTH, moment, &texts());
        v.settle(&now, &rows);
        assert_eq!(rows[v.cursor].target, Target::Power);
        assert!(
            rows[v.cursor].disabled,
            "a key that can write nothing is live"
        );
        let said = texts().t(ronoctua_i18n::key::DISPLAY_DARK_NOT_OURS);
        assert!(
            rows[v.cursor].fields.iter().any(|f| f.text == said),
            "the row refuses without saying why",
        );
        assert_eq!(v.key(Press::More, &lid, moment, &texts()), Errand::Nothing);
    }

    #[test]
    fn keeping_writes_the_one_knob_that_was_asked_about() {
        // IT WROTE THREE OF THEM, whatever the question was. A person who
        // nudged the scale on a laptop that had never been configured came out
        // of it with the mode AND the position pinned as well — opted out of
        // niri's own arrangement for that monitor, silently.
        let now = snapshot(vec![screen("eDP-1", true)]);
        let moment = Instant::now();
        let mut v = on_the_row(&now, &Target::Scale);
        v.key(Press::More, &now, moment, &texts());
        assert!(v.asking.is_some());
        // The cursor is on "put it back"; one row down is "keep it".
        v.key(Press::Down, &now, moment, &texts());
        let out = v.key(Press::Enter, &now, moment, &texts());
        let w = kept(&out);
        assert!(
            w.scale.is_some(),
            "the scale it asked about was not written"
        );
        assert!(w.mode.is_none(), "it pinned a mode nobody asked about");
        assert!(
            w.position.is_none(),
            "it pinned a position nobody asked about"
        );
    }

    #[test]
    fn keeping_a_turn_writes_the_turn() {
        // IT WROTE NOTHING AT ALL. `Keep` carried `wanted.transform` straight
        // from the clone, so `asks_for` then asked niri for `Normal` and the
        // picture snapped back the moment somebody said "keep it". No gesture
        // in the view could write a rotation.
        let now = snapshot(vec![screen("eDP-1", true)]);
        let moment = Instant::now();
        let mut v = on_the_row(&now, &Target::Turn);
        let out = v.key(Press::More, &now, moment, &texts());
        let Ask::Transform(asked) = asked_for(&out) else {
            panic!("the turn did not ask for a turn");
        };
        assert_ne!(asked, Transform::Normal);
        v.key(Press::Down, &now, moment, &texts());
        let out = v.key(Press::Enter, &now, moment, &texts());
        assert_eq!(kept(&out).transform, asked, "the turn was not kept");
    }

    #[test]
    fn keeping_the_compositors_choice_keeps_the_choice() {
        // THE LADDER HANDS THE DECISION BACK at the bottom (`Scale(None)`),
        // and `Keep` used to write down the number niri picked instead — so
        // the change confirmed was never the change previewed.
        let mut screens = vec![screen("eDP-1", true)];
        screens[0].wanted.scale = Some(LADDER[0]);
        let now = snapshot(screens);
        let moment = Instant::now();
        let mut v = on_the_row(&now, &Target::Scale);
        let out = v.key(Press::Less, &now, moment, &texts());
        assert_eq!(asked_for(&out), Ask::Scale(None), "it did not hand it back");
        v.key(Press::Down, &now, moment, &texts());
        let out = v.key(Press::Enter, &now, moment, &texts());
        assert_eq!(kept(&out).scale, None, "it wrote a number down instead");
    }

    #[test]
    fn the_scale_arrow_steps_from_the_setting_or_the_glass() {
        // THE ROW SHOWED THE LIVE SCALE AND THE ARROW STEPPED FROM THE WANTED
        // ONE. On a panel nobody has configured those are 2,0 and `None`: the
        // row read 200% and "more" answered 100%, halving the picture.
        let mut screens = vec![screen("eDP-1", true)];
        screens[0].wanted.scale = None;
        if let Some(l) = screens[0].logical.as_mut() {
            l.scale = 2.0;
        }
        let now = snapshot(screens);
        let moment = Instant::now();
        let mut v = on_the_row(&now, &Target::Scale);
        let out = v.key(Press::More, &now, moment, &texts());
        let Ask::Scale(Some(next)) = asked_for(&out) else {
            panic!("more did not ask for a scale");
        };
        assert!(next > 2.0, "`more` answered {next}, which is less than 2.0");
    }

    #[test]
    fn a_scale_between_two_rungs_steps_the_way_it_was_asked_to() {
        // `position(…).unwrap_or(0)` put every value the ladder does not carry
        // at the BOTTOM, and niri's own choice for a 13-inch 4K panel is one
        // of those. "More" from 1,6 answered 1,25.
        assert_eq!(next_scale(Some(1.6), true), Some(Some(1.75)));
        assert_eq!(next_scale(Some(1.6), false), Some(Some(1.5)));
        assert_eq!(next_scale(Some(0.5), true), Some(Some(1.0)));
        assert_eq!(next_scale(Some(9.0), false), Some(Some(3.0)));
        // AND BELOW THE BOTTOM RUNG, DOWN IS THE COMPOSITOR'S CHOICE —
        // `Some(None)`, a step to take whose value is "you decide".
        assert_eq!(next_scale(Some(0.5), false), Some(None));
        // A VALUE ON A RUNG DOES NOT FIND ITSELF.
        assert_eq!(next_scale(Some(2.0), true), Some(Some(2.5)));
        assert_eq!(next_scale(Some(2.0), false), Some(Some(1.75)));
        // AND THE TOP HAS NOWHERE TO GO — the OUTER `None`, which is what
        // keeps the arrow from raising a question about a change that would
        // not happen. It used to answer the current scale here.
        assert_eq!(next_scale(Some(3.0), true), None);
        // Same at the other end, one rung below the bottom.
        assert_eq!(next_scale(None, false), None);
    }

    #[test]
    fn the_turn_row_says_the_setting_and_names_the_picture() {
        // The row showed the PICTURE's turn while the arrow stepped the
        // SETTING's, so on a panel with an orientation of its own the left
        // arrow did nothing and the right one turned on top of the panel.
        let t = texts();
        let plain = turn_said(Transform::Normal, Transform::Normal, &t);
        assert_eq!(plain, t.t(ronoctua_i18n::key::DISPLAY_TURN_NORMAL));
        let both = turn_said(Transform::Normal, Transform::Flipped180, &t);
        assert_ne!(both, plain, "it said nothing about a picture that differs");
        assert!(both.contains(&t.t(ronoctua_i18n::key::DISPLAY_TURN_FLIPPED_180)));
    }

    #[test]
    fn a_sentence_is_never_a_row() {
        // PROSE THE CURSOR CAN STAND ON LOOKS CHOSEN, and `RET` on it copies
        // it to the clipboard. The question's two sentences were kept off the
        // cursor for exactly this and the rule was written beside them; the
        // six other sentences in the view went through the half that did not
        // skip. The first frame of a machine whose fragment does not parse lit
        // its own error message in the focus colour.
        //
        // EVERY STATE THAT DRAWS ONE, and each is a state the view exists for:
        // no reading thread, no compositor, nothing plugged in, a fragment
        // that does not parse, a sheet whose screen went away, an arrangement
        // with one screen, and a machine whose panel has no readable light.
        let t = texts();
        let moment = Instant::now();
        let standable = |v: &View, now: &Snapshot, key: &str| -> bool {
            let want = t.t(key);
            v.rows(now, DECIDING_WIDTH, moment, &t)
                .iter()
                .any(|r| !r.heading && r.fields.iter().any(|f| f.text == want))
        };
        // THE OTHER HALF OF EVERY ASSERTION BELOW: the sentence has to be on
        // the screen at all, or "the cursor cannot stand on it" is true of
        // something that is not drawn.
        let standable_at_all = |v: &View, now: &Snapshot, key: &str| -> bool {
            let want = t.t(key);
            v.rows(now, DECIDING_WIDTH, moment, &t)
                .iter()
                .any(|r| r.fields.iter().any(|f| f.text == want))
        };

        // A VIEW THAT HAS BEEN READ TO, which every one of these is about:
        // before the first reading the list draws a fourth sentence and the
        // three below are not on the screen at all — so asserting they are not
        // STANDABLE would pass by their being absent, which is vacuity.
        let read_to = || {
            let mut v = View::fresh();
            v.saw_a_reading();
            v
        };
        let mut bare = snapshot(vec![]);
        bare.compositor = false;
        assert!(standable_at_all(
            &read_to(),
            &bare,
            ronoctua_i18n::key::DISPLAY_NO_COMPOSITOR
        ));
        assert!(!standable(
            &read_to(),
            &bare,
            ronoctua_i18n::key::DISPLAY_NO_COMPOSITOR
        ));
        assert!(!standable(
            &read_to(),
            &snapshot(vec![]),
            ronoctua_i18n::key::DISPLAY_NOTHING_HERE
        ));
        // AND THE FOURTH ONE, which is the ordinary first frame of the window.
        assert!(!standable(
            &View::fresh(),
            &bare,
            ronoctua_i18n::key::DISPLAY_READING
        ));

        let mut broken = snapshot(vec![screen("eDP-1", true)]);
        broken.config_failed = true;
        let fresh = View::fresh();
        assert!(!standable(
            &fresh,
            &broken,
            ronoctua_i18n::key::DISPLAY_CONFIG_FAILED
        ));
        // AND THE CURSOR DOES NOT OPEN ON IT. `fresh` starts at row 1, which
        // is where that sentence is drawn.
        let rows = fresh.rows(&broken, DECIDING_WIDTH, moment, &t);
        assert!(
            !rows[fresh.cursor()].fields.iter().any(|f| f
                .text
                .contains(&t.t(ronoctua_i18n::key::DISPLAY_CONFIG_FAILED))),
            "the view opens with its own error message under the cursor",
        );

        let mut lost = View::fresh();
        lost.go_to(Level::Monitor("HDMI-1".to_owned()));
        assert!(!standable(
            &lost,
            &snapshot(vec![screen("eDP-1", true)]),
            ronoctua_i18n::key::DISPLAY_WENT_AWAY
        ));

        let mut arranging = View::fresh();
        arranging.go_to(Level::Arrange);
        assert!(!standable(
            &arranging,
            &snapshot(vec![screen("eDP-1", true)]),
            ronoctua_i18n::key::DISPLAY_ONE_SCREEN_ONLY
        ));

        let mut dim = View::fresh();
        dim.go_to(Level::Monitor("eDP-1".to_owned()));
        assert!(!standable(
            &dim,
            &snapshot(vec![screen("eDP-1", true)]),
            ronoctua_i18n::key::DISPLAY_LIGHT_ON_ITS_BUTTONS
        ));
    }

    #[test]
    fn every_row_of_a_sheet_can_be_walked_to() {
        // THE CURSOR USED TO PING-PONG BETWEEN THE FIRST TWO FACTS. `key`
        // settles before it acts, `settle` looked the cursor up by TARGET, and
        // every fact carries `Target::Nothing` — so the second `Down` found
        // the first fact again and the sheet's own rows, the scale and the
        // turn among them, were unreachable. Six `Down`s on the screen drew
        // two pictures, alternating.
        //
        // This walks the whole sheet the way a person does — through `key`,
        // which is where the settle is — and asks that every row it can stand
        // on is reached exactly once, in order.
        let now = snapshot(vec![screen("eDP-1", true)]);
        let mut v = View::fresh();
        let moment = Instant::now();
        v.key(Press::Enter, &now, moment, &texts());
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        let want: Vec<Target> = rows
            .iter()
            .filter(|r| !r.heading)
            .map(|r| r.target.clone())
            .collect();
        assert!(
            want.contains(&Target::Scale) && want.contains(&Target::Turn),
            "the sheet is meant to have the rows this test walks to: {want:?}",
        );

        let mut walked = vec![rows[v.cursor].target.clone()];
        for _ in 1..want.len() {
            v.key(Press::Down, &now, moment, &texts());
            let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
            walked.push(rows[v.cursor].target.clone());
        }
        assert_eq!(walked, want, "the cursor did not walk the sheet in order");

        // AND IT STOPS AT THE END rather than wrapping: the last `Down` on the
        // last row leaves the cursor where it is, which is what `walk` breaks
        // for.
        v.key(Press::Down, &now, moment, &texts());
        let rows = v.rows(&now, DECIDING_WIDTH, moment, &texts());
        assert_eq!(rows[v.cursor].target, *want.last().unwrap());
    }

    #[test]
    fn a_question_puts_the_way_back_first_and_the_cursor_on_it() {
        // THE RULE THIS VIEW SHARES WITH THE PAIRING QUESTION AND THE DISK
        // FORMATTING: a question that appears by itself answers NO to `RET`.
        // Here it is not politeness — the person may be looking at a picture
        // they cannot read, and the first thing under the cursor has to be
        // the way out.
        let now = snapshot(vec![screen("eDP-1", true)]);
        let mut v = View::fresh();
        v.level = Level::Monitor("eDP-1".to_owned());
        let errand = v.turn(&Target::Scale, true, &now, Instant::now());
        assert!(matches!(errand, Errand::Order(_)), "no preview was applied");
        assert!(v.asking().is_some());

        let rows = v.rows(&now, 80, Instant::now(), &texts());
        let acts: Vec<&Target> = rows
            .iter()
            .filter(|r| !r.heading)
            .map(|r| &r.target)
            .collect();
        let first = acts
            .iter()
            .find(|t| matches!(t, Target::PutItBack | Target::KeepIt))
            .expect("the question offers neither answer");
        assert_eq!(**first, Target::PutItBack, "keeping it comes first");
        assert_eq!(rows[v.cursor].target, Target::PutItBack);
    }

    #[test]
    fn a_question_nobody_answers_puts_itself_back() {
        // The whole reason the question exists: somebody looking at a black
        // rectangle cannot answer, and waiting has to BE an answer.
        let mut v = View::fresh();
        v.level = Level::Monitor("eDP-1".to_owned());
        let began = Instant::now();
        v.asking = Some(Asking {
            at: "eDP-1".to_owned(),
            what: ronoctua_i18n::key::DISPLAY_WHAT_SCALE,
            since: began,
            deciding: Deciding::Preview(Ask::Scale(Some(1.5))),
            stood_at: 3,
            was: None,
        });
        assert_eq!(v.ran_out(began), Errand::Nothing, "it went back at once");
        assert_eq!(
            v.ran_out(began + PATIENCE - Duration::from_secs(1)),
            Errand::Nothing,
            "it went back a second early"
        );
        let out = v.ran_out(began + PATIENCE);
        assert!(
            matches!(&out, Errand::Order(o) if **o == ronoctua_display::Order::Undo),
            "it did not put itself back: {out:?}"
        );
        assert!(v.asking().is_none());
    }

    #[test]
    fn escape_answers_the_question_before_it_leaves_the_level() {
        let now = snapshot(vec![screen("eDP-1", true)]);
        let mut v = View::fresh();
        v.level = Level::Monitor("eDP-1".to_owned());
        v.asking = Some(Asking {
            at: "eDP-1".to_owned(),
            what: ronoctua_i18n::key::DISPLAY_WHAT_TURN,
            since: Instant::now(),
            deciding: Deciding::Preview(Ask::Transform(Transform::R90)),
            stood_at: 3,
            was: None,
        });
        let out = v.key(Press::Escape, &now, Instant::now(), &texts());
        assert!(matches!(&out, Errand::Order(o) if **o == ronoctua_display::Order::Undo));
        // AND IT STAYS ON THE SHEET. `Esc` answered the question; it did not
        // also walk out of the screen one was looking at.
        assert_eq!(v.level, Level::Monitor("eDP-1".to_owned()));
        // The next one leaves the level, and the one after closes.
        assert_eq!(
            v.key(Press::Escape, &now, Instant::now(), &texts()),
            Errand::Nothing
        );
        assert_eq!(v.level, Level::List);
        assert_eq!(
            v.key(Press::Escape, &now, Instant::now(), &texts()),
            Errand::Close
        );
    }

    #[test]
    fn the_last_lit_screen_cannot_be_turned_off() {
        // It leaves a machine with a compositor drawing nowhere, and no way
        // back that does not involve another computer.
        let alone = snapshot(vec![screen("eDP-1", true)]);
        let v = View {
            level: Level::Monitor("eDP-1".to_owned()),
            ..View::fresh()
        };
        let rows = v.rows(&alone, 80, Instant::now(), &texts());
        let power = rows
            .iter()
            .find(|r| r.target == Target::Power)
            .expect("no row for whether the screen is driven");
        assert!(power.disabled, "the only screen could be turned off");

        // With a second one, it can.
        let two = snapshot(vec![screen("eDP-1", true), screen("DP-1", true)]);
        let rows = v.rows(&two, 80, Instant::now(), &texts());
        let power = rows.iter().find(|r| r.target == Target::Power).unwrap();
        assert!(!power.disabled);
    }

    #[test]
    fn the_cursor_follows_what_it_was_on_when_a_monitor_arrives() {
        // A LIST OF SCREENS REORDERS UNDER THE CURSOR. Restoring by index
        // would leave a key aimed at whatever took that row — which is how a
        // gesture meant for one monitor reaches another.
        let one = snapshot(vec![screen("eDP-1", true)]);
        let mut v = View::fresh();
        let rows = v.rows(&one, 80, Instant::now(), &texts());
        let at = rows.iter().position(|r| r.target == Target::Text).unwrap();
        v.clicked(at, &rows);
        assert_eq!(v.cursor, at);

        // A monitor arrives and pushes the row down.
        let two = snapshot(vec![screen("DP-1", true), screen("eDP-1", true)]);
        let rows = v.rows(&two, 80, Instant::now(), &texts());
        v.settle(&two, &rows);
        assert_eq!(
            rows[v.cursor].target,
            Target::Text,
            "the cursor moved to another row when a screen arrived"
        );
    }

    #[test]
    fn the_resolutions_are_distinct_and_the_rates_belong_to_one() {
        // A monitor lists one mode per size AND rate. Walking the modes with
        // an arrow would take four presses to change the resolution once,
        // passing through rates nobody asked about.
        let s = screen("eDP-1", true);
        assert_eq!(sizes_of(&s), vec![(1920, 1200), (1280, 720)]);
        let fast = rates_of(&s, (1920, 1200));
        assert_eq!(fast.len(), 2);
        assert_eq!(fast[0].refresh_rate, 143_856, "the highest is not first");
        assert_eq!(rates_of(&s, (1280, 720)).len(), 1);
    }

    #[test]
    fn the_scale_ladder_hands_the_decision_back_at_the_bottom() {
        // Stepping down from the smallest rung gives the choice to the
        // compositor rather than stopping — which is the one thing a person
        // cannot reach by naming a number.
        assert_eq!(next_scale(None, true), Some(Some(1.0)));
        // NOWHERE BELOW THE COMPOSITOR'S CHOICE: the outer `None`.
        assert_eq!(next_scale(None, false), None);
        // AND THE BOTTOM RUNG HANDS IT BACK: a step, whose value is "you
        // decide".
        assert_eq!(next_scale(Some(1.0), false), Some(None));
        assert_eq!(next_scale(Some(1.0), true), Some(Some(1.25)));
        assert_eq!(
            next_scale(Some(3.0), true),
            None,
            "it offered a step it could not take"
        );
        // And every rung is reachable from the one below it.
        let mut at = next_scale(None, true).unwrap();
        for want in LADDER {
            assert_eq!(at, Some(want));
            at = next_scale(at, true).unwrap_or(at);
        }
    }

    #[test]
    fn a_screen_that_says_nothing_says_which_way_its_settings_follow() {
        // Three different sentences, because the two ways of falling back to
        // the socket mean different things to whoever is reading.
        let mut s = screen("eDP-1", true);
        let t = texts();
        let by_itself = filed_said(&s, &t);
        s.naming = Naming::Connector {
            name: "eDP-1".to_owned(),
            why: ronoctua_niri::Anonymous::NoSerial,
        };
        let no_serial = filed_said(&s, &t);
        s.naming = Naming::Connector {
            name: "eDP-1".to_owned(),
            why: ronoctua_niri::Anonymous::Silent,
        };
        let silent = filed_said(&s, &t);
        assert_ne!(by_itself, no_serial);
        assert_ne!(no_serial, silent);
        assert_ne!(by_itself, silent);
    }

    #[test]
    fn answering_for_one_screen_does_not_drop_another_ones_nudge() {
        // `place_pending` HOLDS ONE SCREEN, and `a` used to clear it whatever
        // it named. Hold `→` on `DP-1`, walk to `eDP-1`, press `a`: `DP-1`'s
        // row snapped back to the position the last reading carried, and the
        // next arrow there stepped from the stale number — one nudge lost.
        // That is the exact defect `place_pending` was added to prevent,
        // reintroduced one screen over.
        let t = texts();
        let moment = Instant::now();
        let two = snapshot(vec![screen("eDP-1", true), screen("DP-1", true)]);
        let mut v = View::fresh();
        v.go_to(Level::Arrange);
        v.place_pending = Some(("DP-1".to_owned(), 1920, 0));

        let rows = v.rows(&two, DECIDING_WIDTH, moment, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Place("eDP-1".to_owned()))
            .expect("the arrangement has no row for eDP-1");
        v.cursor = at;
        v.at = Some(Target::Place("eDP-1".to_owned()));
        let _ = v.key(Press::Automatic, &two, moment, &t);
        assert_eq!(
            v.place_pending.as_ref().map(|(who, _, _)| who.as_str()),
            Some("DP-1"),
            "answering for one screen threw away another screen's nudge",
        );

        // AND IT DOES CLEAR ITS OWN, or the row it just set to `automatic`
        // would go on drawing the place it no longer has.
        let at = rows
            .iter()
            .position(|r| r.target == Target::Place("DP-1".to_owned()))
            .expect("the arrangement has no row for DP-1");
        v.cursor = at;
        v.at = Some(Target::Place("DP-1".to_owned()));
        let _ = v.key(Press::Automatic, &two, moment, &t);
        assert_eq!(v.place_pending, None, "the screen's own nudge outlived `a`");
    }

    #[test]
    fn the_last_lit_screen_is_not_turned_off_by_a_yes() {
        // `only_one` DISABLES THE ROW WHEN THE QUESTION IS RAISED, and a
        // question with a clock stands for fifteen seconds. Inside that window
        // the other screen can stop being lit for a reason of its own — a lid
        // shut, DPMS, a cable half out — and none of those is an unplug, so
        // the `Gone` check does not fire either. Answering "keep it" then
        // wrote `off` for the only screen left: a compositor drawing nowhere,
        // and no way back that does not involve another computer.
        let t = texts();
        let moment = Instant::now();
        let two = snapshot(vec![screen("eDP-1", true), screen("DP-1", true)]);
        let mut v = View::fresh();
        v.go_to(Level::Monitor("eDP-1".to_owned()));

        // The question is raised while both are lit, so the row allows it.
        let rows = v.rows(&two, DECIDING_WIDTH, moment, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Power)
            .expect("the sheet has no power row");
        v.cursor = at;
        v.at = Some(Target::Power);
        let armed = v.key(Press::Less, &two, moment, &t);
        assert!(
            !matches!(armed, Errand::Nothing),
            "turning the screen off was refused while two were lit",
        );

        // AND NOW THE OTHER ONE GOES DARK ON ITS OWN — still present, so this
        // is not the `Gone` path.
        let alone = snapshot(vec![screen("eDP-1", true), screen("DP-1", false)]);
        let rows = v.rows(&alone, DECIDING_WIDTH, moment, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::KeepIt)
            .expect("the question has no keep button");
        v.cursor = at;
        // THE CURSOR IS BORN ON THE SAFE ANSWER, so pressing "keep it" means
        // saying so — without this the press landed on "put it back", which
        // answers `Undo` whether or not the guard exists, and the test passed
        // over a missing guard.
        v.at = Some(Target::KeepIt);
        let answered = v.key(Press::Enter, &alone, moment, &t);
        match answered {
            Errand::Order(o) => assert!(
                matches!(*o, ronoctua_display::Order::Undo),
                "the yes wrote something for the only lit screen: {o:?}",
            ),
            other => panic!("the yes did not put the picture back: {other:?}"),
        }
    }

    #[test]
    fn a_settings_file_that_will_not_parse_says_so_on_the_list() {
        // WITHOUT THE SENTENCE THE PERSON SEES THE LOSS AND NOT THE REASON.
        // When `display.json` will not parse the reading falls back to the
        // default, so every screen draws as though nothing had ever been
        // chosen for it — and the file is intact, and the crate refuses to
        // write over it. It is the same silent failure `config_failed` is
        // drawn for, one file over.
        let t = texts();
        let moment = Instant::now();
        let mut now = snapshot(vec![screen("eDP-1", true)]);
        // WIDE, SO NOTHING ELIDES. Prose is cut to the width it is drawn at,
        // and at `DECIDING_WIDTH` this sentence loses its tail — which is a
        // fact about DRAWING, not about whether the row is there. Asserting
        // against the whole sentence at a narrow width tests the ellipsis.
        let said = |v: &View, s: &Snapshot| {
            v.rows(s, 400, moment, &t)
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let sentence = t.t(ronoctua_i18n::key::DISPLAY_SETTINGS_FAILED);

        // An ordinary machine does not carry the sentence.
        assert!(
            !said(&View::fresh(), &now).contains(&sentence),
            "the list warns about a file that parses",
        );

        now.settings_failed = true;
        assert!(
            said(&View::fresh(), &now).contains(&sentence),
            "the settings look lost and nothing on the list says why",
        );
    }
}
