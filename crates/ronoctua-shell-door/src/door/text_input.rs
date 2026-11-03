//! THE INPUT METHOD — the one file that names `zwp_text_input_v3`, and the
//! door's half of composing text.
//!
//! # Why a door speaks it
//!
//! Japanese, Chinese and Korean are not typed a key a character: an input
//! method (fcitx5, `nix/modules/input-method.nix`) reads the keys, COMPOSES
//! — `nihon` shown as `にほん` while it is still a guess, `日本` once chosen —
//! and hands the window the text it settled on. The window has to show the
//! guess where it will land, and take the text when it lands. A key's
//! character cannot carry either: the composition is not typed, and what is
//! committed was never a key. `zwp_text_input_v3` is the compositor's word
//! for both, a STANDARD Wayland protocol of the kind the virtual keyboard and
//! the cursor shape are — the owner's approved exception to invariant 3
//! (2026-11-02), and `one-text-input` keeps every other file from naming it.
//! A compositor without it costs the composition, never the window: every
//! key still arrives as the keyboard gave it.
//!
//! # What a window says, and what it is told
//!
//! A window that types into something answers [`crate::door::Content::text_input`]
//! with a [`Field`] — what it is for, and where its cursor is on the surface,
//! so the input method's list of guesses opens beside it. `None`, the
//! default, is every window that types into nothing an input method could
//! compose for (a list, a question read key by key): it is never enabled,
//! and the input method never takes its keys. The answer is followed once a
//! turn while the keyboard is in — enabled when one appears, the cursor told
//! again when it moves, disabled when it goes.
//!
//! What the input method says comes in BATCHES: the composition, the text
//! committed and a deletion, each held until `done`, which applies them in
//! the protocol's order — the old composition taken away, the commit typed,
//! the new composition shown. The window hears exactly that, in that order
//! ([`crate::door::Content::preedit`], [`crate::door::Content::committed`]).
//!
//! # A seat each
//!
//! The door hears every seat, and so does this: a text input per seat with a
//! keyboard, each following its own seat's focus. The protocol's focus is
//! the KEYBOARD's (`enter` and `leave` of the text input itself, which the
//! compositor sends as the keyboard moves): asked before that `enter`, an
//! `enable` is one the compositor must ignore, so it is sent from it.
//!
//! # No Wayland in the arithmetic
//!
//! [`Typing`] is one seat's state with no proxy in it — what was heard, what
//! was shown, what was told — so a test plays a batch through it; the door
//! carries out its [`Turn`]s on the proxies.

use super::{Content, Door, Ui};
use smithay_client_toolkit::dispatch2::Dispatch2;
use smithay_client_toolkit::reexports::client::{
    Connection, Dispatch, QueueHandle, globals::GlobalList, protocol::wl_seat::WlSeat,
};
use smithay_client_toolkit::reexports::protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::{self, ZwpTextInputManagerV3},
    zwp_text_input_v3::{self, ContentHint, ContentPurpose, ZwpTextInputV3},
};

/// What a window's field is for — OUR word for the protocol's purpose, so no
/// other file names it (`one-text-input`). Only the purposes a window of
/// ours has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Text of any kind.
    Normal,
    /// A terminal: what is typed goes to the program running in it.
    Terminal,
}

impl Purpose {
    /// The protocol's word for it.
    const fn content_purpose(self) -> ContentPurpose {
        match self {
            Self::Normal => ContentPurpose::Normal,
            Self::Terminal => ContentPurpose::Terminal,
        }
    }
}

/// A rectangle of the surface, in LOGICAL pixels from its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LogicalRect {
    /// Across.
    pub x: i32,
    /// Down.
    pub y: i32,
    /// Its width.
    pub width: i32,
    /// Its height.
    pub height: i32,
}

/// WHERE A WINDOW TAKES TEXT FROM AN INPUT METHOD: what the field is for,
/// and the cursor's rectangle — where the input method opens its guesses,
/// beside the text and not over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// What the field is for.
    pub purpose: Purpose,
    /// The cursor, on the surface.
    pub cursor: LogicalRect,
}

/// WHAT AN INPUT METHOD IS COMPOSING — not typed yet: drawn where it will
/// land, underlined, and never sent anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preedit {
    /// The composition.
    pub text: String,
    /// The cursor inside it, in BYTES of `text` — a point when the range is
    /// empty, the part being converted when it is not — or `None` when the
    /// input method hides it, or named a place that is no place in the text.
    pub cursor: Option<std::ops::Range<usize>>,
}

impl Preedit {
    /// The composition the protocol said: `None` for no text, and a cursor
    /// at bytes `begin..end` — both `-1` hide it, and an index past the
    /// text or inside a character is refused as hidden, with a line in the
    /// log, never drawn somewhere plausible.
    fn of(text: Option<String>, begin: i32, end: i32) -> Option<Self> {
        let text = text.filter(|t| !t.is_empty())?;
        let cursor = match (usize::try_from(begin), usize::try_from(end)) {
            (Ok(b), Ok(e)) => {
                let (b, e) = (b.min(e), b.max(e));
                if e <= text.len() && text.is_char_boundary(b) && text.is_char_boundary(e) {
                    Some(b..e)
                } else {
                    tracing::debug!(
                        begin,
                        end,
                        "the input method's cursor is no place in its text; hidden"
                    );
                    None
                }
            }
            _ => None,
        };
        Some(Self { text, cursor })
    }
}

/// What the input method said, in OUR words — what [`Typing`] holds until
/// `done`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Heard {
    /// The composition, and its cursor's bytes.
    Preedit(Option<String>, i32, i32),
    /// The text committed.
    Commit(Option<String>),
}

/// What the compositor is told, each committed at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Told {
    /// The field is enabled: its purpose and its cursor.
    Enable(Field),
    /// Its cursor moved.
    Moved(LogicalRect),
    /// There is no field any more.
    Disable,
}

/// What the window is shown, in this order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Shown {
    /// The composition, or none.
    Preedit(Option<Preedit>),
    /// Text committed — typed.
    Committed(String),
}

/// One step of a seat's text input: what the compositor is told and what
/// the window is shown.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Turn {
    /// What the compositor is told, if anything.
    pub told: Option<Told>,
    /// What the window is shown, in order.
    pub shown: Vec<Shown>,
}

/// The batch heard since the last `done` — the protocol's pending state,
/// reset to nothing at every `done`.
#[derive(Debug, Default)]
struct Batch {
    preedit: Option<Preedit>,
    commit: Option<String>,
}

/// ONE SEAT'S TEXT INPUT, with no Wayland in it — see the head of the module.
#[derive(Debug, Default)]
pub(crate) struct Typing {
    /// The seat's text-input focus is on the door's surface.
    entered: bool,
    /// The field the compositor was last told of — `None` while disabled,
    /// and after every `enter` and `leave`, which invalidate it.
    told: Option<Field>,
    /// What was heard since the last `done`.
    batch: Batch,
    /// The composition the window was last shown.
    shown: Option<Preedit>,
}

impl Typing {
    /// The seat's text-input focus came to the door's surface: the window's
    /// field, if it has one, is enabled — state from before is gone (the
    /// protocol invalidates it on every `enter`).
    pub(crate) fn entered(&mut self, wanted: Option<Field>) -> Turn {
        self.entered = true;
        self.told = None;
        self.follow(wanted)
    }

    /// The focus went: the composition is taken away, as the protocol asks,
    /// and a batch half heard is forgotten — nothing is heard again until
    /// the next `enter`. NOTHING IS TOLD: after a `leave` the compositor
    /// ignores this text input until then, and the enter enables afresh.
    pub(crate) fn left(&mut self) -> Turn {
        self.entered = false;
        self.told = None;
        self.batch = Batch::default();
        Turn {
            told: None,
            shown: self.take_the_preedit(),
        }
    }

    /// The window's answer now: enabled when a field appeared, told again
    /// when its cursor moved (re-enabled when it is a field of another
    /// purpose), disabled when it went — and nothing while the focus is
    /// elsewhere, or nothing changed.
    pub(crate) fn follow(&mut self, wanted: Option<Field>) -> Turn {
        if !self.entered || wanted == self.told {
            return Turn::default();
        }
        let before = std::mem::replace(&mut self.told, wanted);
        match (before, wanted) {
            (Some(was), Some(now)) if was.purpose == now.purpose => Turn {
                told: Some(Told::Moved(now.cursor)),
                shown: Vec::new(),
            },
            (_, Some(now)) => Turn {
                told: Some(Told::Enable(now)),
                shown: Vec::new(),
            },
            // NO FIELD, NO COMPOSITION: the input method drops what it was
            // composing as it is disabled, and so does the window.
            (_, None) => Turn {
                told: Some(Told::Disable),
                shown: self.take_the_preedit(),
            },
        }
    }

    /// Part of a batch, held until `done` — and nothing while the focus is
    /// elsewhere: what an input method says then is for no window.
    pub(crate) fn heard(&mut self, heard: Heard) {
        if !self.entered {
            return;
        }
        match heard {
            Heard::Preedit(text, begin, end) => self.batch.preedit = Preedit::of(text, begin, end),
            Heard::Commit(text) => self.batch.commit = text.filter(|t| !t.is_empty()),
        }
    }

    /// `done`: the batch applied, in the protocol's order — the composition
    /// shown taken away, the commit typed, the new composition shown — and
    /// reset. A composition that did not change is not shown again.
    ///
    /// THE SERIAL IS NOT ASKED. It says which of this side's commits the
    /// batch was worked out against, which matters to a window that sends
    /// the text around its cursor: the input method's deletions are counted
    /// in it. A window of ours sends none (the door never says
    /// `set_surrounding_text`), so nothing in a batch can be stale against
    /// it — and a stale batch is still applied, the protocol says.
    pub(crate) fn done(&mut self) -> Turn {
        let Batch { preedit, commit } = std::mem::take(&mut self.batch);
        let mut shown = Vec::new();
        match commit {
            Some(text) => {
                shown.extend(self.take_the_preedit());
                shown.push(Shown::Committed(text));
                if preedit.is_some() {
                    shown.push(Shown::Preedit(preedit.clone()));
                }
            }
            None if preedit != self.shown => shown.push(Shown::Preedit(preedit.clone())),
            None => {}
        }
        self.shown = preedit;
        Turn { told: None, shown }
    }

    /// The composition shown, taken away — nothing when none is.
    fn take_the_preedit(&mut self) -> Vec<Shown> {
        self.shown
            .take()
            .map(|_| Shown::Preedit(None))
            .into_iter()
            .collect()
    }
}

/// The user data of the manager, which says nothing.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Manager;

impl<S> Dispatch2<ZwpTextInputManagerV3, S> for Manager {
    fn event(
        &self,
        _: &mut S,
        _: &ZwpTextInputManagerV3,
        _: zwp_text_input_manager_v3::Event,
        _: &Connection,
        _: &QueueHandle<S>,
    ) {
    }
}

/// The user data of a seat's text input: the door finds the seat by the
/// proxy.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Seated;

/// One seat's text input, and its state.
#[derive(Debug)]
struct Input {
    seat: WlSeat,
    proxy: ZwpTextInputV3,
    typing: Typing,
}

/// THE MANAGER, when the compositor offers one, and a text input per seat
/// with a keyboard.
#[derive(Debug, Default)]
pub(crate) struct Inputs {
    manager: Option<ZwpTextInputManagerV3>,
    inputs: Vec<Input>,
}

impl Inputs {
    /// Binds the manager — at version 1, the one this speaks (2 adds events
    /// a window of ours has no use for). Its absence is not an error: every
    /// key still arrives as the keyboard gave it.
    pub(crate) fn bind<S>(globals: &GlobalList, qh: &QueueHandle<S>) -> Self
    where
        S: Dispatch<ZwpTextInputManagerV3, Manager> + 'static,
    {
        let manager = globals
            .bind::<ZwpTextInputManagerV3, _, _>(qh, 1..=1, Manager)
            .map_err(
                |e| tracing::debug!(error = %e, "no text input; no input method composes here"),
            )
            .ok();
        Self {
            manager,
            inputs: Vec::new(),
        }
    }

    /// A seat's keyboard arrived: its text input, made.
    pub(crate) fn took<S>(&mut self, seat: &WlSeat, qh: &QueueHandle<S>)
    where
        S: Dispatch<ZwpTextInputV3, Seated> + 'static,
    {
        if self.inputs.iter().any(|i| &i.seat == seat) {
            return;
        }
        if let Some(m) = self.manager.as_ref() {
            self.inputs.push(Input {
                seat: seat.clone(),
                proxy: m.get_text_input(seat, qh, Seated),
                typing: Typing::default(),
            });
        }
    }

    /// A seat's keyboard went: its text input with it — and the composition
    /// it was showing, which nobody can commit now.
    pub(crate) fn lost(&mut self, seat: &WlSeat) -> Vec<Shown> {
        let Some(at) = self.inputs.iter().position(|i| &i.seat == seat) else {
            return Vec::new();
        };
        let mut gone = self.inputs.remove(at);
        gone.proxy.destroy();
        gone.typing.left().shown
    }

    /// Whether any seat's focus is on the door — the only time a window's
    /// field is asked for.
    fn any_entered(&self) -> bool {
        self.inputs.iter().any(|i| i.typing.entered)
    }
}

/// Says `told` to the compositor, and commits it.
fn tell(proxy: &ZwpTextInputV3, told: Told) {
    let cursor = |a: LogicalRect| proxy.set_cursor_rectangle(a.x, a.y, a.width, a.height);
    match told {
        Told::Enable(field) => {
            proxy.enable();
            proxy.set_content_type(ContentHint::None, field.purpose.content_purpose());
            cursor(field.cursor);
        }
        Told::Moved(rect) => cursor(rect),
        Told::Disable => proxy.disable(),
    }
    proxy.commit();
}

/// The window's field now: none while the door's own line asks a question
/// (it reads key by key, and an input method would take the keys it
/// answers with), else the window's answer.
pub(crate) fn wanted_of<C: Content>(content: &C, ui: &Ui) -> Option<Field> {
    if ui.asking.is_some() {
        None
    } else {
        content.text_input(ui)
    }
}

/// What the window is shown, told to it in order.
pub(crate) fn show_into<C: Content>(content: &mut C, ui: &mut Ui, shown: Vec<Shown>) {
    for s in shown {
        match s {
            Shown::Preedit(preedit) => content.preedit(ui, preedit),
            Shown::Committed(text) => content.committed(ui, &text),
        }
    }
}

impl<C: Content> Door<C> {
    /// A seat's keyboard arrived: its text input.
    pub(crate) fn text_input_took(&mut self, seat: &WlSeat) {
        let qh = self.qh.clone();
        self.inputs.took(seat, &qh);
    }

    /// A seat's keyboard went, and its text input with it.
    pub(crate) fn text_input_lost(&mut self, seat: &WlSeat) {
        let shown = self.inputs.lost(seat);
        self.shown(shown);
    }

    /// THE WINDOW'S FIELD, followed: asked once a turn while a seat's focus
    /// is on the door (`Door::settle`, after the frame — a cursor the frame
    /// moved is told now, beside the text it moved with).
    pub(crate) fn follow_the_field(&mut self) {
        if !self.inputs.any_entered() {
            return;
        }
        let wanted = wanted_of(&self.content, &self.ui);
        for at in 0..self.inputs.inputs.len() {
            let turn = self.inputs.inputs[at].typing.follow(wanted);
            self.carry_out(at, turn);
        }
    }

    /// What one seat's text input said.
    fn text_input_said(&mut self, proxy: &ZwpTextInputV3, event: zwp_text_input_v3::Event) {
        use zwp_text_input_v3::Event;
        let Some(at) = self.inputs.inputs.iter().position(|i| &i.proxy == proxy) else {
            return;
        };
        let turn = match event {
            // ONLY THE DOOR'S SURFACE: the plain screens of a lock are this
            // door's too, and nobody types on them.
            Event::Enter { surface } if surface == self.ui.surface => {
                let wanted = wanted_of(&self.content, &self.ui);
                self.inputs.inputs[at].typing.entered(wanted)
            }
            Event::Enter { .. } => return,
            Event::Leave { .. } => self.inputs.inputs[at].typing.left(),
            Event::PreeditString {
                text,
                cursor_begin,
                cursor_end,
            } => {
                let heard = Heard::Preedit(text, cursor_begin, cursor_end);
                self.inputs.inputs[at].typing.heard(heard);
                return;
            }
            Event::CommitString { text } => {
                self.inputs.inputs[at].typing.heard(Heard::Commit(text));
                return;
            }
            // NOTHING OF A WINDOW'S IS DELETED. The input method can only
            // count what it was told of the text around the cursor, and the
            // door tells it none; in a terminal it would be backspaces typed
            // into a program for keys the person never pressed.
            Event::DeleteSurroundingText { .. } => return,
            Event::Done { .. } => self.inputs.inputs[at].typing.done(),
            // Version 2's events, which a manager bound at 1 never sends.
            _ => return,
        };
        self.carry_out(at, turn);
    }

    /// Seat `at`'s turn carried out: the compositor told, the window shown.
    fn carry_out(&mut self, at: usize, turn: Turn) {
        if let (Some(told), Some(input)) = (turn.told, self.inputs.inputs.get(at)) {
            tell(&input.proxy, told);
        }
        self.shown(turn.shown);
    }

    /// The window shown what the input method said — and the look under
    /// every pointer asked again: text committed is text typed, and a window
    /// that hides the pointer as somebody types hides it now.
    fn shown(&mut self, shown: Vec<Shown>) {
        if shown.is_empty() {
            return;
        }
        show_into(&mut self.content, &mut self.ui, shown);
        self.look_again();
    }
}

impl<C: Content> Dispatch2<ZwpTextInputV3, Door<C>> for Seated {
    fn event(
        &self,
        door: &mut Door<C>,
        proxy: &ZwpTextInputV3,
        event: zwp_text_input_v3::Event,
        _: &Connection,
        _: &QueueHandle<Door<C>>,
    ) {
        door.text_input_said(proxy, event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smithay_client_toolkit::seat::keyboard::{KeyEvent, Modifiers};

    fn field(x: i32) -> Field {
        Field {
            purpose: Purpose::Terminal,
            cursor: LogicalRect {
                x,
                y: 20,
                width: 9,
                height: 18,
            },
        }
    }

    fn preedit(text: &str, cursor: Option<std::ops::Range<usize>>) -> Preedit {
        Preedit {
            text: text.to_owned(),
            cursor,
        }
    }

    /// What a window was shown, in the order it was shown it.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Seen {
        Preedit(Option<String>),
        Committed(String),
    }

    /// A window that keeps what it is shown — and types into a field only
    /// when it is told it has one.
    struct Recorder {
        field: Option<Field>,
        seen: Vec<Seen>,
    }

    impl Content for Recorder {
        type Hand = ();
        fn app_id(&self) -> &'static str {
            "test"
        }
        fn title(&self, _: &ronoctua_i18n::Texts) -> String {
            String::new()
        }
        fn typed(&mut self, _: &mut Ui, _: &KeyEvent, _: Modifiers) {}
        fn text_input(&self, _: &Ui) -> Option<Field> {
            self.field
        }
        fn preedit(&mut self, _: &mut Ui, preedit: Option<Preedit>) {
            self.seen.push(Seen::Preedit(preedit.map(|p| p.text)));
        }
        fn committed(&mut self, _: &mut Ui, text: &str) {
            self.seen.push(Seen::Committed(text.to_owned()));
        }
    }

    /// A window of the door's defaults: it says nothing of a field.
    struct Plain;

    impl Content for Plain {
        type Hand = ();
        fn app_id(&self) -> &'static str {
            "test"
        }
        fn title(&self, _: &ronoctua_i18n::Texts) -> String {
            String::new()
        }
        fn typed(&mut self, _: &mut Ui, _: &KeyEvent, _: Modifiers) {}
    }

    fn ui() -> Ui {
        Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"))
    }

    #[test]
    fn a_batch_shows_its_composition_and_then_its_commit_in_the_protocols_order() {
        // FAILS IF a composition is shown before its `done`, the commit of a
        // batch reaches the window before the composition it replaces is
        // taken away (a field that edits in place would type the commit
        // INTO the guess), the new composition comes before the commit, or
        // a batch with no composition leaves the old one drawn — the
        // protocol resets it at every `done`.
        let mut t = Typing::default();
        let _ = t.entered(Some(field(0)));
        let mut w = Recorder {
            field: Some(field(0)),
            seen: Vec::new(),
        };
        let mut ui = ui();
        t.heard(Heard::Preedit(Some("にほ".to_owned()), 6, 6));
        t.heard(Heard::Preedit(Some("にほん".to_owned()), 9, 9));
        assert!(w.seen.is_empty(), "shown before done");
        show_into(&mut w, &mut ui, t.done().shown);
        t.heard(Heard::Commit(Some("日本".to_owned())));
        t.heard(Heard::Preedit(Some("ご".to_owned()), 3, 3));
        show_into(&mut w, &mut ui, t.done().shown);
        // A DONE WITH NOTHING IN IT takes the composition away.
        show_into(&mut w, &mut ui, t.done().shown);
        // AND ONE WITH ONLY A COMMIT, nothing drawn before it, types it.
        t.heard(Heard::Commit(Some("語".to_owned())));
        show_into(&mut w, &mut ui, t.done().shown);
        assert_eq!(
            w.seen,
            vec![
                Seen::Preedit(Some("にほん".to_owned())),
                Seen::Preedit(None),
                Seen::Committed("日本".to_owned()),
                Seen::Preedit(Some("ご".to_owned())),
                Seen::Preedit(None),
                Seen::Committed("語".to_owned()),
            ]
        );
    }

    #[test]
    fn a_window_that_types_into_nothing_is_never_enabled() {
        // FAILS IF a window that says nothing of a field — every window but
        // the ones that type — is enabled on the keyboard's enter or later:
        // the input method would take every key of a list, a question, the
        // rescue console, and a hung input method would leave them deaf.
        let ui = ui();
        assert_eq!(wanted_of(&Plain, &ui), None);
        let mut t = Typing::default();
        assert_eq!(t.entered(wanted_of(&Plain, &ui)), Turn::default());
        assert_eq!(t.follow(wanted_of(&Plain, &ui)), Turn::default());
        // NOR WHILE THE DOOR'S LINE ASKS, whatever the window says: the
        // question reads its answer key by key.
        let mut ui = ui;
        ui.ask(crate::line::Line::new("q", false, None));
        let typing = Recorder {
            field: Some(field(0)),
            seen: Vec::new(),
        };
        assert_eq!(wanted_of(&typing, &ui), None);
    }

    #[test]
    fn the_field_is_enabled_on_enter_told_as_it_moves_and_disabled_when_it_goes() {
        // FAILS IF a window's field is not enabled at the focus's enter, its
        // cursor is told again when it did not move (a commit a turn) or not
        // when it did (the guesses open where the cursor was), going away is
        // not a disable, the composition outlives its field, or anything is
        // told while the focus is elsewhere.
        let mut t = Typing::default();
        assert_eq!(t.follow(Some(field(0))), Turn::default(), "before enter");
        assert_eq!(t.entered(Some(field(0))).told, Some(Told::Enable(field(0))));
        assert_eq!(t.follow(Some(field(0))), Turn::default());
        assert_eq!(
            t.follow(Some(field(9))).told,
            Some(Told::Moved(field(9).cursor))
        );
        let other = Field {
            purpose: Purpose::Normal,
            ..field(9)
        };
        assert_eq!(t.follow(Some(other)).told, Some(Told::Enable(other)));
        t.heard(Heard::Preedit(Some("ㅎ".to_owned()), 3, 3));
        let _ = t.done();
        assert_eq!(
            t.follow(None),
            Turn {
                told: Some(Told::Disable),
                shown: vec![Shown::Preedit(None)],
            }
        );
        assert_eq!(t.follow(None), Turn::default());
    }

    #[test]
    fn leaving_takes_the_composition_away_and_the_next_enter_enables_afresh() {
        // FAILS IF the composition stays drawn after the focus left, a batch
        // half heard before the leave is applied after it — at a `done`
        // while the focus is elsewhere, or after the next enter — what an
        // input method says while the focus is elsewhere is applied, or the
        // next enter does not enable again: the protocol forgot every state
        // at the leave, and a field still taken for enabled would never be.
        let mut t = Typing::default();
        let _ = t.entered(Some(field(0)));
        t.heard(Heard::Preedit(Some("한".to_owned()), 3, 3));
        let _ = t.done();
        t.heard(Heard::Commit(Some("half".to_owned())));
        assert_eq!(
            t.left(),
            Turn {
                told: None,
                shown: vec![Shown::Preedit(None)],
            }
        );
        assert_eq!(t.follow(Some(field(0))), Turn::default(), "told while away");
        t.heard(Heard::Preedit(Some("away".to_owned()), 0, 0));
        assert_eq!(t.done(), Turn::default(), "applied while away");
        assert_eq!(t.entered(Some(field(0))).told, Some(Told::Enable(field(0))));
        assert_eq!(t.done(), Turn::default(), "the batch from before came back");
    }

    #[test]
    fn a_preedit_cursor_that_is_no_place_in_its_text_is_hidden() {
        // FAILS IF the cursor the input method hides (-1, -1) is drawn, one
        // past the text or inside a character is drawn somewhere plausible
        // instead of refused, a range given backwards is not the range, or
        // an empty composition is shown as one.
        assert_eq!(
            Preedit::of(Some("abc".to_owned()), 1, 3),
            Some(preedit("abc", Some(1..3)))
        );
        assert_eq!(
            Preedit::of(Some("abc".to_owned()), 3, 1),
            Some(preedit("abc", Some(1..3)))
        );
        assert_eq!(
            Preedit::of(Some("abc".to_owned()), -1, -1),
            Some(preedit("abc", None))
        );
        assert_eq!(
            Preedit::of(Some("abc".to_owned()), 2, 9),
            Some(preedit("abc", None))
        );
        assert_eq!(
            Preedit::of(Some("日".to_owned()), 1, 1),
            Some(preedit("日", None)),
            "inside a character"
        );
        assert_eq!(Preedit::of(Some(String::new()), 0, 0), None);
        assert_eq!(Preedit::of(None, 0, 0), None);
    }
}
