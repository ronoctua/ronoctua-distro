//! THE LOCK SCREEN — `ronoctua-shell lock`.
//!
//! # What it shows: the footer's two lines, over an empty desk
//!
//! This system has no screen of its own besides the content and the footer's
//! two lines, and the lock keeps to that: every screen is the chrome's colour,
//! and at the bottom of the one that had the focus the two lines a person
//! already reads every day — the modeline saying the session is locked, and
//! whose, with the fingerprint reader's state when there is one, and the
//! minibuffer asking the password, as every question of the system is asked
//! (`line::Line`, its `ESC` and `RET` clicks). What the lock has to say —
//! checking, a wrong password — is a line above them, and so is the sentence
//! about the shortcut under the pointer.
//!
//! # The password, and the finger beside it
//!
//! The password goes to PAM under the fixed service `ronoctua-unlock`
//! (`ronoctua_pam::verify`), on a thread: PAM waits three seconds after a
//! wrong one. Where there is a reader and a finger enrolled on it for this
//! account (`ronoctua_security::fprint`, asked as the person), a second
//! thread waits for a touch under `ronoctua-unlock-finger` at the same time —
//! whichever answers first opens the session, and the other is left to the
//! process's end. Where there is none, nothing about a finger is said at all.
//! The reader stops waiting after its own while, and a finger it did not take
//! is offered again when somebody is there: a key, a click, or `C-f`.
//!
//! # A lock that cannot be opened is never taken
//!
//! Before the compositor is asked, the lock checks that this machine declares
//! the password's stack (`ronoctua_pam::declared`) and that it knows the
//! account's name. Without either it does not lock, and ends with
//! [`REFUSED`] — the status its unit never starts again. A session locked with
//! nothing able to open it would be a machine to switch off at the wall.
//!
//! # The secret
//!
//! What is typed lives in the line until `RET`, then in a `Secret` handed to
//! the thread, and the line is wiped (`zeroize`) on every answer, every `ESC`
//! and when the door goes. It is never logged — the line's `Debug` masks it —
//! never an argument and never on a socket.

use crate::door::{self as base, Canvas, Content, Hand, LEFT_BUTTON, Shell, Ui, Wiring};
use crate::line::Step;
use ronoctua_consent::keys::Secret;
use ronoctua_render::Size;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::reexports::calloop::channel;
use smithay_client_toolkit::seat::{
    keyboard::{KeyEvent, Modifiers},
    pointer::{PointerEvent, PointerEventKind},
};
use std::cell::Cell;
use std::rc::Rc;
use wayland_client::protocol::wl_surface;
use zeroize::Zeroize as _;

/// The lock's `app_id` (it is no window; the door asks for one all the same).
pub const APP_ID: &str = "br.ronoctua.lock";

/// The command that locks — `LEADER s q l`, and the idle watcher's.
pub const COMMAND: &str = "lock-screen";

/// The status of a lock that refused to lock: `EX_CONFIG`, the status every
/// program of this system ends with when what it needs is not there. The
/// unit does not start a lock again on it (`RestartPreventExitStatus=`).
pub const REFUSED: i32 = 78;

/// The password's question, `ask-lock-password`.
const QUESTION: &str = "lock-password";

/// The finger's shortcut, `C-f`: the reader waits again.
const FINGER: &str = "lock-finger";

/// The keys the finger's line teaches, and their words.
const OF_THE_FINGER: &[(&str, &str)] = &[("C-f", FINGER)];

/// The line's `ESC` as the lock explains it, `about-lock-clear`: here it
/// clears what was typed, and the question stays.
const CLEAR: &str = "lock-clear";

/// How a lock that did not open the session ended.
#[derive(Debug)]
pub enum Ended {
    /// It did not lock: why, in the log's words.
    Refused(String),
    /// It locked and then failed: its unit starts another, which takes the
    /// session over.
    Failed(anyhow::Error),
}

/// What the door went through, read by [`host_it`] once it is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// The compositor has not said `locked` yet.
    Asked,
    /// It has: the session is locked.
    Locked,
    /// The person opened it.
    Opened,
    /// The compositor ended the lock itself.
    EndedThere,
    /// The compositor never locked: another lock holds the session.
    NeverLocked,
}

/// What a thread of the lock hands back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    /// What PAM said of the password.
    Password(Result<(), ronoctua_pam::Refusal>),
    /// What the reader did.
    Finger(AtTheReader),
}

/// What the reader's thread found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtTheReader {
    /// No reader, or no finger enrolled on it for this account.
    Absent,
    /// A reader and a finger: it is waiting for a touch.
    Offered,
    /// The finger matched.
    Matched,
    /// No match, or the reader stopped waiting.
    Missed,
    /// The reader could not be asked.
    Unavailable,
}

/// The finger, as the screen says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Finger {
    /// Being asked about: nothing is said yet.
    Asking,
    /// None to offer.
    Absent,
    /// Waiting for a touch.
    Waiting,
    /// It did not take the last touch, or stopped waiting.
    Missed,
    /// It did not answer.
    Unavailable,
}

impl Finger {
    /// Whether the reader is to be asked again when somebody is there.
    const fn rests(self) -> bool {
        matches!(self, Self::Missed | Self::Unavailable)
    }

    /// Its words on the modeline, when it has any.
    const fn words(self) -> Option<&'static str> {
        match self {
            Self::Asking | Self::Absent => None,
            Self::Waiting => Some(crate::key::LOCK_FINGER_WAITING),
            Self::Missed => Some(crate::key::LOCK_FINGER_MISSED),
            Self::Unavailable => Some(crate::key::LOCK_FINGER_UNAVAILABLE),
        }
    }
}

/// What the line above the two says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Said {
    /// The password is with PAM.
    Checking,
    /// PAM refused it.
    Refused(ronoctua_pam::Refusal),
}

/// The sentence of a refusal — a closed set (`lock-<id>`).
const fn refusal_key(r: ronoctua_pam::Refusal) -> &'static str {
    match r {
        ronoctua_pam::Refusal::Wrong => crate::key::LOCK_WRONG,
        ronoctua_pam::Refusal::NoService => crate::key::LOCK_NO_SERVICE,
        ronoctua_pam::Refusal::Unavailable => crate::key::LOCK_UNAVAILABLE,
    }
}

/// What the reader's refusal is on the screen. Pure.
const fn after_the_reader(r: ronoctua_pam::Refusal) -> AtTheReader {
    match r {
        // NO MATCH, OR THE READER STOPPED WAITING: the module says which in
        // words of its own, translated, and never read — both are "not taken".
        ronoctua_pam::Refusal::Wrong => AtTheReader::Missed,
        // NO FINGER'S STACK on this machine: there is no finger to offer.
        ronoctua_pam::Refusal::NoService => AtTheReader::Absent,
        ronoctua_pam::Refusal::Unavailable => AtTheReader::Unavailable,
    }
}

/// The account whose password opens the lock: the session's own.
fn account() -> Option<String> {
    ["USER", "LOGNAME"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|n| !n.trim().is_empty())
}

/// The screen with the focus, by connector — where the password is asked.
/// `None` when the compositor cannot say: the first screen, then.
fn focused_screen() -> Option<String> {
    let mut wm = crate::daemon::open_the_compositor()
        .map_err(|e| tracing::debug!(error = %e, "no compositor to ask which screen has the focus"))
        .ok()?;
    wm.focused_output().ok().flatten().map(|o| o.name)
}

/// LOCKS THE SESSION and keeps it locked until the person opens it.
///
/// # Errors
///
/// [`Ended::Refused`] when it did not lock — nothing could open the session
/// again, or the compositor offers no lock; [`Ended::Failed`] when it locked
/// and then could not go on.
pub fn host_it() -> Result<(), Ended> {
    let user = account().ok_or_else(|| {
        Ended::Refused("no account name to check a password for (USER, LOGNAME)".to_owned())
    })?;
    if !ronoctua_pam::declared() {
        return Err(Ended::Refused(format!(
            "{}/{} is not there: nothing could open the session again",
            ronoctua_pam::CONFIG_DIR,
            ronoctua_pam::SERVICE
        )));
    }
    let wanted = focused_screen();
    let outcome = Rc::new(Cell::new(Outcome::Asked));
    let seen = Rc::clone(&outcome);
    let ran = base::run(move |wiring: &Wiring<'_, Lock>| {
        let (send, heard) = channel::channel::<Heard>();
        wiring
            .handle
            .insert_source(heard, |event, (), door: &mut base::Door<Lock>| {
                if let channel::Event::Msg(h) = event {
                    door.content.heard(&mut door.ui, h);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the lock's channel: {e}"))?;
        let mut lock = Lock::new(user, wanted, Some(send), seen);
        lock.ask_the_reader();
        Ok(lock)
    });
    match (ran, outcome.get()) {
        (Ok(()), Outcome::Opened | Outcome::EndedThere | Outcome::NeverLocked) => Ok(()),
        (Ok(()), Outcome::Asked) => Err(Ended::Refused(
            "the door ended before the compositor locked".to_owned(),
        )),
        (Ok(()), Outcome::Locked) => Err(Ended::Failed(anyhow::anyhow!(
            "the door ended with the session still locked"
        ))),
        (Err(e), Outcome::Asked | Outcome::NeverLocked) => Err(Ended::Refused(format!("{e:#}"))),
        (Err(e), _) => Err(Ended::Failed(e)),
    }
}

/// The lock screen, as a door's content.
pub struct Lock {
    /// The account the password is checked for.
    user: String,
    /// The screen the password is asked on, by connector.
    wanted: Option<String>,
    /// What is typed — masked.
    line: crate::line::Line,
    /// What the line above the two says.
    said: Option<Said>,
    /// The reader.
    finger: Finger,
    /// Where the threads answer; `None` for a camera's lock, which asks
    /// nothing of anybody.
    send: Option<channel::Sender<Heard>>,
    /// Whether a question goes to the machine — PAM, the reader. A test's
    /// lock walks the same rules and asks nobody (`daemon::about_to_act`
    /// fails a test that would).
    machine: bool,
    /// What the door went through, for [`host_it`].
    outcome: Rc<Cell<Outcome>>,
    /// Where every control of the two lines was drawn.
    marks: Vec<crate::chrome::Mark>,
    /// The control under the pointer.
    lit: Option<&'static str>,
}

impl Drop for Lock {
    fn drop(&mut self) {
        self.line.text.zeroize();
    }
}

impl Lock {
    fn new(
        user: String,
        wanted: Option<String>,
        send: Option<channel::Sender<Heard>>,
        outcome: Rc<Cell<Outcome>>,
    ) -> Self {
        Self {
            user,
            wanted,
            line: crate::line::Line::new(QUESTION, true, None),
            said: None,
            finger: Finger::Asking,
            machine: send.is_some(),
            send,
            outcome,
            marks: Vec::new(),
            lit: None,
        }
    }

    /// Asks the reader, on a thread: is there one, and a finger on it — and
    /// if so, waits for a touch. A camera's lock asks nothing.
    fn ask_the_reader(&mut self) {
        let Some(send) = self.send.clone() else {
            self.finger = Finger::Absent;
            return;
        };
        let user = self.user.clone();
        self.finger = Finger::Asking;
        if !self.machine {
            return;
        }
        crate::daemon::about_to_act("the fingerprint reader, asked");
        let started = std::thread::Builder::new()
            .name("lock-finger".into())
            .spawn(move || {
                let heard = at_the_reader(&user, &send);
                let _ = send.send(Heard::Finger(heard));
            });
        if let Err(e) = started {
            tracing::warn!(error = %e, "no thread for the reader; the finger is not offered");
            self.finger = Finger::Unavailable;
        }
    }

    /// SOMEBODY IS HERE — a key, a click: a reader that stopped waiting waits
    /// again.
    fn somebody_is_here(&mut self) {
        if self.finger.rests() {
            self.ask_the_reader();
        }
    }

    /// The password typed, to PAM on a thread.
    fn submit(&mut self, typed: String) {
        let secret = Secret::new(typed);
        if secret.is_empty() || self.said == Some(Said::Checking) {
            return;
        }
        let Some(send) = self.send.clone() else {
            return;
        };
        let user = self.user.clone();
        self.said = Some(Said::Checking);
        if !self.machine {
            return;
        }
        crate::daemon::about_to_act("PAM, asked a password");
        let started = std::thread::Builder::new()
            .name("lock-password".into())
            .spawn(move || {
                let said = ronoctua_pam::verify_saying(&user, &secret).map_err(|told| {
                    // PAM'S WORDS ARE DATA FOR THE LOG, never the secret.
                    tracing::info!(refusal = told.refusal.id(), pam = %told.said, "PAM did not take the password");
                    told.refusal
                });
                drop(secret);
                let _ = send.send(Heard::Password(said));
            });
        if let Err(e) = started {
            tracing::warn!(error = %e, "no thread to check the password");
            self.said = Some(Said::Refused(ronoctua_pam::Refusal::Unavailable));
        }
    }

    /// The session opened: the line wiped, the outcome kept, the door ends.
    fn open(&mut self, ui: &mut Ui) {
        self.line.text.zeroize();
        self.outcome.set(Outcome::Opened);
        ui.unlock();
    }

    /// What a thread handed back.
    pub fn heard(&mut self, ui: &mut Ui, heard: Heard) {
        match heard {
            Heard::Password(Ok(())) => {
                tracing::info!("the password opened the session");
                self.open(ui);
            }
            Heard::Password(Err(refusal)) => {
                tracing::info!(
                    refusal = refusal.id(),
                    said = refusal_key(refusal),
                    "the password was refused"
                );
                self.said = Some(Said::Refused(refusal));
            }
            Heard::Finger(AtTheReader::Matched) => {
                tracing::info!("a finger opened the session");
                self.open(ui);
            }
            Heard::Finger(AtTheReader::Absent) => self.finger = Finger::Absent,
            Heard::Finger(AtTheReader::Offered) => self.finger = Finger::Waiting,
            Heard::Finger(AtTheReader::Missed) => self.finger = Finger::Missed,
            Heard::Finger(AtTheReader::Unavailable) => self.finger = Finger::Unavailable,
        }
        ui.ask_paint();
    }

    /// The control drawn under `(x, y)`.
    fn mark_at(&self, (x, y): (f64, f64)) -> Option<crate::chrome::Mark> {
        let (line, cell) = crate::text_select::Selecting::cell_of(x, y)?;
        self.marks.iter().find(|m| m.covers(line, cell)).cloned()
    }

    /// The modeline: the lock, whose session, and the reader.
    fn modeline(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        use crate::chrome::{Piece, Register};
        let mut out = vec![
            Piece {
                text: format!("{} ", tokens::icon::DEVICE_LOCKED),
                register: Register::Glyph,
                id: None,
                press: None,
            },
            Piece::said(texts.t(crate::key::LOCK_LOCKED)),
            Piece::separator(),
            Piece::prose(texts.t_with(crate::key::LOCK_ACCOUNT, &[("user", self.user.as_str())])),
        ];
        if let Some(words) = self.finger.words() {
            out.push(Piece::separator());
            out.push(Piece::prose(format!(
                "{} {}",
                tokens::icon::SECURITY_FINGERPRINT,
                texts.t(words)
            )));
            if self.finger.rests() {
                out.push(Piece::separator());
                out.extend(crate::chrome::hint_of(OF_THE_FINGER, texts));
            }
        }
        out
    }

    /// The minibuffer: the password asked, as every line asks — and its `ESC`
    /// explained as the lock's. The line's own sentence says the question is
    /// let go; here nothing lets it go, and `ESC` only clears what was typed.
    fn asked(&self, texts: &ronoctua_i18n::Texts, room: u16) -> Vec<crate::chrome::Piece> {
        let mut pieces = crate::door::list::question_pieces(&self.line, texts, room);
        for p in &mut pieces {
            if p.id == Some("line-cancel") {
                p.id = Some(CLEAR);
            }
        }
        pieces
    }

    /// The sentence above the two lines, and its colour.
    fn said_now(&self, texts: &ronoctua_i18n::Texts) -> Option<(String, tokens::Color)> {
        Some(match self.said? {
            Said::Checking => (texts.t(crate::key::LOCK_CHECKING), tokens::color::TEXT_DIM),
            Said::Refused(r) => (texts.t(refusal_key(r)), tokens::color::STATE_ERROR),
        })
    }
}

/// The reader's thread: is there one, a finger on it — and the touch.
fn at_the_reader(user: &str, send: &channel::Sender<Heard>) -> AtTheReader {
    use ronoctua_security::fprint;
    match fprint::reader() {
        Ok(Some(_)) => {}
        Ok(None) => return AtTheReader::Absent,
        Err(e) => {
            tracing::debug!(error = ?e, "the reader could not be asked; no finger is offered");
            return AtTheReader::Absent;
        }
    }
    match fprint::enrolled(user) {
        Ok(fingers) if !fingers.is_empty() => {}
        Ok(_) => return AtTheReader::Absent,
        Err(e) => {
            tracing::debug!(error = ?e, "the enrolled fingers could not be read; none is offered");
            return AtTheReader::Absent;
        }
    }
    let _ = send.send(Heard::Finger(AtTheReader::Offered));
    match ronoctua_pam::verify_finger(user) {
        Ok(()) => AtTheReader::Matched,
        Err(told) => {
            tracing::info!(refusal = told.refusal.id(), pam = %told.said, "the reader did not open the session");
            after_the_reader(told.refusal)
        }
    }
}

impl Content for Lock {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t(crate::key::CMD_LOCK_SCREEN)
    }

    /// THE SESSION LOCKED, the door's surface the lock's on the screen that
    /// had the focus (`crate::session_lock`).
    fn open(
        &self,
        wiring: &Wiring<'_, Self>,
        surface: wl_surface::WlSurface,
    ) -> anyhow::Result<Shell> {
        crate::session_lock::lock(wiring, surface, self.wanted.clone())
    }

    /// THE FOOTER'S PAGE: the lock is the footer's two lines over the desk.
    fn page(&self) -> crate::theme::Page {
        crate::theme::footer_page()
    }

    fn screens_locked(&mut self, _: &mut Ui) {
        if self.outcome.get() == Outcome::Asked {
            self.outcome.set(Outcome::Locked);
        }
    }

    fn lock_ended(&mut self, _: &mut Ui, was_locked: bool) {
        if self.outcome.get() != Outcome::Opened {
            self.outcome.set(if was_locked {
                Outcome::EndedThere
            } else {
                Outcome::NeverLocked
            });
        }
    }

    /// NOTHING CLOSES A LOCK but the person's password or finger.
    fn closing(&mut self, _: &mut Ui) -> bool {
        false
    }

    /// No header, no drawer: `Esc` is the line's.
    fn header_keys(&self) -> bool {
        false
    }

    /// NO SWEEP OF TEXT on a locked screen.
    fn text_area(&self, _: &Ui, _: u16, _: u16) -> Option<crate::text_select::Area> {
        None
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        if key.notation() == OF_THE_FINGER[0].0 {
            if self.finger.rests() {
                self.ask_the_reader();
                ui.ask_paint();
            }
            return;
        }
        self.somebody_is_here();
        if crate::line::pastes(key) {
            ui.paste();
            return;
        }
        match self.line.key(key) {
            Step::Answer(_, typed) => {
                self.line.text.zeroize();
                self.submit(typed);
            }
            // `ESC` LETS WHAT WAS TYPED GO, and never the lock.
            Step::Cancel => self.line.text.zeroize(),
            Step::Changed => {
                // TYPING AGAIN takes the last refusal off: it was about the
                // password before this one.
                if matches!(self.said, Some(Said::Refused(_))) {
                    self.said = None;
                }
            }
            Step::WantsPaste => ui.paste(),
            Step::Nothing => {}
        }
        ui.ask_paint();
    }

    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        if self.line.paste(text) == Step::Changed {
            ui.ask_paint();
        }
    }

    fn pointer(&mut self, ui: &mut Ui, _: &mut Hand<()>, events: &[PointerEvent]) {
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    let lit = self.mark_at(e.position).map(|m| m.id);
                    if lit != self.lit {
                        self.lit = lit;
                        ui.ask_paint();
                    }
                }
                PointerEventKind::Leave { .. } => {
                    if self.lit.take().is_some() {
                        ui.ask_paint();
                    }
                }
                // A CONTROL OF THE TWO LINES IS A CLICK: it presses its key,
                // through the keyboard's own path.
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    match self.mark_at(e.position).and_then(|m| m.press) {
                        Some(crate::chrome::Act::Key(key)) => {
                            let (event, mods) = crate::door::keys::event_of(key);
                            self.typed(ui, &event, mods);
                        }
                        _ => self.somebody_is_here(),
                    }
                    ui.ask_paint();
                }
                _ => {}
            }
        }
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        self.marks.clear();
        let lines = ui.lines;
        if lines < 2 {
            return;
        }
        let texts = ui.texts.clone();
        let (margin, room) = (ui.margin, ui.columns);
        let lit = self.lit;
        // THE MINIBUFFER, at the bottom: the password asked.
        let asked_at = lines - 1;
        let asked = crate::chrome::lit(&self.asked(&texts, room), lit);
        // THE MODELINE above it: locked, whose, and the reader.
        let modeline_at = lines - 2;
        let modeline = crate::chrome::lit(&self.modeline(&texts), lit);
        for (pieces, line) in [(&asked, asked_at), (&modeline, modeline_at)] {
            canvas.spans.extend(crate::chrome::hint_spans(
                pieces,
                Size::Grid,
                margin,
                line,
                room,
            ));
            self.marks
                .extend(crate::chrome::hint_marks(pieces, margin, line, room));
        }
        // WHAT THE LOCK SAYS, above the two — wrapped, never cut.
        let mut above = modeline_at;
        if let Some((sentence, colour)) = self.said_now(&texts) {
            for part in crate::chrome::note_lines(&sentence, room).into_iter().rev() {
                let Some(line) = above.checked_sub(1) else {
                    break;
                };
                above = line;
                canvas.spans.push(ronoctua_render::Span {
                    size: Size::Grid,
                    text: part,
                    column: margin,
                    line,
                    color: colour,
                    edge: false,
                    foot: false,
                });
            }
        }
        // AND WHAT THE CONTROL UNDER THE POINTER DOES, above that.
        if let (Some(id), Some(line)) = (lit, above.checked_sub(1))
            && let Some(sentence) = crate::chrome::hint_sentence(id, &texts)
        {
            let explained = [crate::chrome::Piece {
                text: sentence,
                register: crate::chrome::Register::Explain,
                id: None,
                press: None,
            }];
            canvas.spans.extend(crate::chrome::hint_spans(
                &explained,
                Size::Grid,
                margin,
                line,
                room,
            ));
        }
    }
}

/// What this crate adds to the leader's keymap: the command that locks, in
/// the session's group beside suspending and quitting (`s q`).
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "lock",
    commands: &[ronoctua_cmd::Command {
        id: "lock-screen",
        label: crate::key::CMD_LOCK_SCREEN,
    }],
    // WRITTEN OUT, and not through `COMMAND`, so the check that every key
    // reaches an arm of the footer's `run_with` reads it.
    bindings: &[("s q l", "lock-screen", None)],
    ..crate::bindings::Contribution::NONE
};

/// The camera's scene: the lock as it would be, given in words.
#[derive(Debug, Default)]
struct Camera {
    user: Option<String>,
    finger: Option<Finger>,
    said: Option<Said>,
    typed: usize,
}

impl crate::photo::scenes::Scene for Camera {
    const DOOR: &'static str = crate::session_lock::LOCK_WORD;
    const IS: &'static str =
        "the lock screen over a scene given in words; nothing is locked and nothing is asked";
    const WORDS: &'static str = "\
  --user <name>         the account shown            (default: alice)
  --finger <state>      none, waiting, missed or unavailable (default: none)
  --said <what>         none, checking, wrong, unavailable or no-service
  --typed <n>           how many letters are typed   (default: 0)";

    fn word(
        &mut self,
        flag: &str,
        value: &mut dyn FnMut() -> anyhow::Result<String>,
    ) -> anyhow::Result<bool> {
        match flag {
            "--user" => self.user = Some(value()?),
            "--finger" => {
                self.finger = Some(match value()?.as_str() {
                    "none" => Finger::Absent,
                    "waiting" => Finger::Waiting,
                    "missed" => Finger::Missed,
                    "unavailable" => Finger::Unavailable,
                    other => {
                        anyhow::bail!("`--finger {other}` is none, waiting, missed or unavailable")
                    }
                });
            }
            "--said" => {
                self.said = match value()?.as_str() {
                    "none" => None,
                    "checking" => Some(Said::Checking),
                    "wrong" => Some(Said::Refused(ronoctua_pam::Refusal::Wrong)),
                    "unavailable" => Some(Said::Refused(ronoctua_pam::Refusal::Unavailable)),
                    "no-service" => Some(Said::Refused(ronoctua_pam::Refusal::NoService)),
                    other => anyhow::bail!(
                        "`--said {other}` is none, checking, wrong, unavailable or no-service"
                    ),
                };
            }
            "--typed" => self.typed = crate::photo::number("--typed", &value()?, 0..=64)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn shoot(self, ui: Ui, asked: &crate::photo::scenes::Asked) -> anyhow::Result<()> {
        let user = self.user.unwrap_or_else(|| "alice".to_owned());
        let mut lock = Lock::new(user, None, None, Rc::new(Cell::new(Outcome::Locked)));
        lock.finger = self.finger.unwrap_or(Finger::Absent);
        lock.said = self.said;
        lock.line.text = "x".repeat(self.typed);
        crate::photo::scenes::walked(base::Detached::new(ui, lock), asked)
    }
}

/// `ronoctua-shell photograph --door lock …`, to its end.
pub fn camera(rest: &[String]) -> ! {
    crate::photo::scenes::taken(rest, Camera::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    /// A lock whose questions reach nobody: the rules walked, no thread.
    fn lock() -> (Lock, Ui, channel::Channel<Heard>) {
        let (send, heard) = channel::channel::<Heard>();
        let mut l = Lock::new(
            "alice".to_owned(),
            None,
            Some(send),
            Rc::new(Cell::new(Outcome::Locked)),
        );
        l.machine = false;
        (l, Ui::for_tests(texts()), heard)
    }

    fn press(l: &mut Lock, ui: &mut Ui, notation: &str) {
        let key = ronoctua_keymap::Key::parse(notation).expect(notation);
        let (event, mods) = crate::door::keys::event_of(key);
        l.typed(ui, &event, mods);
    }

    /// FAILS IF: what is typed is not hidden, or `ESC` leaves it behind.
    #[test]
    fn what_is_typed_is_masked_and_esc_lets_it_go() {
        let (mut l, mut ui, _heard) = lock();
        for k in ["q", "z", "x"] {
            press(&mut l, &mut ui, k);
        }
        assert_eq!(l.line.text, "qzx");
        assert_eq!(l.line.shown(), "•••");
        assert!(
            !format!("{:?}", l.line).contains("qzx"),
            "the secret reached Debug"
        );
        press(&mut l, &mut ui, "ESC");
        assert!(l.line.text.is_empty(), "ESC wipes what was typed");
        assert!(!ui.unlocking(), "and ESC never opens the lock");
    }

    /// FAILS IF: `RET` with nothing typed asks PAM, or a second `RET` while
    /// the first is checked asks again.
    #[test]
    fn an_empty_line_asks_nothing_and_one_question_at_a_time() {
        let (mut l, mut ui, _heard) = lock();
        press(&mut l, &mut ui, "RET");
        assert_eq!(l.said, None, "an empty password is never asked");
        l.said = Some(Said::Checking);
        press(&mut l, &mut ui, "x");
        press(&mut l, &mut ui, "RET");
        assert_eq!(l.said, Some(Said::Checking));
        assert!(l.line.text.is_empty(), "the line is wiped on every answer");
    }

    /// FAILS IF: a wrong password opens the session, or its sentence does not
    /// say so — or a right one, or a finger, does not open it.
    #[test]
    fn only_a_yes_opens_and_a_refusal_is_said() {
        let (mut l, mut ui, _heard) = lock();
        l.said = Some(Said::Checking);
        l.heard(&mut ui, Heard::Password(Err(ronoctua_pam::Refusal::Wrong)));
        assert!(!ui.unlocking());
        assert_eq!(l.said, Some(Said::Refused(ronoctua_pam::Refusal::Wrong)));
        let (sentence, colour) = l.said_now(&ui.texts).expect("a sentence");
        assert_eq!(sentence, ui.texts.t(crate::key::LOCK_WRONG));
        assert_eq!(colour, tokens::color::STATE_ERROR);
        // TYPING AGAIN takes it off.
        press(&mut l, &mut ui, "y");
        assert_eq!(l.said, None);
        l.heard(&mut ui, Heard::Password(Ok(())));
        assert!(ui.unlocking(), "the right password opens the session");
        assert_eq!(l.outcome.get(), Outcome::Opened);

        let (mut l, mut ui, _heard) = lock();
        l.heard(&mut ui, Heard::Finger(AtTheReader::Offered));
        assert_eq!(l.finger, Finger::Waiting);
        assert!(!ui.unlocking());
        l.heard(&mut ui, Heard::Finger(AtTheReader::Matched));
        assert!(ui.unlocking(), "a finger that matched opens the session");
    }

    /// FAILS IF: a machine with no reader (or no finger enrolled) says a word
    /// about a finger, or one with a reader does not.
    #[test]
    fn no_reader_says_nothing_of_a_finger() {
        let (mut l, mut ui, _heard) = lock();
        let texts = texts();
        l.heard(&mut ui, Heard::Finger(AtTheReader::Absent));
        let words: String = l.modeline(&texts).iter().map(|p| p.text.clone()).collect();
        assert!(
            !words.contains(&*tokens::icon::SECURITY_FINGERPRINT.to_string()),
            "{words}"
        );
        l.heard(&mut ui, Heard::Finger(AtTheReader::Offered));
        let words: String = l.modeline(&texts).iter().map(|p| p.text.clone()).collect();
        assert!(
            words.contains(&texts.t(crate::key::LOCK_FINGER_WAITING)),
            "{words}"
        );
        // A NO-SERVICE IS NO FINGER, and a no-match is one to offer again.
        assert_eq!(
            after_the_reader(ronoctua_pam::Refusal::NoService),
            AtTheReader::Absent
        );
        assert_eq!(
            after_the_reader(ronoctua_pam::Refusal::Wrong),
            AtTheReader::Missed
        );
    }

    /// FAILS IF: a reader that stopped waiting is not offered again when
    /// somebody is there, or one that waits is asked twice.
    #[test]
    fn a_reader_that_stopped_waits_again_when_somebody_is_here() {
        let (mut l, mut ui, _heard) = lock();
        l.heard(&mut ui, Heard::Finger(AtTheReader::Missed));
        assert!(l.finger.rests());
        assert!(
            l.modeline(&ui.texts).iter().any(|p| p.id == Some(FINGER)),
            "C-f is taught while it rests"
        );
        // A KEY is somebody here: the reader is asked again (the thread's
        // answer is not waited for: the state is `Asking` at once).
        press(&mut l, &mut ui, "a");
        assert_eq!(l.finger, Finger::Asking);
        // AND ONE THAT WAITS is not asked again.
        l.finger = Finger::Waiting;
        press(&mut l, &mut ui, "C-f");
        assert_eq!(l.finger, Finger::Waiting);
        assert_eq!(l.line.text, "a", "C-f is no letter of the password");
    }

    /// FAILS IF: the lock's keys and words stop being what the system
    /// teaches: every control of the two lines explains itself.
    #[test]
    fn every_control_of_the_two_lines_explains_itself() {
        let texts = texts();
        let (mut l, _ui, _heard) = lock();
        l.finger = Finger::Missed;
        let mut pieces = l.modeline(&texts);
        pieces.extend(l.asked(&texts, 80));
        let ids: Vec<&str> = pieces.iter().filter_map(|p| p.id).collect();
        assert!(
            ids.contains(&FINGER) && ids.contains(&"line-answer"),
            "{ids:?}"
        );
        // `ESC` IS EXPLAINED AS THE LOCK'S: it clears what was typed, and the
        // line's sentence — the question let go — is not true here.
        assert!(
            ids.contains(&CLEAR) && !ids.contains(&"line-cancel"),
            "{ids:?}"
        );
        assert_ne!(
            crate::chrome::hint_sentence(CLEAR, &texts),
            crate::chrome::hint_sentence("line-cancel", &texts)
        );
        for id in ids {
            assert!(
                crate::chrome::hint_sentence(id, &texts).is_some(),
                "{id} has no sentence under the pointer"
            );
        }
        for r in ronoctua_pam::Refusal::ALL {
            assert!(!texts.t(refusal_key(r)).starts_with('['), "{r:?}");
        }
    }
}
