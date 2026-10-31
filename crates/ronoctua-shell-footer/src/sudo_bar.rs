//! THE SUDO BAR — the one place a screen asks for root.
//!
//! # What it is
//!
//! A strip of the footer's own surface, above the modeline and above the
//! notification band, that exists only while a root gesture is being asked
//! (owner, 2026-10-11 — the amendment of invariant 14 in `AGENTS.md`). It
//! says who asks and what for ([`Deed::sentence`]), what sudo is waiting for
//! — the password, a touch of a security key, a finger on the reader — and
//! offers "use the password" while a key or a finger is awaited; `Esc`
//! cancels. It shouts by colour, the one device invariant 15 leaves: the
//! maximum alert's second pair (`alert.ground-alt` / `alert.text-alt`, solved
//! and gated like every ground), held still — the battery's bar is the one
//! that alternates.
//!
//! # Every root gesture of every screen goes through it
//!
//! The sudo door (`ronoctua_security::authority`) is reached from this file
//! and no other (`sudo-door-is-one`): a view hands its door a [`Gesture`],
//! and the gesture runs each verb here.
//!
//! - **In the footer** ([`Gate::footer`]) the gesture runs the conversation on
//!   the view's own thread, and what sudo asks comes to the footer's loop as a
//!   [`ToBar`]; the person's answer goes back through the [`Answerer`] the
//!   bar holds.
//! - **In a window** ([`Gate::window`]) the gesture writes the deed on the
//!   footer's control socket (`root-gesture <json>`, [`ORDER`]) and waits, on the
//!   window's thread, for the footer's lines: the helper's progress words and
//!   the verdict, in the helper's own grammar ([`Said`]). The window says the
//!   verdict in its own door; a footer that is not there is a sentence
//!   ([`Why::NoFooter`]). One connection is one gesture: a second verb of the
//!   same gesture (the file manager's set of paths) goes down it, and the
//!   footer keeps the password typed for it until the window lets go.
//!
//! The footer serves a window's deed on a thread of its own ([`serve`]),
//! with the same conversation the footer's views use.
//!
//! # The conversation
//!
//! [`conversation`] takes the bar's TURN (one root gesture is asked at a
//! time), runs `authority::converse` — `sudo -n` first, so an open window or
//! sudo's fresh timestamp asks nothing and the bar never appears — and, when
//! sudo wants the person, shows what it asks. A wrong password is asked
//! again, up to [`ATTEMPTS`] times as sudo itself does in a terminal, with
//! the bar saying it was wrong; the door never tries a password again in the
//! person's name. The bar goes when the conversation ends, however it ends.
//!
//! # The password
//!
//! Typed into the bar's own secret line (`crate::line::Line`, one dot per
//! cell), never echoed, wiped when answered, cancelled or dropped, and handed
//! to the door as a [`Secret`] — which writes it only to sudo's stdin or to
//! its own terminal right after its own sentinel. A gesture of several verbs
//! keeps the password that answered the first (in a [`Secret`], wiped with
//! the gesture) and answers the next ones with it; a touch or a finger is
//! asked again each time.
//!
//! # Who may claim to ask
//!
//! The control socket is the person's (`$XDG_RUNTIME_DIR`, `0700`): what
//! reaches it is a process of the person's, which could run sudo in a
//! terminal as well. The bar adds no authority — the person still answers —
//! and a window's deed is said as a WINDOW's whatever it claims ([`serve`]
//! sets it). The verb is the helper's closed set, read back through
//! `RootVerb::parse` before anything is asked ([`Deed::valid`]).

use std::io::{BufRead as _, Write as _};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex, PoisonError};

use crate::key;
use crate::sudo::{Ask, Asker, Deed, Gate, ORDER, Root};
use ronoctua_lookup::keys::Secret;
use ronoctua_render::layout;
use ronoctua_security::Why;
use ronoctua_security::authority::{Answer, Answerer, Asking};
use ronoctua_security::verb::{RelaxSpan, RootVerb, Said};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use zeroize::Zeroize as _;

/// How many lines the bar takes: who asks, and what is asked. A constant,
/// never a function of the width — the footer's height is the same on every
/// output.
pub const ROWS: u16 = 2;

/// How many times a wrong password is asked again — sudo's own
/// `passwd_tries`.
pub const ATTEMPTS: u8 = 3;

/// The key that leaves a key or a finger for the password — beside its
/// binding, never text in a `.ftl` (`keys-are-never-text`).
pub const USE_PASSWORD_KEY: &str = "C-p";
/// The key that answers.
pub const ANSWER_KEY: &str = "RET";
/// The key that cancels.
pub const CANCEL_KEY: &str = "ESC";

// ─── who asks, and what for ─────────────────────────────────────────────────

// ─── the gestures ───────────────────────────────────────────────────────────

/// The password that answered a gesture's first verb, kept for the rest of
/// it — wiped with the gesture ([`Secret`] zeroes itself).
#[derive(Debug, Clone, Default)]
struct Kept(Arc<Mutex<Option<Secret>>>);

impl Kept {
    fn get(&self) -> Option<Secret> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set(&self, secret: Secret) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(secret);
    }

    fn clear(&self) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// The footer's gate: each gesture a conversation on the asking view's
/// thread, with the bar hearing through `to_bar` (the footer's loop).
#[must_use]
pub fn gate(to_bar: Sender<ToBar>) -> Gate {
    Gate::footer(move |asker| {
        Box::new(InFooter {
            asker,
            to_bar: to_bar.clone(),
            kept: Kept::default(),
        })
    })
}

/// The footer's gesture.
struct InFooter {
    asker: Asker,
    to_bar: Sender<ToBar>,
    kept: Kept,
}

impl std::fmt::Debug for InFooter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InFooter")
            .field("asker", &self.asker)
            .finish_non_exhaustive()
    }
}

impl Root for InFooter {
    fn run(&mut self, ask: Ask, step: &mut dyn FnMut(&str)) -> Result<(), Why> {
        let deed = Deed::of(self.asker, false, ask);
        if !deed.valid() {
            return Err(Why::Invalid);
        }
        let to_bar = self.to_bar.clone();
        conversation(
            &deed,
            &move |m: ToBar| {
                let _ = to_bar.send(m);
            },
            &self.kept,
            &Cancel::default(),
            &mut at_the_door,
            step,
        )
    }
}

// ─── the conversation ───────────────────────────────────────────────────────

/// One root gesture is asked at a time: a second waits for the first to end.
static TURN: Mutex<()> = Mutex::new(());

/// The next conversation's number — what tells the bar's messages about one
/// from another's.
static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// What a conversation tells the footer's loop.
#[derive(Debug)]
pub enum ToBar {
    /// sudo is waiting for this — the bar comes up, or says the new thing.
    Asked {
        /// The conversation.
        id: u64,
        /// Who asks, and what for.
        deed: Deed,
        /// What sudo waits for.
        asking: Asking,
        /// Where the person's answer goes.
        answerer: BarAnswerer,
        /// How many passwords were wrong before this attempt.
        wrong: u8,
    },
    /// The conversation ended, however it ended — the bar goes.
    Over {
        /// The conversation.
        id: u64,
    },
}

/// The bar's end of a conversation: the person's answer goes to the door,
/// and a password is kept for the gesture's next verbs.
#[derive(Debug, Clone)]
pub struct BarAnswerer {
    answerer: Answerer,
    kept: Kept,
}

impl BarAnswerer {
    /// Answers the conversation.
    pub fn answer(&self, answer: Answer) {
        if let Answer::Password(secret) = &answer {
            self.kept.set(secret.clone());
        }
        self.answerer.answer(answer);
    }
}

/// What ends a conversation from outside it: the window that asked went away.
#[derive(Debug, Clone, Default)]
struct Cancel(Arc<Mutex<Cancelling>>);

#[derive(Debug, Default)]
struct Cancelling {
    cancelled: bool,
    now: Option<Answerer>,
}

impl Cancel {
    /// The conversation now under way answers to `answerer`; `false` when it
    /// was cancelled already (and was told so).
    fn hold(&self, answerer: &Answerer) -> bool {
        let mut c = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if c.cancelled {
            answerer.answer(Answer::Cancel);
            return false;
        }
        c.now = Some(answerer.clone());
        true
    }

    fn release(&self) {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).now = None;
    }

    /// Cancels what is under way and everything after it.
    fn cancel(&self) {
        let mut c = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        c.cancelled = true;
        if let Some(answerer) = c.now.take() {
            answerer.answer(Answer::Cancel);
        }
    }

    fn cancelled(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .cancelled
    }
}

/// One attempt of a verb at the sudo door: the verb, the person's end of the
/// conversation (`Answers`, and the `Answerer` paired with it), where what
/// it asks goes, and where its progress goes.
type Attempt<'a> = dyn FnMut(
        &RootVerb,
        ronoctua_security::authority::Answers,
        &Answerer,
        &mut dyn FnMut(Asking),
        &mut dyn FnMut(&str),
    ) -> Result<(), Why>
    + 'a;

/// The real attempt: `authority::converse` — the one call of the sudo door
/// in the tree that is not a test's (`sudo-door-is-one`).
fn at_the_door(
    verb: &RootVerb,
    answers: ronoctua_security::authority::Answers,
    _: &Answerer,
    asked: &mut dyn FnMut(Asking),
    step: &mut dyn FnMut(&str),
) -> Result<(), Why> {
    use ronoctua_security::authority::Event;
    ronoctua_security::authority::converse(verb.clone(), answers, &mut |event| match event {
        Event::Asking(asking) => asked(asking),
        Event::Step(word) => step(&word),
    })
    .map(|_| ())
    .map_err(|r| r.why)
}

/// Runs `deed` through the sudo door, asking in the bar: the TURN taken,
/// up to [`ATTEMPTS`] attempts while the password is wrong, what each asks
/// told to the bar (`tell`) — unless `kept` holds the gesture's password,
/// which answers at once — and the bar told the conversation is over at the
/// end, however it ended.
fn conversation(
    deed: &Deed,
    tell: &dyn Fn(ToBar),
    kept: &Kept,
    cancel: &Cancel,
    attempt: &mut Attempt<'_>,
    step: &mut dyn FnMut(&str),
) -> Result<(), Why> {
    let _turn = TURN.lock().unwrap_or_else(PoisonError::into_inner);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut result = Err(Why::Cancelled);
    for wrong in 0..ATTEMPTS {
        let (answerer, answers) = ronoctua_security::authority::answers();
        if !cancel.hold(&answerer) {
            result = Err(Why::Cancelled);
            break;
        }
        let bar = BarAnswerer {
            answerer: answerer.clone(),
            kept: kept.clone(),
        };
        let mut asked = |asking: Asking| match kept.get() {
            // THE GESTURE'S PASSWORD answers its next verbs, and the bar is
            // not raised for a question already answered.
            Some(password) => answerer.answer(Answer::Password(password)),
            None => tell(ToBar::Asked {
                id,
                deed: deed.clone(),
                asking,
                answerer: bar.clone(),
                wrong,
            }),
        };
        result = attempt(&deed.verb, answers, &answerer, &mut asked, step);
        cancel.release();
        if result == Err(Why::WrongPassword) {
            kept.clear();
            if !cancel.cancelled() {
                continue;
            }
        }
        break;
    }
    tell(ToBar::Over { id });
    result
}

/// Serves a window's root gesture, on a thread of its own: the deed on
/// `first` (the words after [`ORDER`]) and every one the window sends after
/// it down the same connection, each answered with the helper's progress
/// words and a verdict line. When the window goes away the conversation
/// under way is cancelled, and the gesture's password goes with the thread.
pub fn serve(stream: Arc<UnixStream>, first: String, to_bar: Sender<ToBar>) {
    let spawned = std::thread::Builder::new()
        .name("sudo-bar-window".into())
        .spawn(move || serve_on(&stream, first, &to_bar));
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "no thread for a window's root gesture");
    }
}

fn serve_on(stream: &Arc<UnixStream>, first: String, to_bar: &Sender<ToBar>) {
    let kept = Kept::default();
    let cancel = Cancel::default();
    // THE WINDOW'S NEXT DEEDS, and its going away, heard on a thread: a
    // window that dies while the bar asks cancels what it asked.
    let (next, deeds) = std::sync::mpsc::channel::<String>();
    if let Ok(reading) = stream.try_clone() {
        let watching = cancel.clone();
        let heard = std::thread::Builder::new()
            .name("sudo-bar-hear".into())
            .spawn(move || {
                for line in std::io::BufReader::new(reading).lines() {
                    let Ok(line) = line else { break };
                    if next.send(line).is_err() {
                        break;
                    }
                }
                watching.cancel();
            });
        if let Err(e) = heard {
            tracing::warn!(error = %e, "a window's root gesture cannot be heard going away");
        }
    }
    let tell = {
        let to_bar = to_bar.clone();
        move |m: ToBar| {
            let _ = to_bar.send(m);
        }
    };
    let say = |said: &Said| {
        let line = format!("{}\n", said.line());
        let _ = (&**stream).write_all(line.as_bytes());
    };
    let mut line = Some(first);
    while let Some(words) = line.take() {
        let words = words.trim();
        let words = words.strip_prefix(ORDER).map_or(words, str::trim_start);
        let verdict = match deed_of(words) {
            Some(deed) => conversation(
                &deed,
                &tell,
                &kept,
                &cancel,
                &mut at_the_door,
                &mut |word| say(&Said::Step(word.to_owned())),
            ),
            None => {
                tracing::warn!("a window's root gesture that does not read");
                Err(Why::Invalid)
            }
        };
        say(&match verdict {
            Ok(()) => Said::Done,
            Err(why) => Said::Refused(why),
        });
        line = deeds.recv().ok();
    }
}

/// A window's deed, from the JSON after [`ORDER`] — said as a window's
/// whatever it claims, and `None` unless it is one the helper would take.
/// Pure.
fn deed_of(json: &str) -> Option<Deed> {
    let mut deed: Deed = serde_json::from_str(json).ok()?;
    deed.window = true;
    deed.valid().then_some(deed)
}

// ─── a key's passphrase, in the same bar ────────────────────────────────────
//
// The ssh askpass (`ronoctua-ssh-askpass`, run by ssh as `$SSH_ASKPASS`) asks a
// key's passphrase in THIS bar, not a floating window: it writes one line to
// the control socket, the bar comes up asking the password, and what is typed
// goes back down that line. It never reaches the sudo door — the bar's other
// half — so `sudo-door-is-one` holds: this half answers a socket, not root.

/// What the askpass bar hands back to the socket that asked.
#[derive(Debug)]
pub enum AskAnswer {
    /// The passphrase typed — possibly empty, as some keys have none.
    Password(Secret),
    /// The person cancelled, or the bar fell for them (`Esc`, the socket gone).
    Cancel,
}

/// Which secret the bar asks for: it picks the title and the line's label. The
/// helper's own name decides it (`nix/ssh-askpass.rs`), written after the
/// `askpass` word (`crate::leader::ASKPASS`); a name it does not know reads as
/// [`Kind::Ssh`], the key passphrase this bar began with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An SSH key's passphrase.
    Ssh,
    /// The system (sudo) password.
    Sudo,
    /// A security key's PIN.
    Pin,
    /// A login password (`user@host`).
    LoginPassword,
    /// A security key's touch (user presence): a CONFIRMATION, not a secret —
    /// no line is typed into, and Answer alone (or Enter) confirms it.
    Touch,
    /// A host's authenticity on a first connection: `yes` is typed to trust it.
    HostAuthenticity,
    /// An ssh ask none of the above name: a neutral title and its own prompt,
    /// never titled a passphrase.
    SshOther,
}

/// The token the askpass helper sends when sudo runs it (`SUDO_ASKPASS`,
/// `nix/ssh-askpass.rs`, spelled again there): the person's own password, for
/// a root gesture typed in a terminal — [`Kind::Sudo`].
///
/// NAMED BY WHAT IS ASKED, never by the program asking: only the sudo door
/// (`ronoctua_security::authority`) names sudo (`sudo-door-is-one`), and a
/// token that spelled it here would be a second name for the door.
pub const SYSTEM_PASSWORD: &str = "system-password";

impl Kind {
    /// The kind the helper's token names (`nix/ssh-askpass.rs`'s `ssh_kind`),
    /// and whether the ask came with a CONTEXT (what is served and from
    /// where). A token it does not know is [`Kind::SshOther`], shown as what
    /// it is — an ssh ask — and never as a passphrase.
    ///
    /// UNLESS IT CAME WITH A CONTEXT: the helper sends one for the system
    /// password alone, and `-` for every ssh ask, so an unknown token that
    /// carries one is the system password asked by an OLDER helper, one that
    /// spelled the kind by the program asking. `SUDO_ASKPASS` is a session
    /// variable: a terminal of a session that began before a switch still
    /// runs the helper of that session's generation until the person logs in
    /// again, long after this footer was restarted on the new one — and the
    /// bar titles that ask for what it is all the while.
    #[must_use]
    pub fn of(token: &str, context: bool) -> Self {
        match token {
            SYSTEM_PASSWORD => Self::Sudo,
            "ssh-passphrase" => Self::Ssh,
            "ssh-pin" => Self::Pin,
            "ssh-password" => Self::LoginPassword,
            "ssh-touch" => Self::Touch,
            "ssh-host" => Self::HostAuthenticity,
            "ssh" => Self::SshOther,
            _ if context => Self::Sudo,
            _ => Self::SshOther,
        }
    }

    /// Whether a line is typed into for this kind: all but a touch, which
    /// carries no secret — its bar shows no line, and Answer confirms it.
    const fn typed(self) -> bool {
        !matches!(self, Self::Touch)
    }

    /// The bar's title for this kind.
    const fn title(self) -> &'static str {
        match self {
            Self::Ssh => key::ASKPASS_BAR_TITLE_SSH,
            Self::Sudo => key::ASKPASS_BAR_TITLE_SUDO,
            Self::Pin => key::ASKPASS_BAR_TITLE_PIN,
            Self::LoginPassword => key::ASKPASS_BAR_TITLE_PASSWORD,
            Self::Touch => key::ASKPASS_BAR_TITLE_TOUCH,
            Self::HostAuthenticity => key::ASKPASS_BAR_TITLE_HOST,
            Self::SshOther => key::ASKPASS_BAR_TITLE_SSH_OTHER,
        }
    }

    /// The label the line (or, for a touch, the bar) carries for this kind.
    const fn ask(self) -> &'static str {
        match self {
            Self::Ssh => key::ASKPASS_BAR_ASK_SSH,
            Self::Sudo => key::ASKPASS_BAR_ASK_SUDO,
            Self::Pin => key::ASKPASS_BAR_ASK_PIN,
            Self::LoginPassword => key::ASKPASS_BAR_ASK_PASSWORD,
            Self::Touch => key::ASKPASS_BAR_ASK_TOUCH,
            Self::HostAuthenticity => key::ASKPASS_BAR_ASK_HOST,
            Self::SshOther => key::ASKPASS_BAR_ASK_SSH_OTHER,
        }
    }
}

/// What a passphrase conversation tells the footer's loop: raise the bar, or
/// drop it because the socket that asked went away.
pub enum ToAsk {
    /// Raise the bar asking a secret; what is typed goes to `sink`.
    Ask {
        /// The conversation, [`NEXT`]'s next — never a sudo one's.
        id: u64,
        /// Which secret, for the title and the label.
        kind: Kind,
        /// The prompt the helper passed, drawn after the title; may be empty.
        prompt: String,
        /// What sudo serves and from where, drawn in the prompt's place when
        /// present (sudo only); `None` for an ssh key's passphrase.
        context: Option<String>,
        /// Where the typed secret (or the cancel) goes.
        sink: std::sync::mpsc::Sender<AskAnswer>,
    },
    /// The socket that asked went away — drop the bar `id` if it is still up.
    Gone {
        /// The conversation.
        id: u64,
    },
}

impl std::fmt::Debug for ToAsk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ask { id, .. } => f
                .debug_struct("Ask")
                .field("id", id)
                .finish_non_exhaustive(),
            Self::Gone { id } => f.debug_struct("Gone").field("id", id).finish(),
        }
    }
}

/// Serves an askpass request on a thread of its own: the bar raised through
/// `to_bar`, the typed passphrase (or nothing, when cancelled) written back
/// down `stream`. A closed `stream` — ssh went away — drops the bar; writing
/// nothing closes it, which ssh reads as "no passphrase" and gives up.
pub fn serve_askpass(
    stream: Arc<UnixStream>,
    kind: Kind,
    context: Option<String>,
    prompt: String,
    to_bar: Sender<ToAsk>,
) {
    let spawned = std::thread::Builder::new()
        .name("askpass".into())
        .spawn(move || serve_askpass_on(&stream, kind, context, prompt, &to_bar));
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "no thread for a key's passphrase");
    }
}

fn serve_askpass_on(
    stream: &Arc<UnixStream>,
    kind: Kind,
    context: Option<String>,
    prompt: String,
    to_bar: &Sender<ToAsk>,
) {
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (sink, answered) = std::sync::mpsc::channel::<AskAnswer>();
    // THE SOCKET GOING AWAY — ssh killed, the terminal closed — drops the bar:
    // the reader sees EOF and tells the loop, which lets the bar's `sink` go
    // and unblocks the wait below.
    if let Ok(reading) = stream.try_clone() {
        let to_bar = to_bar.clone();
        let watching = std::thread::Builder::new()
            .name("askpass-hear".into())
            .spawn(move || {
                for line in std::io::BufReader::new(reading).lines() {
                    if line.is_err() {
                        break;
                    }
                }
                let _ = to_bar.send(ToAsk::Gone { id });
            });
        if let Err(e) = watching {
            tracing::warn!(error = %e, "a key's passphrase cannot be heard going away");
        }
    }
    if to_bar
        .send(ToAsk::Ask {
            id,
            kind,
            prompt,
            context,
            sink,
        })
        .is_err()
    {
        return;
    }
    // The bar answers once, or its `sink` is dropped (cancel, or the socket
    // gone): a passphrase is written back, nothing else is.
    if let Ok(AskAnswer::Password(secret)) = answered.recv() {
        let mut bytes = format!("{}\n", secret.expose()).into_bytes();
        let _ = (&**stream).write_all(&bytes);
        bytes.zeroize();
    }
}

// ─── the bar, in the footer ─────────────────────────────────────────────────

/// A control of the bar, grabbed where it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    /// Answer with what is typed.
    Answer,
    /// Leave the key or the finger, and ask the password.
    UsePassword,
    /// Cancel.
    Cancel,
}

/// What the bar is waiting on after the person answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// A password went to sudo.
    Checking,
    /// The person cancelled.
    Cancelling,
}

/// A secret asked in the bar: its kind (title and label), the helper's prompt,
/// the context (what sudo serves and from where, drawn in place of the prompt
/// when there is one — sudo only), and where what is typed goes — a socket,
/// never the sudo door ([`AskAnswer`]).
struct AskPass {
    kind: Kind,
    prompt: String,
    context: Option<String>,
    sink: std::sync::mpsc::Sender<AskAnswer>,
}

/// What is being asked, while it is.
struct Up {
    id: u64,
    deed: Deed,
    asking: Asking,
    /// What was asked before this, in the same conversation — a key or a
    /// finger left for the password says so.
    was: Option<Asking>,
    answerer: BarAnswerer,
    wrong: u8,
    /// The secret line the password is typed into.
    line: crate::line::Line,
    /// Answered, and waiting for sudo.
    ending: Option<Ending>,
    /// The control under a pointer.
    lit: Option<Control>,
    /// A KEY'S PASSPHRASE, not sudo: set when the bar was raised by the ssh
    /// askpass ([`Bar::ask`]). The answer goes to its `sink`, `deed` and
    /// `answerer` are a still-born placeholder the draw never shows, and the
    /// bar falls the moment it is answered.
    ask: Option<AskPass>,
}

impl Drop for Up {
    fn drop(&mut self) {
        self.line.text.zeroize();
    }
}

/// The bar: what is asked, when anything is.
#[derive(Default)]
pub struct Bar {
    up: Option<Up>,
}

impl std::fmt::Debug for Bar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bar")
            .field("up", &self.up.as_ref().map(|u| (u.id, u.asking, u.wrong)))
            .finish()
    }
}

/// What a message moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    /// Nothing.
    Nothing,
    /// The bar came up: the footer grows and takes the keyboard.
    Rose,
    /// What it says changed.
    Changed,
    /// It went: the footer shrinks and lets the keyboard go.
    Fell,
}

/// What a key or a click did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressed {
    /// Nothing changed.
    Nothing,
    /// What is drawn changed.
    Changed,
    /// The door reads the clipboard for the line ([`Bar::paste`]), for this
    /// conversation.
    Paste(u64),
    /// The ASKPASS bar answered or cancelled and went at once (no sudo verdict
    /// to wait for): the footer shrinks and gives the keyboard back.
    Fell,
}

/// The id under which the bar's line asks — never a question of the
/// minibuffer's, and never answered under it.
const LINE_ID: &str = "sudo-bar";

impl Bar {
    /// Whether it is up.
    #[must_use]
    pub const fn is_up(&self) -> bool {
        self.up.is_some()
    }

    /// How many lines it takes: [`ROWS`] while up, none at rest.
    #[must_use]
    pub const fn rows(&self) -> u16 {
        if self.up.is_some() { ROWS } else { 0 }
    }

    /// What a conversation said.
    pub fn heard(&mut self, to: ToBar) -> Moved {
        match to {
            ToBar::Asked {
                id,
                deed,
                asking,
                answerer,
                wrong,
            } => match self.up.as_mut() {
                Some(up) if up.id == id => {
                    if up.asking != asking {
                        up.was = Some(up.asking);
                    }
                    up.asking = asking;
                    up.answerer = answerer;
                    // A NEW ATTEMPT starts on a clean line, the old one wiped.
                    if wrong != up.wrong {
                        up.line.text.zeroize();
                        up.line.text.clear();
                        up.was = None;
                    }
                    up.wrong = wrong;
                    up.ending = None;
                    Moved::Changed
                }
                _ => {
                    let rose = self.up.is_none();
                    self.up = Some(Up {
                        id,
                        deed,
                        asking,
                        was: None,
                        answerer,
                        wrong,
                        line: crate::line::Line::new(LINE_ID, true, None),
                        ending: None,
                        lit: None,
                        ask: None,
                    });
                    if rose { Moved::Rose } else { Moved::Changed }
                }
            },
            ToBar::Over { id } => {
                if self.up.as_ref().is_some_and(|u| u.id == id) {
                    self.up = None;
                    Moved::Fell
                } else {
                    Moved::Nothing
                }
            }
        }
    }

    /// Raises the bar to ask a secret of `kind` for conversation `id`, what is
    /// typed going to `sink` — the askpass's socket, never the sudo door.
    /// `false` when a bar is already up: one is asked at a time, and the caller
    /// tells the asker so (it falls back to its own way of asking).
    #[must_use]
    pub fn ask(
        &mut self,
        id: u64,
        kind: Kind,
        context: Option<String>,
        prompt: String,
        sink: std::sync::mpsc::Sender<AskAnswer>,
    ) -> bool {
        if self.up.is_some() {
            return false;
        }
        self.up = Some(Up {
            id,
            // A PLACEHOLDER, never drawn and never answered: the askpass branch
            // of `drawn` shows the prompt instead of this deed, and `press`
            // sends what is typed to `ask.sink`, not this answerer.
            deed: Deed {
                asker: Asker::Security,
                window: false,
                verb: RootVerb::Relax {
                    span: RelaxSpan::Fifteen,
                },
                vault: None,
            },
            asking: Asking::Password,
            was: None,
            answerer: BarAnswerer {
                answerer: ronoctua_security::authority::answers().0,
                kept: Kept::default(),
            },
            wrong: 0,
            line: crate::line::Line::new(LINE_ID, true, None),
            ending: None,
            lit: None,
            ask: Some(AskPass {
                kind,
                prompt,
                context,
                sink,
            }),
        });
        true
    }

    /// Drops the askpass bar `id` if it is still up — the socket that asked
    /// went away. Letting the bar go lets its `sink` go with it, which the
    /// serving thread reads as a cancel.
    pub fn drop_ask(&mut self, id: u64) -> Moved {
        if self
            .up
            .as_ref()
            .is_some_and(|u| u.id == id && u.ask.is_some())
        {
            self.up = None;
            Moved::Fell
        } else {
            Moved::Nothing
        }
    }

    /// A key, while the bar is up (it holds the keyboard).
    pub fn key(&mut self, key: ronoctua_keymap::Key) -> Pressed {
        use ronoctua_keymap::Sym;
        let Some(up) = self.up.as_mut() else {
            return Pressed::Nothing;
        };
        // `Esc` and `C-g` cancel whatever stage it is at.
        let cancels = key.sym == Sym::Esc || (key.mods.ctrl && key.sym == Sym::Char('g'));
        if cancels {
            return self.press(Control::Cancel);
        }
        if up.ending.is_some() {
            return Pressed::Nothing;
        }
        if key.mods.ctrl && !key.mods.alt && key.sym == Sym::Char('p') {
            return self.press(Control::UsePassword);
        }
        match up.line.key(key) {
            crate::line::Step::Nothing | crate::line::Step::Cancel => Pressed::Nothing,
            crate::line::Step::Changed => Pressed::Changed,
            crate::line::Step::WantsPaste => Pressed::Paste(up.id),
            crate::line::Step::Answer(_, mut text) => {
                text.zeroize();
                self.press(Control::Answer)
            }
        }
    }

    /// A control pressed — by its key, or by a click where it is drawn.
    pub fn press(&mut self, control: Control) -> Pressed {
        // THE ASKPASS BAR: what is typed goes to the socket that asked, never
        // the sudo door, and the bar falls the moment it is answered — there
        // is no verdict to wait for. "Use the password" is not its control.
        if self.up.as_ref().is_some_and(|u| u.ask.is_some()) {
            match control {
                Control::UsePassword => return Pressed::Nothing,
                Control::Answer | Control::Cancel => {
                    let mut up = self.up.take().expect("a bar is up");
                    let answer = if control == Control::Answer {
                        AskAnswer::Password(Secret::new(std::mem::take(&mut up.line.text)))
                    } else {
                        up.line.text.zeroize();
                        up.line.text.clear();
                        AskAnswer::Cancel
                    };
                    if let Some(ask) = up.ask.take() {
                        let _ = ask.sink.send(answer);
                    }
                    return Pressed::Fell;
                }
            }
        }
        let Some(up) = self.up.as_mut() else {
            return Pressed::Nothing;
        };
        match control {
            Control::Cancel => {
                if up.ending == Some(Ending::Cancelling) {
                    return Pressed::Nothing;
                }
                up.line.text.zeroize();
                up.line.text.clear();
                up.answerer.answer(Answer::Cancel);
                up.ending = Some(Ending::Cancelling);
                Pressed::Changed
            }
            _ if up.ending.is_some() => Pressed::Nothing,
            Control::UsePassword => {
                if up.asking == Asking::Password {
                    return Pressed::Nothing;
                }
                up.answerer.answer(Answer::UsePassword);
                Pressed::Changed
            }
            Control::Answer => {
                if up.line.text.is_empty() {
                    // NOTHING TYPED while a key or a finger is awaited: the
                    // password asked, as "use the password" does.
                    return if up.asking == Asking::Password {
                        Pressed::Nothing
                    } else {
                        self.press(Control::UsePassword)
                    };
                }
                let secret = Secret::new(std::mem::take(&mut up.line.text));
                up.answerer.answer(Answer::Password(secret));
                up.ending = Some(Ending::Checking);
                Pressed::Changed
            }
        }
    }

    /// What was pasted into the line, for conversation `id` — nobody's when
    /// another is asked now.
    pub fn paste(&mut self, id: u64, text: &str) -> Pressed {
        match self.up.as_mut() {
            Some(up) if up.id == id && up.ending.is_none() => match up.line.paste(text) {
                crate::line::Step::Changed => Pressed::Changed,
                _ => Pressed::Nothing,
            },
            _ => Pressed::Nothing,
        }
    }

    /// The control a pointer rests on, lit; `true` when that changed.
    pub fn light(&mut self, control: Option<Control>) -> bool {
        match self.up.as_mut() {
            Some(up) if up.lit != control => {
                up.lit = control;
                true
            }
            _ => false,
        }
    }

    /// The bar, `columns` wide — two lines: the glyph, the title in capitals
    /// and who asks what; then what sudo waits for, the line typed into, and
    /// the controls at the edge. Empty at rest.
    #[must_use]
    pub fn drawn(&self, columns: u16, phase: bool, texts: &ronoctua_i18n::Texts) -> Vec<Piece> {
        let Some(up) = self.up.as_ref() else {
            return Vec::new();
        };
        // THE INK OF THE PHASE: the bar alternates between the alert's two
        // pairs on a clock (`crate::alert::roles`), so a passphrase or a touch
        // being awaited is unmissable — the one device invariant 15 leaves,
        // now spent on this bar too.
        let ink = crate::alert::roles(phase).1;
        let left = tokens::space::MODELINE_EDGELEFT;
        let mut out = Vec::new();
        let text = |row: u16, at: u16, text: String| Piece {
            row,
            at,
            text,
            color: ink,
            lit: false,
            control: None,
            explain: None,
            edge: false,
        };
        // A KEY'S PASSPHRASE is drawn on its own: the glyph and title, the ssh
        // prompt, and the line typed into with answer and cancel — no deed, no
        // touch, no "use the password".
        if let Some(ask) = up.ask.as_ref() {
            let right = tokens::space::MODELINE_EDGERIGHT;
            out.push(text(0, left, tokens::icon::SUDO_BAR.to_string()));
            let at = left + 2;
            let title = texts.t(ask.kind.title()).to_uppercase();
            out.push(text(0, at, title.clone()));
            let after = at + layout::width(&title) + 2;
            let room = columns.saturating_sub(right).saturating_sub(after);
            // THE CONTEXT WINS THE ROW when there is one — what sudo serves and
            // from where says more than its "[sudo] password for …" prompt. An
            // ssh ask has none, and shows its prompt (the key it is for).
            let said = ask.context.as_deref().unwrap_or(ask.prompt.as_str());
            if room > 0 && !said.is_empty() {
                out.push(text(0, after, layout::elide_end(said, room)));
            }
            // THE CONTROLS FIRST, from the edge, because they decide the room.
            let controls = [
                (
                    Control::Answer,
                    ANSWER_KEY,
                    key::SUDO_BAR_ANSWER,
                    key::ABOUT_SUDO_BAR_ANSWER,
                ),
                (
                    Control::Cancel,
                    CANCEL_KEY,
                    key::SUDO_BAR_CANCEL,
                    key::ABOUT_SUDO_BAR_CANCEL,
                ),
            ];
            let mut edge = 0u16;
            let mut placed = Vec::new();
            for (control, notation, label, about) in controls.iter().rev() {
                let shown = format!(" {notation} {} ", texts.t(label));
                let width = layout::width(&shown);
                placed.push(Piece {
                    row: 1,
                    at: crate::chrome::from_the_edge(columns, edge + width),
                    text: shown,
                    color: ink,
                    lit: up.lit == Some(*control),
                    control: Some(*control),
                    explain: Some(texts.t(about)),
                    edge: true,
                });
                edge += width + 1;
            }
            let first = placed.iter().map(|p| p.at).min().unwrap_or(columns);
            // A TOUCH shows no line — there is nothing to type, and Answer
            // alone confirms it; every other kind shows what is typed.
            let asked = if ask.kind.typed() {
                format!("{}: {}", texts.t(ask.kind.ask()), up.line.shown())
            } else {
                texts.t(ask.kind.ask())
            };
            out.push(text(
                1,
                left,
                layout::elide_start(&asked, first.saturating_sub(left + 1)),
            ));
            out.extend(placed);
            return out;
        }
        // THE FIRST LINE: the glyph in the margin, a cell of air, the title in
        // the language's capitals, two cells, and who asks what. WHO ASKS WHAT
        // WINS THE ROOM: on a line too narrow for both, the title gives way
        // (the glyph and the bar's colour still say what this is).
        let room = columns.saturating_sub(left + tokens::space::MODELINE_EDGERIGHT);
        out.push(text(0, left, tokens::icon::SUDO_BAR.to_string()));
        let at = left + 2;
        let sentence = up.deed.sentence(texts);
        let title = texts.t(key::SUDO_BAR_TITLE).to_uppercase();
        let both = layout::width(&title) + 2 + layout::width(&sentence);
        let after = if both <= room.saturating_sub(2) {
            out.push(text(0, at, title.clone()));
            at + layout::width(&title) + 2
        } else {
            at
        };
        let rest = (left + room).saturating_sub(after);
        if rest > 0 {
            out.push(text(0, after, layout::elide_end(&sentence, rest)));
        }
        // THE CONTROLS FIRST, at the edge, because they decide the room — the
        // band's own rule: laying the words out first is how a line spills.
        let controls: Vec<(Control, &str, &str, &str)> = match (up.ending, up.asking) {
            (Some(_), _) => vec![(
                Control::Cancel,
                CANCEL_KEY,
                key::SUDO_BAR_CANCEL,
                key::ABOUT_SUDO_BAR_CANCEL,
            )],
            (None, Asking::Password) => vec![
                (
                    Control::Answer,
                    ANSWER_KEY,
                    key::SUDO_BAR_ANSWER,
                    key::ABOUT_SUDO_BAR_ANSWER,
                ),
                (
                    Control::Cancel,
                    CANCEL_KEY,
                    key::SUDO_BAR_CANCEL,
                    key::ABOUT_SUDO_BAR_CANCEL,
                ),
            ],
            (None, Asking::Touch | Asking::Finger) => vec![
                (
                    Control::UsePassword,
                    USE_PASSWORD_KEY,
                    key::SUDO_BAR_USE_PASSWORD,
                    key::ABOUT_SUDO_BAR_USE_PASSWORD,
                ),
                (
                    Control::Answer,
                    ANSWER_KEY,
                    key::SUDO_BAR_ANSWER,
                    key::ABOUT_SUDO_BAR_ANSWER,
                ),
                (
                    Control::Cancel,
                    CANCEL_KEY,
                    key::SUDO_BAR_CANCEL,
                    key::ABOUT_SUDO_BAR_CANCEL,
                ),
            ],
        };
        // EACH CONTROL its key and its word — or, on a line that would cut
        // what sudo asks to make room for them, its key alone: the word is
        // still said under the pointer.
        let place = |words: bool| -> Vec<Piece> {
            let mut edge = 0u16;
            let mut placed = Vec::new();
            for (control, notation, label, about) in controls.iter().rev() {
                let shown = if words {
                    format!(" {notation} {} ", texts.t(label))
                } else {
                    format!(" {notation} ")
                };
                let width = layout::width(&shown);
                let at = crate::chrome::from_the_edge(columns, edge + width);
                placed.push(Piece {
                    row: 1,
                    at,
                    text: shown,
                    color: ink,
                    lit: up.lit == Some(*control),
                    control: Some(*control),
                    explain: Some(texts.t(about)),
                    edge: true,
                });
                edge += width + 1;
            }
            placed
        };
        let first_of = |placed: &[Piece]| placed.iter().map(|p| p.at).min().unwrap_or(columns);
        // THE SECOND LINE: what sudo waits for, and the line typed into.
        let asked = match up.ending {
            Some(Ending::Checking) => texts.t(key::SUDO_BAR_CHECKING),
            Some(Ending::Cancelling) => texts.t(key::SUDO_BAR_CANCELLING),
            None => {
                let what = match (up.asking, up.was) {
                    (Asking::Touch, _) => texts.t(key::SUDO_BAR_TOUCH),
                    (Asking::Finger, _) => texts.t(key::SUDO_BAR_FINGER),
                    (Asking::Password, Some(Asking::Touch | Asking::Finger)) => {
                        texts.t(key::SUDO_BAR_PASSWORD_INSTEAD)
                    }
                    (Asking::Password, _) => texts.t(key::SUDO_BAR_PASSWORD),
                };
                let what = if up.wrong > 0 {
                    texts.t_with(key::SUDO_BAR_WRONG, &[("asked", &what)])
                } else {
                    what
                };
                format!("{what}: {}", up.line.shown())
            }
        };
        let mut placed = place(true);
        if layout::width(&asked) > first_of(&placed).saturating_sub(left + 1) {
            placed = place(false);
        }
        let room = first_of(&placed).saturating_sub(left + 1);
        // WHAT IS TYPED KEEPS ITS PLACE: a sentence too long for the line is
        // cut at its START, so the dots a key just added stay in sight.
        out.push(text(1, left, layout::elide_start(&asked, room)));
        out.extend(placed);
        out
    }

    /// A photograph's bar: `shot`, asked by a deed of the camera's — never
    /// a conversation, and nothing it answers goes anywhere.
    pub fn photographed(&mut self, shot: Shot) {
        let (answerer, _answers) = ronoctua_security::authority::answers();
        let bar = BarAnswerer {
            answerer,
            kept: Kept::default(),
        };
        let deed = match shot {
            Shot::Window => Deed {
                asker: Asker::Files,
                window: true,
                verb: RootVerb::FilesRoot,
                vault: None,
            },
            _ => Deed {
                asker: Asker::Security,
                window: false,
                verb: RootVerb::Relax {
                    span: RelaxSpan::Fifteen,
                },
                vault: None,
            },
        };
        let (asking, was, wrong, typed, ending) = match shot {
            Shot::Password | Shot::Window => (Asking::Password, None, 0, "secret", None),
            Shot::Touch => (Asking::Touch, None, 0, "", None),
            Shot::Finger => (Asking::Finger, None, 0, "", None),
            Shot::UsePassword => (Asking::Password, Some(Asking::Touch), 0, "sec", None),
            Shot::Wrong => (Asking::Password, None, 1, "", None),
            Shot::Checking => (Asking::Password, None, 0, "", Some(Ending::Checking)),
        };
        let mut line = crate::line::Line::new(LINE_ID, true, None);
        line.text.push_str(typed);
        self.up = Some(Up {
            id: 0,
            deed,
            asking,
            was,
            answerer: bar,
            wrong,
            line,
            ending,
            lit: None,
            ask: None,
        });
    }
}

/// A state of the bar a photograph shows (`photograph --door footer --sudo
/// <shot>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shot {
    /// The password asked, something typed.
    Password,
    /// A touch of a security key.
    Touch,
    /// A finger on the reader.
    Finger,
    /// The password asked after "use the password".
    UsePassword,
    /// A wrong password, asked again.
    Wrong,
    /// Answered, waiting for sudo.
    Checking,
    /// Asked from a window.
    Window,
}

impl Shot {
    /// Every one, by its word.
    pub const ALL: [(Self, &'static str); 7] = [
        (Self::Password, "password"),
        (Self::Touch, "touch"),
        (Self::Finger, "finger"),
        (Self::UsePassword, "use-password"),
        (Self::Wrong, "wrong"),
        (Self::Checking, "checking"),
        (Self::Window, "window"),
    ];

    /// The shot a word names.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(_, w)| *w == word)
            .map(|(shot, _)| *shot)
    }
}

/// One drawn piece of the bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// Its line of the bar, from the top (`0` or `1`).
    pub row: u16,
    /// Its first cell (counted from the left even when placed from the edge).
    pub at: u16,
    /// What is drawn.
    pub text: String,
    /// Its ink.
    pub color: tokens::Color,
    /// A control under a pointer: drawn on the other alert pair, a chip.
    pub lit: bool,
    /// What a click does.
    pub control: Option<Control>,
    /// What the message line says while a pointer rests here.
    pub explain: Option<String>,
    /// Placed from the surface's right edge ([`crate::chrome::from_the_edge`]).
    pub edge: bool,
}

/// The piece at `row`, `column` of what was drawn — an edge-placed one
/// against the column counted from the border (`from_the_edge`), as it was
/// placed. Pure.
#[must_use]
pub fn at(drawn: &[Piece], row: u16, column: u16, from_the_edge: Option<u16>) -> Option<&Piece> {
    drawn.iter().find(|p| {
        let at = if p.edge { from_the_edge } else { Some(column) };
        p.row == row && at.is_some_and(|c| c >= p.at && c < p.at + layout::width(&p.text))
    })
}

/// The bar's ground and a lit control's: `(ground, chip ground, chip ink)`.
/// The ground is the alert role of `phase`, and a lit control's chip the other
/// — the two alternate on the clock, so the bar flashes (`crate::alert::roles`).
#[must_use]
pub const fn grounds(phase: bool) -> (tokens::Color, tokens::Color, tokens::Color) {
    let (ground, _) = crate::alert::roles(phase);
    let (chip, chip_ink) = crate::alert::roles(!phase);
    (ground, chip, chip_ink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sudo::order_line;
    use ronoctua_security::verb::Fingers;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    fn deed(verb: RootVerb) -> Deed {
        Deed::of(Asker::Security, false, verb.into())
    }

    /// Every verb the helper knows, with the words it carries.
    fn every_verb() -> Vec<RootVerb> {
        use ronoctua_security::fprint::Finger;
        let mut verbs = vec![
            RootVerb::RelaxEnd,
            RootVerb::FirewallApply,
            RootVerb::FingerDelete {
                which: Fingers::All,
            },
            RootVerb::FingerDelete {
                which: Fingers::One(Finger::LeftThumb),
            },
            RootVerb::FingerEnroll {
                finger: Finger::RightIndex,
            },
            RootVerb::FingerSudo { on: true },
            RootVerb::FingerSudo { on: false },
            RootVerb::FilesRoot,
            RootVerb::PrivateRoot,
            RootVerb::StickWrite,
            RootVerb::PersonModuleApprove {
                id: "my-tea".to_owned(),
                hash: "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=".to_owned(),
            },
            RootVerb::U2fKeys {
                credentials: Vec::new(),
            },
            RootVerb::U2fKeys {
                credentials: vec!["owBYtXj1Pq+Zb5c1Kd/AbQ==,04d6f1aa,es256,+presence".to_owned()],
            },
            RootVerb::PersonSignerPin {
                key: format!(
                    "{}{}",
                    ronoctua_person::local::KEY_PREFIX,
                    "A".repeat(ronoctua_person::local::KEY_BASE64_CHARS)
                ),
            },
            RootVerb::BinaryCacheNetrc {
                name: "someone".to_owned(),
            },
            RootVerb::BinaryCacheNetrcRemove,
            RootVerb::BuildSwitchReference,
        ];
        verbs.extend(RelaxSpan::ALL.map(|span| RootVerb::Relax { span }));
        verbs
    }

    /// FAILS IF: a screen or a gesture has no words in a language, a window
    /// is said as the screen, or a deed's sentence leaves out who asks or
    /// what for.
    #[test]
    fn every_deed_says_who_asks_and_what_for_in_every_language() {
        use ronoctua_private::Op;
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for asker in Asker::ALL {
                for window in [false, true] {
                    let name = t.t(asker.key(window));
                    assert!(!name.starts_with('['), "{language}: {asker:?} {window}");
                }
                assert_ne!(t.t(asker.key(false)), t.t(asker.key(true)));
            }
            for verb in every_verb() {
                let said = deed(verb.clone()).sentence(&t);
                assert!(!said.contains('['), "{language}: {verb:?}: {said}");
                assert!(said.contains(&t.t(Asker::Security.key(false))), "{said}");
            }
            for op in [
                Op::Create { cap_bytes: 1 << 30 },
                Op::Open,
                Op::Close,
                Op::EnrollFido2,
                Op::ForgetFido2,
                Op::Passphrase,
                Op::Grow { cap_bytes: 1 << 31 },
            ] {
                let d = Deed {
                    vault: Some(op),
                    ..deed(RootVerb::PrivateRoot)
                };
                let said = d.sentence(&t);
                assert!(!said.contains('['), "{language}: {op:?}: {said}");
            }
            for key in [
                key::SUDO_BAR_TITLE,
                key::SUDO_BAR_PASSWORD,
                key::SUDO_BAR_PASSWORD_INSTEAD,
                key::SUDO_BAR_TOUCH,
                key::SUDO_BAR_FINGER,
                key::SUDO_BAR_CHECKING,
                key::SUDO_BAR_CANCELLING,
                key::SUDO_BAR_ANSWER,
                key::SUDO_BAR_USE_PASSWORD,
                key::SUDO_BAR_CANCEL,
                key::ABOUT_SUDO_BAR_ANSWER,
                key::ABOUT_SUDO_BAR_USE_PASSWORD,
                key::ABOUT_SUDO_BAR_CANCEL,
            ] {
                assert!(!t.t(key).starts_with('['), "{language}: {key}");
            }
            assert!(
                !t.t_with(key::SUDO_BAR_WRONG, &[("asked", "x")])
                    .starts_with('[')
            );
        }
    }

    /// FAILS IF: a deed whose verb carries words the helper would refuse —
    /// a credential that could end a line, an approval of a module that is
    /// not the person's — or a vault operation on another verb is taken, a
    /// window's deed is said as the screen's, or a field the deed does not
    /// have reads.
    #[test]
    fn a_windows_deed_is_read_as_the_helper_reads_it() {
        for verb in every_verb() {
            let d = deed(verb);
            assert!(d.valid(), "{d:?}");
            let line = order_line(&d).unwrap();
            assert!(line.ends_with('\n') && line.matches('\n').count() == 1);
            let json = line.trim().strip_prefix(ORDER).unwrap().trim();
            let back = deed_of(json).unwrap();
            assert!(back.window, "a window's deed is a window's");
            assert_eq!(
                Deed {
                    window: false,
                    ..back
                },
                d
            );
        }
        assert!(
            !order_line(&deed(RootVerb::FilesRoot))
                .unwrap()
                .contains("window"),
            "never on the wire"
        );
        for bad in [
            r#"{"asker":"security","verb":{"verb":"u2f-keys","credentials":["a:b,c"]}}"#,
            r#"{"asker":"security","verb":{"verb":"person-module-approve","id":"tea","hash":"x"}}"#,
            r#"{"asker":"security","verb":{"verb":"relax-end"},"vault":{"op":"open"}}"#,
            r#"{"asker":"security","verb":{"verb":"relax-end"},"window":false}"#,
            r#"{"asker":"terminal","verb":{"verb":"relax-end"}}"#,
            r#"{"asker":"security","verb":{"verb":"sh","c":"id"}}"#,
            "not json",
        ] {
            assert_eq!(deed_of(bad), None, "{bad}");
        }
        assert!(
            deed_of(r#"{"asker":"files","verb":{"verb":"private-root"},"vault":{"op":"open"}}"#)
                .is_some()
        );
    }

    /// The messages a conversation told the bar.
    fn told() -> (Arc<Mutex<Vec<String>>>, impl Fn(ToBar)) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let to = Arc::clone(&log);
        (log, move |m: ToBar| {
            let said = match m {
                ToBar::Asked { asking, wrong, .. } => format!("asked {asking:?} {wrong}"),
                ToBar::Over { .. } => "over".to_owned(),
            };
            to.lock().unwrap().push(said);
        })
    }

    /// FAILS IF: a wrong password is not asked again, is asked more than
    /// sudo's tries, is said as anything but wrong after the last; the bar is
    /// not told the conversation is over — once, at the end, however it
    /// ended — or a cancel is tried again.
    #[test]
    fn a_wrong_password_is_asked_again_and_the_bar_goes_at_the_end() {
        let (log, tell) = told();
        let mut attempts = 0;
        let mut script = |_: &RootVerb,
                          _: ronoctua_security::authority::Answers,
                          _: &Answerer,
                          asked: &mut dyn FnMut(Asking),
                          _: &mut dyn FnMut(&str)| {
            attempts += 1;
            asked(Asking::Password);
            Err(Why::WrongPassword)
        };
        let result = conversation(
            &deed(RootVerb::FilesRoot),
            &tell,
            &Kept::default(),
            &Cancel::default(),
            &mut script,
            &mut |_| {},
        );
        assert_eq!(result, Err(Why::WrongPassword));
        assert_eq!(attempts, usize::from(ATTEMPTS));
        assert_eq!(
            *log.lock().unwrap(),
            [
                "asked Password 0",
                "asked Password 1",
                "asked Password 2",
                "over"
            ]
        );
        // RIGHT THE SECOND TIME.
        let (log, tell) = told();
        let mut n = 0;
        let mut script = |_: &RootVerb,
                          _: ronoctua_security::authority::Answers,
                          _: &Answerer,
                          asked: &mut dyn FnMut(Asking),
                          _: &mut dyn FnMut(&str)| {
            n += 1;
            asked(Asking::Touch);
            if n == 1 {
                Err(Why::WrongPassword)
            } else {
                Ok(())
            }
        };
        let ok = conversation(
            &deed(RootVerb::FilesRoot),
            &tell,
            &Kept::default(),
            &Cancel::default(),
            &mut script,
            &mut |_| {},
        );
        assert_eq!(ok, Ok(()));
        assert_eq!(
            *log.lock().unwrap(),
            ["asked Touch 0", "asked Touch 1", "over"]
        );
        // A CANCEL IS ONCE.
        let (log, tell) = told();
        let mut script = |_: &RootVerb,
                          _: ronoctua_security::authority::Answers,
                          _: &Answerer,
                          _: &mut dyn FnMut(Asking),
                          _: &mut dyn FnMut(&str)| Err(Why::Cancelled);
        let cancelled = conversation(
            &deed(RootVerb::FilesRoot),
            &tell,
            &Kept::default(),
            &Cancel::default(),
            &mut script,
            &mut |_| {},
        );
        assert_eq!(cancelled, Err(Why::Cancelled));
        assert_eq!(*log.lock().unwrap(), ["over"]);
    }

    /// FAILS IF: the bar is raised for a question the gesture's password
    /// already answered, a wrong password is kept for the next verb, or a
    /// conversation whose window went away asks anything.
    #[test]
    fn the_gestures_password_answers_its_next_verbs_and_a_gone_window_asks_nothing() {
        let kept = Kept::default();
        kept.set(Secret::new("correct horse"));
        let (log, tell) = told();
        let mut script = |_: &RootVerb,
                          _: ronoctua_security::authority::Answers,
                          _: &Answerer,
                          asked: &mut dyn FnMut(Asking),
                          _: &mut dyn FnMut(&str)| {
            asked(Asking::Touch);
            Ok(())
        };
        conversation(
            &deed(RootVerb::FilesRoot),
            &tell,
            &kept,
            &Cancel::default(),
            &mut script,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(*log.lock().unwrap(), ["over"], "the bar was raised");
        let mut script = |_: &RootVerb,
                          _: ronoctua_security::authority::Answers,
                          _: &Answerer,
                          _: &mut dyn FnMut(Asking),
                          _: &mut dyn FnMut(&str)| Err(Why::WrongPassword);
        let _ = conversation(
            &deed(RootVerb::FilesRoot),
            &|_| {},
            &kept,
            &Cancel::default(),
            &mut script,
            &mut |_| {},
        );
        assert!(kept.get().is_none(), "a wrong password was kept");
        // THE WINDOW WENT AWAY before its turn: nothing is asked.
        let cancel = Cancel::default();
        cancel.cancel();
        let (log, tell) = told();
        let mut ran = false;
        let mut script = |_: &RootVerb,
                          _: ronoctua_security::authority::Answers,
                          _: &Answerer,
                          _: &mut dyn FnMut(Asking),
                          _: &mut dyn FnMut(&str)| {
            ran = true;
            Ok(())
        };
        let gone = conversation(
            &deed(RootVerb::FilesRoot),
            &tell,
            &Kept::default(),
            &cancel,
            &mut script,
            &mut |_| {},
        );
        assert_eq!(gone, Err(Why::Cancelled));
        assert!(!ran);
        assert_eq!(*log.lock().unwrap(), ["over"]);
    }

    fn asked(bar: &mut Bar, id: u64, asking: Asking, wrong: u8) -> Moved {
        let (answerer, _answers) = ronoctua_security::authority::answers();
        bar.heard(ToBar::Asked {
            id,
            deed: deed(RootVerb::FilesRoot),
            asking,
            answerer: BarAnswerer {
                answerer,
                kept: Kept::default(),
            },
            wrong,
        })
    }

    fn key(notation: &str) -> ronoctua_keymap::Key {
        ronoctua_keymap::Key::parse(notation).unwrap()
    }

    /// FAILS IF: the bar does not come up with the first question, take its
    /// rows, change with the next and go with the end of ITS conversation
    /// only; or what is typed is drawn as anything but dots, is still in the
    /// line after it was answered, or a key reaches the line while sudo
    /// checks.
    #[test]
    fn the_bar_rises_masks_and_falls_with_its_conversation() {
        let mut bar = Bar::default();
        assert_eq!(bar.rows(), 0);
        assert_eq!(asked(&mut bar, 7, Asking::Touch, 0), Moved::Rose);
        assert_eq!(bar.rows(), ROWS);
        assert_eq!(asked(&mut bar, 7, Asking::Password, 0), Moved::Changed);
        for c in ["h", "u", "n", "t", "e", "r"] {
            assert_eq!(bar.key(key(c)), Pressed::Changed);
        }
        let t = texts("en-GB");
        let drawn = bar.drawn(120, true, &t);
        let words: String = drawn.iter().map(|p| p.text.clone()).collect();
        assert!(!words.contains("hunter"), "{words}");
        assert!(words.contains("••••••"), "{words}");
        assert!(
            words.contains(&t.t(key::SUDO_BAR_PASSWORD_INSTEAD)),
            "a key left for the password says so: {words}"
        );
        assert_eq!(bar.key(key("RET")), Pressed::Changed);
        let up = bar.up.as_ref().unwrap();
        assert!(up.line.text.is_empty(), "the answer stayed in the line");
        assert_eq!(up.ending, Some(Ending::Checking));
        assert_eq!(bar.key(key("x")), Pressed::Nothing, "typed while it checks");
        assert_eq!(bar.heard(ToBar::Over { id: 8 }), Moved::Nothing);
        assert_eq!(bar.heard(ToBar::Over { id: 7 }), Moved::Fell);
        assert_eq!(bar.rows(), 0);
        assert!(bar.drawn(120, true, &t).is_empty());
    }

    /// FAILS IF: `Esc` does not cancel at any stage, "use the password" is
    /// offered or works while the password is asked, `RET` with nothing typed
    /// does not leave a key for the password, or a paste for another
    /// conversation lands in this one's line.
    #[test]
    fn the_controls_do_what_they_say() {
        let mut bar = Bar::default();
        asked(&mut bar, 1, Asking::Finger, 0);
        let t = texts("pt-BR");
        let controls = |bar: &Bar| -> Vec<Control> {
            bar.drawn(120, true, &t)
                .iter()
                .filter_map(|p| p.control)
                .collect()
        };
        assert_eq!(
            controls(&bar),
            [Control::Cancel, Control::Answer, Control::UsePassword]
        );
        assert_eq!(bar.key(key("C-p")), Pressed::Changed);
        assert_eq!(
            bar.key(key("RET")),
            Pressed::Changed,
            "nothing typed: the password"
        );
        asked(&mut bar, 1, Asking::Password, 0);
        assert_eq!(controls(&bar), [Control::Cancel, Control::Answer]);
        assert_eq!(bar.press(Control::UsePassword), Pressed::Nothing);
        assert_eq!(
            bar.key(key("RET")),
            Pressed::Nothing,
            "nothing to answer with"
        );
        assert_eq!(bar.paste(2, "other"), Pressed::Nothing);
        assert_eq!(bar.paste(1, "pasted"), Pressed::Changed);
        assert_eq!(bar.up.as_ref().unwrap().line.text, "pasted");
        assert_eq!(bar.key(key("ESC")), Pressed::Changed);
        assert_eq!(bar.up.as_ref().unwrap().ending, Some(Ending::Cancelling));
        assert!(bar.up.as_ref().unwrap().line.text.is_empty());
        assert_eq!(controls(&bar), [Control::Cancel]);
        assert_eq!(bar.key(key("C-g")), Pressed::Nothing, "cancelled once");
    }

    /// FAILS IF: a wrong password's new attempt keeps what was typed, or
    /// does not say it was wrong.
    #[test]
    fn a_new_attempt_starts_clean_and_says_why() {
        let mut bar = Bar::default();
        asked(&mut bar, 3, Asking::Password, 0);
        bar.key(key("a"));
        assert_eq!(asked(&mut bar, 3, Asking::Password, 1), Moved::Changed);
        assert!(bar.up.as_ref().unwrap().line.text.is_empty());
        let t = texts("en-GB");
        let wrong = t.t_with(
            key::SUDO_BAR_WRONG,
            &[("asked", &t.t(key::SUDO_BAR_PASSWORD))],
        );
        let words: String = bar
            .drawn(160, true, &t)
            .iter()
            .map(|p| p.text.clone())
            .collect();
        assert!(words.contains(&wrong), "{words}");
    }

    /// FAILS IF: the askpass bar does not rise, its draw omits the title or
    /// masks nothing, the KIND does not pick the title (an SSH key vs the
    /// system password), a second ask is taken while one is up, the typed
    /// secret does not reach the socket with the bar falling, a cancel sends
    /// anything but a cancel, or a dropped socket does not take the bar.
    #[test]
    fn the_askpass_bar_asks_a_secret_by_kind_and_hands_it_to_the_socket() {
        let t = texts("en-GB");
        let ssh_title = t.t(key::ASKPASS_BAR_TITLE_SSH).to_uppercase();
        let sudo_title = t.t(key::ASKPASS_BAR_TITLE_SUDO).to_uppercase();
        let shown = |bar: &Bar| -> String {
            bar.drawn(120, true, &t)
                .iter()
                .map(|p| p.text.clone())
                .collect()
        };
        let mut bar = Bar::default();
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(bar.ask(
            1,
            Kind::Ssh,
            None,
            "Enter passphrase for key 'id_x'".to_owned(),
            tx
        ));
        assert_eq!(bar.rows(), ROWS);
        // A SECOND ASK is refused while one is up — the caller tells the asker so.
        let (tx2, _rx2) = std::sync::mpsc::channel();
        assert!(!bar.ask(2, Kind::Ssh, None, "other".to_owned(), tx2));
        for c in ["h", "u", "n", "t"] {
            assert_eq!(bar.key(key(c)), Pressed::Changed);
        }
        // THE SSH KIND wears the SSH-key title, not the system one.
        let words = shown(&bar);
        assert!(words.contains(&ssh_title), "no ssh title: {words}");
        assert!(
            !words.contains(&sudo_title),
            "sudo title on an ssh ask: {words}"
        );
        assert!(words.contains("••••"), "not masked: {words}");
        assert!(!words.contains("hunt"), "typed in the clear: {words}");
        // RET hands the secret to the socket and the bar falls.
        assert_eq!(bar.key(key("RET")), Pressed::Fell);
        assert_eq!(bar.rows(), 0);
        assert!(matches!(rx.try_recv(), Ok(AskAnswer::Password(s)) if s.expose() == "hunt"));
        // THE SUDO KIND wears the system-password title and its label, and the
        // CONTEXT (what sudo serves and from where) is drawn in the prompt's
        // place — the "[sudo] password for …" prompt is not.
        let (tx, _rx) = std::sync::mpsc::channel();
        assert!(bar.ask(
            2,
            Kind::Sudo,
            Some("nixos-rebuild switch  —  zsh · pts/3".to_owned()),
            "[sudo] password for noctua:".to_owned(),
            tx
        ));
        let words = shown(&bar);
        assert!(words.contains(&sudo_title), "no sudo title: {words}");
        assert!(
            words.contains(&t.t(key::ASKPASS_BAR_ASK_SUDO)),
            "no sudo label: {words}"
        );
        assert!(
            !words.contains(&ssh_title),
            "ssh title on a sudo ask: {words}"
        );
        assert!(
            words.contains("nixos-rebuild switch"),
            "no sudo context drawn: {words}"
        );
        assert!(
            !words.contains("[sudo] password"),
            "the prompt drawn over the context: {words}"
        );
        assert_eq!(bar.key(key("ESC")), Pressed::Fell);
        // ESC sends a cancel.
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(bar.ask(3, Kind::Ssh, None, String::new(), tx));
        assert_eq!(bar.key(key("ESC")), Pressed::Fell);
        assert!(matches!(rx.try_recv(), Ok(AskAnswer::Cancel)));
        // THE SOCKET GONE drops the bar, and its sink with it.
        let (tx, rx) = std::sync::mpsc::channel::<AskAnswer>();
        assert!(bar.ask(4, Kind::Ssh, None, String::new(), tx));
        assert_eq!(bar.drop_ask(99), Moved::Nothing);
        assert_eq!(bar.drop_ask(4), Moved::Fell);
        assert!(rx.try_recv().is_err(), "the sink outlived the bar");
    }

    /// FAILS IF: a token is read as the wrong kind, a non-passphrase ask is
    /// titled a passphrase, or a touch draws a secret line instead of a bar a
    /// touch alone confirms.
    #[test]
    fn the_kind_is_read_from_the_token_and_a_touch_carries_no_line() {
        assert_eq!(Kind::of("system-password", true), Kind::Sudo);
        assert_eq!(Kind::of("system-password", false), Kind::Sudo);
        assert_eq!(Kind::of("ssh-passphrase", false), Kind::Ssh);
        assert_eq!(Kind::of("ssh-pin", false), Kind::Pin);
        assert_eq!(Kind::of("ssh-password", false), Kind::LoginPassword);
        assert_eq!(Kind::of("ssh-touch", false), Kind::Touch);
        assert_eq!(Kind::of("ssh-host", false), Kind::HostAuthenticity);
        assert_eq!(Kind::of("ssh", false), Kind::SshOther);
        assert_eq!(Kind::of("anything-else", false), Kind::SshOther);
        // A KNOWN TOKEN DECIDES, context or not.
        assert_eq!(Kind::of("ssh-passphrase", true), Kind::Ssh);
        assert_eq!(Kind::of("ssh", true), Kind::SshOther);
        // AN OLDER HELPER'S SYSTEM PASSWORD: a token this footer does not know,
        // with the context only the system password carries.
        assert_eq!(Kind::of("anything-else", true), Kind::Sudo);

        let t = texts("en-GB");
        let shown = |bar: &Bar| -> String {
            bar.drawn(120, true, &t)
                .iter()
                .map(|p| p.text.clone())
                .collect()
        };
        // A TOUCH wears its own title, draws NO secret line, and Answer alone
        // (an empty secret to the socket) confirms it.
        let mut bar = Bar::default();
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(bar.ask(
            1,
            Kind::Touch,
            None,
            "Confirm user presence for key ED25519-SK SHA256:x".to_owned(),
            tx
        ));
        let words = shown(&bar);
        assert!(
            words.contains(&t.t(key::ASKPASS_BAR_TITLE_TOUCH).to_uppercase()),
            "no touch title: {words}"
        );
        assert!(
            !words.contains(&t.t(key::ASKPASS_BAR_TITLE_SSH).to_uppercase()),
            "a touch titled a passphrase: {words}"
        );
        assert!(
            !words.contains("••••"),
            "a touch drew a secret line: {words}"
        );
        assert_eq!(bar.key(key("RET")), Pressed::Fell);
        assert!(matches!(rx.try_recv(), Ok(AskAnswer::Password(s)) if s.expose().is_empty()));
    }

    /// FAILS IF: a piece runs into another or past the edge, a control is
    /// found anywhere but where it is drawn — counted from the border, as it
    /// is placed — or the two lines are not the bar's two.
    #[test]
    fn a_control_is_grabbed_where_it_is_drawn() {
        let t = texts("pt-BR");
        for shot in Shot::ALL.map(|(s, _)| s) {
            let mut bar = Bar::default();
            bar.photographed(shot);
            for columns in [60u16, 100, 200] {
                let drawn = bar.drawn(columns, true, &t);
                assert!(drawn.iter().all(|p| p.row < ROWS));
                for p in &drawn {
                    assert!(
                        p.at + layout::width(&p.text) <= columns,
                        "{shot:?} at {columns}: `{}` leaks",
                        p.text
                    );
                    for q in drawn.iter().filter(|q| q.row == p.row && *q != p) {
                        let (a, b) = (p.at, p.at + layout::width(&p.text));
                        let (c, d) = (q.at, q.at + layout::width(&q.text));
                        assert!(b <= c || d <= a, "{shot:?} at {columns}: overlap");
                    }
                }
                for p in drawn.iter().filter(|p| p.control.is_some()) {
                    // FROM THE BORDER: the column counted back from the edge.
                    let found = at(&drawn, p.row, 0, Some(p.at)).and_then(|q| q.control);
                    assert_eq!(found, p.control, "{shot:?}");
                    assert!(p.edge);
                    assert!(p.explain.is_some());
                }
            }
        }
    }
}
