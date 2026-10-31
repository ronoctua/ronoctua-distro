//! The leader, the footer's own keys and the control channel — section 7.4.
//!
//! # How the leader gets here
//!
//! At rest the footer **does not capture the keyboard** (`keyboard-interactivity:
//! none`). That is not a detail: if it did, no buffer would receive a key.
//! Therefore the footer itself **cannot see** the leader's key.
//!
//! What sees it is the compositor. `nix/modules/scroll.nix` binds F13 by code (191) — the
//! key keyd emits when Super is TAPPED — to `exec ronoctua-shell leader`, and
//! that momentary process tells the daemon over a unix socket. Only then does the
//! footer ask for the keyboard in `exclusive` mode, and from there it reads the keys
//! itself until `ESC`.
//!
//! The path looks indirect, and it is — but it is the only one that respects at the same
//! time invariant 3 (IPC and layer-shell only) and the rule of not stealing the
//! keyboard at rest.
//!
//! # The socket
//!
//! It lives in `$XDG_RUNTIME_DIR`, with the display's name in the file. It is what
//! allows two sessions (the actual one and a nested `nix run .#dev`) on the same
//! machine without one sending commands to the other.
//!
//! It stopped understanding a single word: it speaks **command**
//! (`command <id> [argument]`), and it is through it that the emergency binds and
//! anything addressable by name come in (section 13). It is the
//! IPC surface section 13 asks for, in its minimal form.

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;

use crate::bindings::{Contribution, Generated, Set, Trees};
use crate::ipc::{socket_path, tone_of};
use crate::key;
use ronoctua_cmd::Command;
use ronoctua_keymap::Tree;
use smithay_client_toolkit::reexports::calloop::channel::Sender;

/// An order coming from the control socket.
///
/// # Why there are only two
///
/// There used to be one variant per feature (`History`, `Dispensar`), and the
/// socket spoke a vocabulary of its own, parallel to the commands'. Two
/// words for the same thing is the pattern this work has been undoing:
/// now the socket **speaks command**, by id, and anything addressable by
/// name arrives through here with no new variant (§13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    /// Activate the leader. It is not a command: it is the sequence's trigger, and what
    /// sends it is the compositor.
    Leader,
    /// Run a command by id, with an optional argument.
    Command(String, Option<String>),
    /// Exit, because another footer has taken this display. See [`yield_the_place`].
    ///
    /// It is not a command, and it has no key: it is not something one asks for, and it is the
    /// only thing on the socket that a footer sends to ANOTHER footer.
    Quit,
    /// A buffer window says which state it is in now.
    ///
    /// Like [`Order::Quit`] it is not a command and has no key: nobody asks for
    /// it. It is a REPORT, sent by the buffer's own process whenever its state
    /// changes, so the modeline can say which of the buffer's screens is up
    /// without a socket round trip inside the frame.
    ///
    /// The PID names the window for the same reason
    /// [`buffer_socket_path`] uses it: it is the only name both sides already
    /// know without a handshake.
    State {
        /// The buffer window's process.
        pid: u32,
        /// The id it declared in its descriptor.
        id: String,
        /// The tone that state declared — the colour the modeline draws it in.
        tone: ronoctua_protocol::Tone,
    },
    /// The clipboard's window wants the history, now and after every change:
    /// the connection it came on stays open, and the footer writes the whole
    /// history down it (`crate::clipboard_wire`).
    Follow(Follower),
    /// The clipboard's window asks something of the footer's history.
    Clipboard {
        /// The history it was looking at.
        generation: u64,
        /// What it asks.
        act: crate::clipboard_wire::Act,
    },
    /// A window's root gesture, for the sudo bar (`crate::sudo_bar::serve`):
    /// the connection it came on stays open for the verdict, and for the
    /// gesture's next verbs.
    Sudo(Follower, String),
    /// A secret, asked in the bar for an askpass helper
    /// (`crate::sudo_bar::serve_askpass`): the connection it came on stays
    /// open, and the typed secret is written back down it. After [`ASKPASS`]
    /// come the KIND (an `ssh…` token or `system-password`, the title), the
    /// CONTEXT (what sudo serves and from where, base64, `-` for none — the
    /// system password only), and then the prompt,
    /// none of them logged: `AskPass(follower, kind, context, prompt)`.
    AskPass(Follower, String, Option<String>, String),
}

/// A connection kept open to be written to — the clipboard's window's.
///
/// Compared by IDENTITY: two connections are the same follower only if they
/// are the same socket.
#[derive(Debug, Clone)]
pub struct Follower(pub std::sync::Arc<UnixStream>);

impl PartialEq for Follower {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Follower {}

/// The word a new footer says to the one it is going to replace.
const YIELD: &str = "quit";

/// The word an askpass helper (`ronoctua-ssh-askpass`, `-sudo-askpass`) writes
/// to ask a secret, the KIND (`sudo_bar::Kind::of`), the CONTEXT and the
/// prompt after it. Hard-coded in the helper too, which has no crate to share
/// it from.
pub const ASKPASS: &str = "askpass";

/// Standard base64 (padding tolerated, not required), for the askpass context
/// the helper sends as one token (`nix/ssh-askpass.rs`'s `base64`). `None` on
/// a character the alphabet does not have, or bytes that are not UTF-8 — the
/// footer shares no base64 crate with the helper, so the pair is spelled on
/// both sides.
fn un_base64(token: &str) -> Option<String> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(u32::from(c - b'A')),
        b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut bytes = Vec::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in token.bytes() {
        if c == b'=' {
            break;
        }
        acc = (acc << 6) | value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            #[allow(clippy::cast_possible_truncation)]
            bytes.push((acc >> bits) as u8);
        }
    }
    String::from_utf8(bytes).ok()
}

/// How long to wait for the previous footer to die before taking the socket by force.
///
/// Half a second: it is a clean exit of a process already in the event loop,
/// not a shutdown. Blowing past this means it has frozen, and then
/// taking its place is better than inheriting its session.
const PATIENCE: std::time::Duration = std::time::Duration::from_millis(500);

/// Asks this display's previous footer to leave, and waits for it to leave.
///
/// # The defect this fixes
///
/// The socket has the display in its name, and the earlier code removed the file and
/// opened its own over the top, with the comment "if there is ANOTHER footer alive in this
/// session, it is the one that is wrong to exist". Nothing made that true: the
/// previous one stayed alive, with its layer surface on screen and its `wl_keyboard`
/// on the seat.
///
/// The damage is not cosmetic. If the previous one is replaced **with the leader
/// open**, it is left with `keyboard-interactivity: exclusive` forever —
/// no order reaches it any more to shut it down. From then on the NEW footer
/// draws the shortcut list and the OLD one receives the keys, which lead to nothing
/// in it. On screen that appears as "the list opens and no key works,
/// not even to open a submenu" — with no error anywhere, and with no way to
/// reproduce, because it depends on the previous one being in the middle of a sequence.
///
/// It returns no error: there was no previous footer (the common case), or it does not
/// answer. In both cases the right thing is to carry on and open the socket.
fn yield_the_place(path: &std::path::Path) {
    let Ok(mut s) = UnixStream::connect(path) else {
        // Nobody listening. An orphan socket of a dead process, or the first
        // start — the caller's `remove_file` solves both.
        return;
    };
    tracing::info!("there is already a footer on this display; asking it for the place");
    if writeln!(s, "{YIELD}").is_err() {
        return;
    }
    drop(s);

    // WAITING FOR REAL, and not sleeping a fixed time. What matters is the
    // previous one having let go of the socket; as long as the `connect` works, it
    // is still there.
    let up_to = std::time::Instant::now() + PATIENCE;
    while std::time::Instant::now() < up_to {
        if UnixStream::connect(path).is_err() {
            tracing::info!("the previous footer left");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    tracing::warn!(
        "the previous footer did not leave within {} ms; taking the socket.\n\
             It may keep a surface on screen until it is shut down by hand.",
        PATIENCE.as_millis()
    );
}

/// Brings up the control socket and the thread that listens on it.
///
/// It returns `false` when it cannot listen — **it is not fatal**: without the socket
/// the footer loses the leader, but goes on showing the modeline. Taking everything down
/// would be trading one feature for none.
pub fn listen(channel: Sender<Order>) -> bool {
    let path = match socket_path() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "no control socket; the leader will not work");
            return false;
        }
    };
    listen_on(&path, channel)
}

/// The same, at a given path.
///
/// Separated from [`listen`] so the test can reach the mechanism without touching
/// `XDG_RUNTIME_DIR`: an environment variable is process-global, and a test
/// that changes it races with all the others in the same binary.
pub fn listen_on(path: &std::path::Path, channel: Sender<Order>) -> bool {
    let path = path.to_path_buf();
    // ONE FOOTER PER DISPLAY, guaranteed — not merely asserted. See `yield_the_place`.
    yield_the_place(&path);

    // An orphan socket of a dead process prevents the bind. After the `ceder`, what
    // is left here belongs to a process that no longer answers.
    let _ = std::fs::remove_file(&path);

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "could not open the socket");
            return false;
        }
    };
    tracing::info!(path = %path.display(), "control socket opened");

    thread::Builder::new()
        .name("control".into())
        .spawn(move || {
            for connection in listener.incoming() {
                let Ok(stream) = connection else { continue };
                let mut line = String::new();
                // BY REFERENCE, so the connection outlives the line: a
                // follower's is kept and written to.
                if BufReader::new(&stream).read_line(&mut line).is_err() {
                    continue;
                }
                // THE CLIPBOARD'S WINDOW, before anything is split or logged:
                // an act can carry a login's password, and the warning below
                // prints the line it could not read.
                if line.trim() == crate::clipboard_wire::FOLLOW {
                    if channel
                        .send(Order::Follow(Follower(std::sync::Arc::new(stream))))
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
                // A WINDOW'S ROOT GESTURE, for the sudo bar: the connection
                // kept for the verdict, and the deed never logged.
                if let Some(deed) = line
                    .trim()
                    .strip_prefix(crate::sudo::ORDER)
                    .filter(|rest| rest.starts_with(' '))
                {
                    let follower = Follower(std::sync::Arc::new(stream));
                    if channel
                        .send(Order::Sudo(follower, deed.trim().to_owned()))
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
                // A SECRET, for the askpass bar: the connection kept open for
                // what is typed, and nothing of it logged. `askpass <kind>
                // <context> <prompt>` — the kind the title (`Kind::of`), the
                // context base64 (`-` for none, sudo's command and origin
                // otherwise), the prompt the rest, which may be empty (ssh
                // sometimes asks with none).
                if let Some(rest) = line
                    .trim()
                    .strip_prefix(ASKPASS)
                    .filter(|rest| rest.is_empty() || rest.starts_with(' '))
                {
                    let mut parts = rest.trim_start().splitn(3, ' ');
                    let kind = parts.next().unwrap_or("");
                    let token = parts.next().unwrap_or("-");
                    let prompt = parts.next().unwrap_or("");
                    let context = (token != "-").then(|| un_base64(token)).flatten();
                    let follower = Follower(std::sync::Arc::new(stream));
                    if channel
                        .send(Order::AskPass(
                            follower,
                            kind.to_owned(),
                            context,
                            prompt.trim().to_owned(),
                        ))
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
                if let Some(rest) = line.trim().strip_prefix(crate::clipboard_wire::ACT) {
                    match crate::clipboard_wire::Act::parse(rest) {
                        Some((generation, act)) => {
                            if channel.send(Order::Clipboard { generation, act }).is_err() {
                                return;
                            }
                        }
                        None => tracing::warn!("a clipboard act that does not read"),
                    }
                    continue;
                }
                // `leader` or `command <id> [argument]`. The argument is the
                // rest of the line, without splitting on spaces: a workspace
                // name may have a space, and splitting it here would lose
                // half of it without saying anything.
                let mut fields = line.trim().splitn(3, ' ');
                let order = match (fields.next(), fields.next()) {
                    (Some("leader"), None) => Order::Leader,
                    (Some(YIELD), None) => Order::Quit,
                    (Some("command"), Some(id)) => {
                        Order::Command(id.to_owned(), fields.next().map(str::to_owned))
                    }
                    // `state <pid> <id> [tone]`. A pid that does not parse is
                    // dropped with the rest: there is nothing sensible to do
                    // with a report about a window nobody can name.
                    (Some("state"), Some(pid)) => match (pid.parse(), fields.next()) {
                        (Ok(pid), Some(rest)) => {
                            let mut words = rest.split_whitespace();
                            let id = words.next().unwrap_or_default().to_owned();
                            Order::State {
                                pid,
                                id,
                                tone: tone_of(words.next()),
                            }
                        }
                        _ => {
                            tracing::warn!(order = line.trim(), "a state with no pid or no id");
                            continue;
                        }
                    },
                    _ => {
                        tracing::warn!(
                            order = line.trim(),
                            "an unknown order on the control socket"
                        );
                        continue;
                    }
                };
                if channel.send(order).is_err() {
                    return;
                }
            }
        })
        .expect("the control thread cannot fail to be born");

    true
}

/// The key a lone TAP of which cycles the bars' modes at the leader's root —
/// the grafted row of the root's list ([`crate::hidden::bars_row`]) and the
/// sentences that teach it.
///
/// A constant beside the tables and never text in a `.ftl`
/// (`keys-are-never-text`): it is a key's name, the same in every language,
/// as `SPC` is.
pub const BARS_TAP_KEY: &str = "CTRL";

/// The key the shortcuts row stands for — a tap of `Alt` at the leader's root
/// opens the focused window's shortcuts (`LEADER Alt`, [`Tapped::Shortcuts`]).
/// A key's name, never text, as [`BARS_TAP_KEY`] is.
pub const SHORTCUTS_TAP_KEY: &str = "ALT";

/// The command that opens the focused window's shortcuts in the region and on
/// the minibuffer's line — what the `Alt` tap, a click on its row, and
/// `LEADER :` all reach (`Footer::open_window_shortcuts`).
pub const WINDOW_SHORTCUTS_COMMAND: &str = "window-shortcuts";

/// A modifier TAPPED — pressed and released with no other key in between.
///
/// # What a tap means now (2026-09-30)
///
/// Only a tap of `Ctrl` means anything: at the leader's root it cycles the
/// bars' modes, normal → push → cover (`crate::hidden::next_mode`). `Alt`
/// chose the window set until the owner took it away, and it still ARMS
/// here — so a `Ctrl` pressed while `Alt` is down is a chord of the two and
/// no tap — but its own release answers nothing.
///
/// # The three cases it keeps apart
///
/// - A CLEAN TAP of `Ctrl` answers `true`.
/// - A modifier held WITH another key is a chord, and no tap: `Control_R`
///   held while the arrows page the list (`Footer::navigate`) or walk the
///   minibuffer's drawer must go on doing so, and the key in between disarms
///   it. So does a second modifier.
/// - A RELEASE WITH NO PRESS answers nothing. In the nested loop the leader
///   IS `Ctrl+Alt+Space`: both are pressed BEFORE the footer has the
///   keyboard, and it sees only their releases. Counting those would cycle
///   the bars on every opening.
///
/// # Both `Ctrl`s, the right one included
///
/// The right `Ctrl` is the drawer's and the pages' modifier, and it is
/// always held WITH an arrow — a chord, which this disarms. Tapped alone it
/// has no other meaning at the root, and a hand that reaches for the nearer
/// `Ctrl` should not find that one of the two does nothing. What is weighed:
/// a right `Ctrl` pressed to page and let go without an arrow cycles the
/// bars — seen at once, and one more tap and a third undo it, where a `Ctrl`
/// that sometimes works is a key nobody can learn.
///
/// `AltGr` (`ISO_Level3_Shift`) is not `Alt`: it is how half the world types
/// its own letters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tap {
    /// The modifier pressed alone, still down.
    armed: Option<smithay_client_toolkit::seat::keyboard::Keysym>,
}

impl Tap {
    /// Any key went DOWN.
    pub fn pressed(&mut self, sym: smithay_client_toolkit::seat::keyboard::Keysym) {
        self.armed = match (self.armed, arms(sym)) {
            // A MODIFIER ALONE arms; the same one again is a repeat of it.
            (None, true) => Some(sym),
            (Some(held), true) if held == sym => Some(held),
            // Anything else — a letter, an arrow, a second modifier — is a
            // chord, and no tap.
            _ => None,
        };
    }

    /// A pointer or a finger ACTED — a button pressed, a wheel turned, a
    /// finger down — while a modifier may be down: that is a chord with the
    /// hand, a Ctrl-click or a Ctrl-wheel, and no tap. Without it, the
    /// release after a Ctrl-click on a hint cycled the bars.
    pub const fn pointer_acted(&mut self) {
        self.armed = None;
    }

    /// A key came UP: what a clean tap of it does at the leader's root — a tap
    /// of `Ctrl` cycles the bars, a tap of `Alt` opens the focused window's
    /// shortcuts (`LEADER Alt`), and anything else is [`Tapped::Nothing`].
    pub fn released(&mut self, sym: smithay_client_toolkit::seat::keyboard::Keysym) -> Tapped {
        match self.armed {
            Some(held) if held == sym => {
                self.armed = None;
                tapped_of(sym)
            }
            _ => Tapped::Nothing,
        }
    }
}

/// What a clean tap of a modifier at the leader's root asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tapped {
    /// Not a clean tap, or a modifier that means nothing tapped.
    Nothing,
    /// A tap of `Ctrl`: the bars' modes in turn (`crate::hidden::next_mode`).
    Bars,
    /// A tap of `Alt`: the focused window's shortcuts, in the region and on the
    /// minibuffer's line (`LEADER Alt`, `Footer::open_window_shortcuts`).
    Shortcuts,
}

/// What a clean tap of this key does — either `Ctrl` cycles the bars, either
/// `Alt` opens the shortcuts, and nothing else arms a tap ([`arms`]).
const fn tapped_of(sym: smithay_client_toolkit::seat::keyboard::Keysym) -> Tapped {
    use smithay_client_toolkit::seat::keyboard::Keysym;
    match sym {
        Keysym::Control_L | Keysym::Control_R => Tapped::Bars,
        Keysym::Alt_L | Keysym::Alt_R => Tapped::Shortcuts,
        _ => Tapped::Nothing,
    }
}

/// Whether a tap of the ON-SCREEN keyboard's modifier cycles the bars — the
/// same answer the physical keys give for `Ctrl` ([`tapped_of`]): `Ctrl`, and
/// neither `Alt` nor `AltGr`.
#[must_use]
pub const fn osk_tap_cycles_the_bars(m: crate::osk::Modifier) -> bool {
    matches!(m, crate::osk::Modifier::Ctrl)
}

/// Whether this key, pressed alone, ARMS a tap: the two modifiers a tap is
/// ever made of — `Ctrl` (cycles the bars) and `Alt` (opens the shortcuts).
fn arms(sym: smithay_client_toolkit::seat::keyboard::Keysym) -> bool {
    use smithay_client_toolkit::seat::keyboard::Keysym;
    matches!(
        sym,
        Keysym::Alt_L | Keysym::Alt_R | Keysym::Control_L | Keysym::Control_R
    )
}

/// The leader's own hint at the ROOT: the NEXT set, which the leader pressed
/// again would switch to — its key (the leader's label, `leader`) and a short
/// word: `SUPER windows` in the default set, `SUPER everything` in the window
/// set and `SUPER default` in the full one.
///
/// It is a CLICK (invariant 19): a pointer cannot press the leader, so the
/// click runs the command the press stands for ([`Set::command`]), and the
/// pointer resting on it reads that command's `cmd-` sentence.
#[must_use]
pub fn set_hint(set: Set, leader: &str, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
    use crate::chrome::Piece;
    let id = set.next().command();
    vec![
        Piece::key(leader, false).about(id).running(id),
        // THE SPACE BELONGS TO THE WORDS, as `chrome::hint_of_marked` keeps it.
        Piece::prose(format!(" {}", texts.t(id)))
            .about(id)
            .running(id),
    ]
}

/// The leader's state: nothing, or a sequence in progress.
#[derive(Debug, Default)]
pub struct Leader {
    /// The keys already typed after the leader. Empty = a freshly activated leader.
    sequence: Vec<ronoctua_keymap::Key>,
    active_one: bool,
    /// Which tree the sequence walks. [`Set::Footer`] from every opening.
    set: Set,
}

impl Leader {
    /// Whether no key has been typed since the leader opened.
    ///
    /// The focused buffer's submenu hangs off the ROOT of the sequence, and
    /// only there: `m` after `w` is `w m`, which belongs to the tree.
    #[must_use]
    pub fn at_root(&self) -> bool {
        self.sequence.is_empty()
    }

    /// The keys typed since the leader opened.
    ///
    /// It exists for the same reason [`Self::at_root`] does: a submenu whose
    /// rows are built at RUNTIME — the focused buffer's keymap, the states it
    /// declares — has to know where in the sequence it is, and the tree cannot
    /// answer because those rows are not in it.
    #[must_use]
    pub fn typed(&self) -> &[ronoctua_keymap::Key] {
        &self.sequence
    }

    /// The set the sequence walks.
    #[must_use]
    pub const fn set_now(&self) -> Set {
        self.set
    }

    /// Switches the set — only at the ROOT of an open leader, and the last
    /// switch wins. `false`, and nothing changed, anywhere else: a set chosen
    /// halfway through a sequence would give the keys already typed a meaning
    /// they were not typed with.
    pub fn set(&mut self, set: Set) -> bool {
        if !self.active_one || !self.sequence.is_empty() {
            return false;
        }
        self.set = set;
        true
    }

    /// The trigger as a sequence shows it: the leader's own label, and the
    /// leader again after it once per press that chose the set.
    fn head(&self, trigger: &str) -> String {
        match self.set.key_with(trigger) {
            Some(k) => format!("{trigger} {k}"),
            None => trigger.to_owned(),
        }
    }
}

/// What opened the leader — which decides whether its list waits
/// ([`Patience`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opener {
    /// A key: the compositor's bind over the control socket, the keyboards'
    /// daemon, or the on-screen keyboard's leader.
    Key,
    /// A click — on the empty minibuffer, or on a row of the list itself.
    Pointer,
}

/// Whether the leader's list waits before it is drawn.
///
/// # Why a list waits at all
///
/// With the bars hidden (`crate::hidden`), a person who knows the sequence
/// types it in one breath, and a list that opened over the windows on every
/// leader would flash across the screen on the way to the command. So when a
/// KEY opened the leader, the list is held back until the hand stops for
/// [`crate::surface::LIST_PATIENCE`] — counted from the LAST key, because a
/// person reading the first level and then typing is exactly who should not
/// wait again. The pending chord is on the minibuffer at once either way.
///
/// With the bars visible nothing waits: the list is where it always was. And
/// a leader opened by the POINTER shows its list at once — a hand on the
/// mouse cannot "type fast", and a click that answered with nothing for two
/// seconds would read as a click that did nothing.
///
/// # Once shown, shown
///
/// Every level after the first, and every change of set, is drawn at once
/// until the sequence ends: the person has stopped once already, and is
/// reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Patience {
    /// The list is held back. The number is the ONE timer that may show it:
    /// a key re-arms with a new number, and an older timer that fires finds
    /// its number stale and does nothing.
    Waiting(u64),
    /// The list is drawn.
    #[default]
    Shown,
}

impl Patience {
    /// At the leader's opening: waiting only with the bars hidden and a key
    /// that opened it.
    #[must_use]
    pub const fn at_opening(hidden: bool, by_key: bool, generation: u64) -> Self {
        if hidden && by_key {
            Self::Waiting(generation)
        } else {
            Self::Shown
        }
    }

    /// A key arrived while the leader is open. A list still waiting counts
    /// again from this key, under `generation`, which is returned for the
    /// caller to arm a timer with; a shown list answers `None`.
    pub const fn after_a_key(&mut self, generation: u64) -> Option<u64> {
        match self {
            Self::Waiting(g) => {
                *g = generation;
                Some(generation)
            }
            Self::Shown => None,
        }
    }

    /// The timer numbered `generation` ran out. `true` when that SHOWED the
    /// list — a stale timer, or a list already shown, answers `false`.
    pub fn ran_out(&mut self, generation: u64) -> bool {
        if *self == Self::Waiting(generation) {
            *self = Self::Shown;
            true
        } else {
            false
        }
    }

    /// Whether the list is held back right now.
    #[must_use]
    pub const fn withheld(self) -> bool {
        matches!(self, Self::Waiting(_))
    }
}

/// What the shell should do after a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Keep capturing; the list changed.
    Continue {
        /// which-key rows, ready to draw.
        inputs: Vec<ronoctua_keymap::Entry>,
    },
    /// Leave the leader and go back to two lines.
    Close {
        /// The SAME sequence with the last key's case swapped, when it is
        /// THAT one that exists in the keymap.
        ///
        /// `Some(i)` when `I` was typed and what is bound is `i`. It serves the
        /// message: "not bound" shows the consequence, and on its own
        /// leaves the person looking for a shortcut they have just pressed. The
        /// cause is almost always Shift or Caps Lock, and it is the cause that needs
        /// to appear on screen.
        other_case: Option<ronoctua_keymap::Key>,
    },
    /// Run a command — with its fixed argument, when there is one — and
    /// leave the leader.
    Run(&'static str, Option<&'static str>),
}

impl Leader {
    /// Whether it is capturing the keyboard right now.
    #[must_use]
    pub const fn active_one(&self) -> bool {
        self.active_one
    }

    /// The typed sequence, as it appears in the minibuffer.
    ///
    /// It ends in `-`, and not in ` …`: it is Doom's notation, and it fits in
    /// two columns fewer. The ellipsis said the same thing taking up
    /// more space — and in a footer where everything elides, space is the resource.
    ///
    /// THE SET IS PART OF IT (`SUPER SUPER-`, `SUPER SUPER SUPER s n-`): the
    /// same `s n` runs a different thing in each set, and the chord on screen
    /// is the one place that says which tree the next key walks — and it says
    /// it as the keys that were pressed to get there.
    #[must_use]
    pub fn prefix(&self, trigger: &str) -> String {
        format!("{}-", self.raw_prefix(trigger))
    }

    /// The typed sequence, WITHOUT the continuation ellipsis.
    ///
    /// The `prefix` ends in `…` because it says "keys are missing". In an
    /// error message that would be a lie: there the sequence ENDED, and with no
    /// destination. Two readings, two functions.
    #[must_use]
    pub fn raw_prefix(&self, trigger: &str) -> String {
        let head = self.head(trigger);
        if self.sequence.is_empty() {
            head
        } else {
            format!("{head} {}", ronoctua_keymap::notation(&self.sequence))
        }
    }

    /// Activates the leader from scratch, in the default set.
    pub fn activate(&mut self) {
        self.active_one = true;
        self.sequence.clear();
        self.set = Set::Footer;
    }

    /// Switches it off.
    pub fn finish(&mut self) {
        self.active_one = false;
        self.sequence.clear();
        self.set = Set::Footer;
    }

    /// Processes a key while the leader is active, in the tree of its set.
    ///
    /// `ESC` **always** ends it, even if there is a binding for it. Section
    /// 7.4 puts that as a rule: there has to be an exit that works
    /// without the user knowing what state they are in.
    pub fn key(&mut self, trees: &Trees, t: ronoctua_keymap::Key) -> Effect {
        if t.sym == ronoctua_keymap::Sym::Esc {
            self.finish();
            return Effect::Close { other_case: None };
        }

        let tree = trees.of(self.set);
        self.sequence.push(t);
        match tree.resolve(&self.sequence) {
            ronoctua_keymap::Resolution::Command(id, argument) => {
                self.finish();
                Effect::Run(id, argument)
            }
            ronoctua_keymap::Resolution::Prefix(inputs) => Effect::Continue { inputs },
            // A sequence with no binding: it ends instead of getting stuck. Leaving the
            // leader active after a wrong key is like locking up the
            // keyboard, and the user would have no way of knowing why.
            ronoctua_keymap::Resolution::Nothing => {
                let other_case = self.the_other_key(tree);
                self.finish();
                Effect::Close { other_case }
            }
        }
    }

    /// The SIMILAR key that does exist in the keymap, when the typed one does not.
    ///
    /// Two differences, because those are the two that appeared on the owner's screen:
    ///
    /// - **a leftover modifier** — `CTRL ALT SPC M-n is not bound`, with the Alt
    ///   stuck in the compositor after a host shortcut;
    /// - **swapped case** — `CTRL ALT SPC I is not bound`, with Caps Lock or
    ///   Shift.
    ///
    /// This is **not** a fallback: nothing is run through the similar key. `w H`
    /// moves the window and `w h` focuses — falling from one to the other would make the shortcut
    /// do something else, which is worse than doing nothing. It exists only
    /// so the message names the cause instead of just the consequence.
    fn the_other_key(&self, tree: &Tree) -> Option<ronoctua_keymap::Key> {
        use ronoctua_keymap::{Key, Mods, Sym};
        let last_one = *self.sequence.last()?;

        let no_mods = Key {
            mods: Mods {
                ctrl: false,
                alt: false,
                shift: false,
                logo: false,
            },
            ..last_one
        };
        let other_case = |t: Key| -> Option<Key> {
            let Sym::Char(c) = t.sym else { return None };
            let swapped = if c.is_uppercase() {
                c.to_lowercase().next()?
            } else if c.is_lowercase() {
                c.to_uppercase().next()?
            } else {
                return None;
            };
            Some(Key {
                sym: Sym::Char(swapped),
                ..t
            })
        };

        // IN ORDER OF MOST LIKELY CAUSE, and stopping at the first that matches.
        // Removing the modifier comes first because it is the case the person cannot
        // undo on their own.
        let candidates = [
            Some(no_mods).filter(|t| *t != last_one),
            other_case(last_one),
            other_case(no_mods),
        ];

        let mut attempt = self.sequence.clone();
        for target in candidates.into_iter().flatten() {
            *attempt.last_mut()? = target;
            if !matches!(tree.resolve(&attempt), ronoctua_keymap::Resolution::Nothing) {
                return Some(target);
            }
        }
        None
    }
}

/// How `brain-abort` is typed — inside the brain's buffer.
///
/// A constant and not a literal in two places: the BAND DRAWS IT, because a
/// control whose key is only in the manual is a control nobody presses — and a
/// sequence that lived in the table and again in the drawing would be two
/// answers to "which key stops it", with the wrong one on screen.
///
/// # Why it is no longer a leader sequence
///
/// It was `LEADER b a`, beside `LEADER b b` for the screen itself. The owner
/// asked for `LEADER b` to open the brain with nothing in between, and the
/// keymap refuses — at BUILD time — a sequence that is both a command and the
/// prefix of another. So `b a` and `b p` had to go, and what they did already
/// existed inside the buffer under its own leader: `, a` and `, p`.
///
/// This is what the band prints, so the line that appears without anybody
/// asking still says where the keyboard's answer is. The mouse keeps both
/// buttons on the band itself; the keyboard opens the brain first.
pub const BRAIN_ABORT_KEYS: &str = ", a";

/// The same, for `brain-proceed`.
pub const BRAIN_PROCEED_KEYS: &str = ", p";

/// And for the button a standing notice offers.
///
/// `, n`, inside the brain, for the same reason the two above are `, a` and
/// `, p`: the band's controls are answered from the brain's own buffer, and
/// the band prints what that buffer binds.
pub const BRAIN_NOTICE_KEYS: &str = ", n";

/// The prefix the buffer's STATE chooser hangs off — `w`, the window's letter,
/// where the buffer commands live — and the key under it.
///
/// Two constants and not two literals, because the footer decides this in TWO
/// places: the which-key list grafts the row (`apply_leader_effect_with`) and
/// the key opens it (`state_key`). When the buffer commands moved from `b` to
/// `w`, one site moved and the other did not: `SPC b` listed a row that did
/// nothing and `SPC w` listed nothing that worked. One name, two readers.
pub const STATE_PREFIX: &str = "w";

/// The key that opens the state chooser under [`STATE_PREFIX`].
pub const STATE_KEY: &str = "s";

/// The root of the list, with the nine workspace digits drawn as ONE row.
///
/// `LEADER 1`…`9` are nine bindings and one gesture, and nine rows saying
/// "Go to workspace" spent a column of the list that is the scarce resource
/// there. The row reads `1…9`, and its key — what a CLICK on it presses — is
/// `TAB`: a click cannot say which number, so it opens the workspace group,
/// where each one is a row of its own. The digits still work from the
/// keyboard; only the drawing is condensed, and only at the root.
#[must_use]
pub fn condensed_root(entries: Vec<ronoctua_keymap::Entry>) -> Vec<ronoctua_keymap::Entry> {
    let is_a_digit = |e: &ronoctua_keymap::Entry| {
        e.command.as_deref() == Some("workspace-go")
            && matches!(
                e.key.as_str(),
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
            )
    };
    let Some(first) = entries.iter().position(is_a_digit) else {
        return entries;
    };
    let label = entries[first].label.clone();
    let mut out: Vec<ronoctua_keymap::Entry> = Vec::with_capacity(entries.len());
    for (i, e) in entries.into_iter().enumerate() {
        if i == first {
            out.push(ronoctua_keymap::Entry {
                key: RANGE_ROW.to_owned(),
                sym: ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Tab),
                label: label.clone(),
                command: Some("workspace-go".to_owned()),
                is_group: false,
            });
        } else if !is_a_digit(&e) {
            out.push(e);
        }
    }
    out
}

/// How the condensed row of workspace digits is written in the list.
pub const RANGE_ROW: &str = "1…9";

/// The footer's own keys, beside the leader that walks them: the window's,
/// the workspace's and the buffer's commands (a buffer IS a window of the
/// compositor, invariant 4), the session and its way out, `M-x`, the
/// leader's three sets, the brain, the launcher, the theme chooser, the
/// glance, the kiosk, the footer's settings and hidden modes, the keyboard's
/// top row and the buffers the footer opens — and, generated, the nine
/// workspace digits, the window menu's levels, the compositor's deeds and
/// the brain modules' actions ([`base_generated`]).
///
/// Every other crate says its own keys beside its view
/// ([`crate::bindings::Contribution`]), and the binary assembles them all
/// (its `keymap`). Labels are text KEYS (invariant 8), and they come from
/// the CONSTANTS generated from the `.ftl`: writing the string by hand would
/// compile and would lose the guarantee; with the constant, a key that does
/// not exist is a compile error.
pub const BASE: Contribution = Contribution {
    owner: "footer",
    commands: &[
        Command {
            id: "buffer-next",
            label: key::CMD_BUFFER_NEXT,
        },
        // `LEADER w w` — the buffer SELECTOR, with completion.
        //
        // It had been taken out of the keymap because it promised something that
        // did not exist: the locator only knew how to list `&'static str` from
        // the command registry, and a buffer has no entry there. With the
        // locator's items carrying their own label, the same list serves both.
        Command {
            id: "buffer-switch",
            label: key::CMD_BUFFER_SWITCH,
        },
        Command {
            id: "buffer-close",
            label: key::CMD_BUFFER_CLOSE,
        },
        Command {
            id: "session-quit",
            label: key::CMD_SESSION_QUIT,
        },
        // `M-x`: the proof, in the user's hand, that every command is
        // addressable by NAME and not only by key (section 13).
        Command {
            id: "mx-open",
            label: key::CMD_MX_OPEN,
        },
        // THE LEADER'S THREE SETS — what the leader pressed again with the
        // list open walks through, as commands for the pointer and for
        // `LEADER :`. See [`Set`].
        Command {
            id: "leader-set-window",
            label: key::CMD_LEADER_SET_WINDOW,
        },
        Command {
            id: "leader-set-full",
            label: key::CMD_LEADER_SET_FULL,
        },
        Command {
            id: "buffer-terminal",
            label: key::CMD_BUFFER_TERMINAL,
        },
        Command {
            id: "buffer-terminal-external",
            label: key::CMD_BUFFER_TERMINAL_EXTERNAL,
        },
        // THE TWO MONITORS. They have no key in this tree, and that is not an
        // oversight: they are reached by CHORD (`LEADER+Esc`, `LEADER+S-Esc`)
        // and, the first one, by `Ctrl+Alt+Esc` with no leader at all —
        // `reachable` counts all three, so neither is an orphan. They are in the
        // registry because `LEADER :` addresses every command by name, and
        // because the chord's label comes from `cmd-<id>`.
        Command {
            id: "monitor-btop",
            label: key::CMD_MONITOR_BTOP,
        },
        // THE RESCUE CONSOLE — `Ctrl+Alt+Del`, outside the leader.
        //
        // It has no sequence, and that is deliberate: the leader depends on
        // keyd telling a tap from a hold, on the daemon being scheduled and on
        // this footer being alive to expand. Every one of those is something
        // the rescue console exists to repair.
        Command {
            id: "rescue-open",
            label: key::CMD_RESCUE_OPEN,
        },
        // `LEADER s m`. The panel's own commands are NOT here: they live in the
        // buffer's descriptor, under `SPC m`, and are resolved by the buffer's
        // own host while it holds the keyboard.
        // THE KEYBOARDS, and the only place a per-device setting exists on this
        // system: the compositor has one keyboard configuration for all of
        // them, so what this reaches is `keyd`, below it.
        Command {
            id: "keyboard-open",
            label: key::CMD_KEYBOARD_OPEN,
        },
        Command {
            id: "memory-open",
            label: key::CMD_MEMORY_OPEN,
        },
        // THE BUILD SWITCH, `/ b` — the development group's: build and switch
        // to the other of the machine's own and a distro checkout's reference
        // build, and say which is running on the modeline (`build_switch`).
        Command {
            id: "build-toggle",
            label: key::CMD_BUILD_TOGGLE,
        },
        // THE BUILD PAGE, `/ p` and the glyph's right-click: what the feature
        // is, which build runs, and the glyph setting (`crate::build_page`).
        // Its three view keys and the setting's own command are reached
        // through the view (`base_reached`), as the footer settings' are.
        Command {
            id: "build-open",
            label: key::CMD_BUILD_OPEN,
        },
        Command {
            id: "build-show-cycle",
            label: key::CMD_BUILD_SHOW_CYCLE,
        },
        Command {
            id: "build-page-up",
            label: key::CMD_BUILD_PAGE_UP,
        },
        Command {
            id: "build-page-down",
            label: key::CMD_BUILD_PAGE_DOWN,
        },
        Command {
            id: "build-page-enter",
            label: key::CMD_BUILD_PAGE_ENTER,
        },
        // THE REFERENCE for several keyboards in one app — by name only: it
        // is a model for apps to come, not a place anybody goes daily.
        Command {
            id: "example-open",
            label: key::CMD_EXAMPLE_OPEN,
        },
        // `LEADER b` — what the machine does BY ITSELF. The panel's own
        // commands are not here either: they live in the buffer's descriptor,
        // under `SPC m`, and its host resolves them while it holds the
        // keyboard.
        Command {
            id: "brain-open",
            label: key::CMD_BRAIN_OPEN,
        },
        // AND THE COUNTDOWN'S TWO ANSWERS, which are the band's two controls.
        //
        // They are registered HERE, and not only in the buffer, because the
        // line they belong to is the FOOTER'S: it appears with no window open
        // and it is clickable, and the mouse may not reach a door the keyboard
        // does not. The buffer has its own `, a` and `, p` for the same two
        // verbs — same letters, its own prefix — and the two never meet: a
        // buffer's sequence is resolved by the buffer's process.
        Command {
            id: "brain-abort",
            label: key::CMD_BRAIN_ABORT,
        },
        Command {
            id: "brain-proceed",
            label: key::CMD_BRAIN_PROCEED,
        },
        // THE ON-SCREEN KEYBOARD, out and away — `m k`. It is drawn at the
        // bottom of the footer and every setting it has is on it (`crate::osk`).
        Command {
            id: "onscreen-keyboard",
            label: key::CMD_ONSCREEN_KEYBOARD,
        },
        Command {
            id: "gestures",
            label: key::CMD_GESTURES,
        },
        Command {
            id: "gestures-calibrate",
            label: key::CMD_GESTURES_CALIBRATE,
        },
        // THE KEYBOARD'S OWN TOP ROW. No leader: see `actions::media` for why
        // these seven are a family of their own and not four more emergencies.
        Command {
            id: "volume-up",
            label: key::CMD_VOLUME_UP,
        },
        Command {
            id: "volume-down",
            label: key::CMD_VOLUME_DOWN,
        },
        Command {
            id: "volume-mute",
            label: key::CMD_VOLUME_MUTE,
        },
        Command {
            id: "mic-mute",
            label: key::CMD_MIC_MUTE,
        },
        Command {
            id: "media-play",
            label: key::CMD_MEDIA_PLAY,
        },
        Command {
            id: "media-next",
            label: key::CMD_MEDIA_NEXT,
        },
        Command {
            id: "media-stop",
            label: key::CMD_MEDIA_STOP,
        },
        Command {
            id: "brightness-up",
            label: key::CMD_BRIGHTNESS_UP,
        },
        Command {
            id: "brightness-down",
            label: key::CMD_BRIGHTNESS_DOWN,
        },
        Command {
            id: "media-previous",
            label: key::CMD_MEDIA_PREVIOUS,
        },
        Command {
            id: "monitor-bottom",
            label: key::CMD_MONITOR_BOTTOM,
        },
        // ---------------------------------------------------------------
        // WINDOW AND WORKSPACE — what used to be the compositor's own binds.
        //
        // Section 7.4 wants ONE keymap. While the compositor had its
        // shortcuts and the shell had its own, there were two tables to
        // maintain, two notations and two places where a key could collide
        // — and which-key knew only half. Now the compositor has ONE bind (the
        // leader) and all the rest lives here, reaching it over the IPC
        // (invariant 3).
        // ---------------------------------------------------------------
        Command {
            id: "window-left",
            label: key::CMD_WINDOW_LEFT,
        },
        Command {
            id: "window-right",
            label: key::CMD_WINDOW_RIGHT,
        },
        Command {
            id: "window-up",
            label: key::CMD_WINDOW_UP,
        },
        Command {
            id: "window-down",
            label: key::CMD_WINDOW_DOWN,
        },
        Command {
            id: "window-move-left",
            label: key::CMD_WINDOW_MOVE_LEFT,
        },
        Command {
            id: "window-move-right",
            label: key::CMD_WINDOW_MOVE_RIGHT,
        },
        Command {
            id: "window-move-up",
            label: key::CMD_WINDOW_MOVE_UP,
        },
        Command {
            id: "window-move-down",
            label: key::CMD_WINDOW_MOVE_DOWN,
        },
        Command {
            id: "window-maximize",
            label: key::CMD_WINDOW_MAXIMIZE,
        },
        Command {
            id: "window-screen-full",
            label: key::CMD_WINDOW_SCREEN_FULL,
        },
        Command {
            id: "window-center",
            label: key::CMD_WINDOW_CENTER,
        },
        Command {
            id: "window-consume",
            label: key::CMD_WINDOW_CONSUME,
        },
        Command {
            id: "window-expel",
            label: key::CMD_WINDOW_EXPEL,
        },
        Command {
            id: "workspace-next",
            label: key::CMD_WORKSPACE_NEXT,
        },
        Command {
            id: "workspace-previous",
            label: key::CMD_WORKSPACE_PREVIOUS,
        },
        Command {
            id: "workspace-move-next",
            label: key::CMD_WORKSPACE_MOVE_NEXT,
        },
        Command {
            id: "workspace-move-previous",
            label: key::CMD_WORKSPACE_MOVE_PREVIOUS,
        },
        Command {
            id: "workspace-overview",
            label: key::CMD_WORKSPACE_OVERVIEW,
        },
        // EVERY WORKSPACE AT ONCE, and ours rather than the compositor's
        // toggle: nothing the compositor says tells whether that view is on,
        // so the footer keeps the answer and says `enable` or `disable`.
        Command {
            id: "workspace-all",
            label: key::CMD_WORKSPACE_ALL,
        },
        // ---------------------------------------------------------------
        // THE ONES THAT EXISTED ONLY AS A KEY OR ONLY AS A SOCKET ORDER.
        //
        // `window-float` was a chord with a hand-written command; the `notif-*` were
        // socket orders with no id at all. All three were features with no
        // name — the opposite of what section 13 asks for, and invisible in `M-x`.
        // ---------------------------------------------------------------
        Command {
            id: "window-float",
            label: key::CMD_WINDOW_FLOAT,
        },
        Command {
            id: "window-menu",
            label: key::CMD_WINDOW_MENU,
        },
        Command {
            id: "drawer-open",
            label: key::CMD_DRAWER_OPEN,
        },
        Command {
            id: "trail-open",
            label: key::CMD_TRAIL_OPEN,
        },
        Command {
            id: "selection-open",
            label: key::CMD_SELECTION_OPEN,
        },
        Command {
            id: "workspace-go",
            label: key::CMD_WORKSPACE_GO,
        },
        Command {
            id: "workspace-move",
            label: key::CMD_WORKSPACE_MOVE,
        },
        // FORMATTING A DISK, in a window of its own (`LEADER s f`). It is also
        // the second door of one thing: the sidebar's own menu reaches the same
        // buffer with the row already chosen, and this reaches it with the list
        // open. Neither is a menu (invariant 14) — both are the same three
        // questions, entered at different points.
        Command {
            id: "format-open",
            label: key::CMD_FORMAT_OPEN,
        },
        // THE LAUNCHER'S ONE KEY OF ITS OWN. Every other key in `LEADER a` is
        // `gesture_of`'s — the arrows, `Esc`, `RET` — and those are the same
        // everywhere. This one is the launcher's, so it is a command, in a
        // table, reachable and listed. A key living only inside a `match` is a
        // key nothing can enumerate: not `M-x`, not the manual, not a check.
        Command {
            id: "apps-pin",
            label: key::CMD_APPS_PIN,
        },
        Command {
            id: "apps-details",
            label: key::CMD_APPS_DETAILS,
        },
        Command {
            id: "apps-remove",
            label: key::CMD_APPS_REMOVE,
        },
        Command {
            id: crate::apps::SEARCH_TO_INSTALL,
            label: key::CMD_APPS_SEARCH_TO_INSTALL,
        },
        // THE KEYBOARD'S SAFETY NET — see `nix/modules/keyboard.nix`.
        //
        // keyd sits between the keyboard and everything else, and if it falls over the leader
        // goes with it. This command brings it back up, and it exists as a command
        // (and not as a loose script) because §13 holds for it more than
        // for any other: it is what one looks for when nothing else responds.
        Command {
            id: "keyboard-restore",
            label: key::CMD_KEYBOARD_RESTORE,
        },
        // THE APPLICATION LAUNCHER. It is the fifth view of the expanded
        // region, and not a start menu: same surface, same type-and-narrow
        // gesture as `M-x` (invariant 14).
        Command {
            id: "apps-open",
            label: key::CMD_APPS_OPEN,
        },
        // ---------------------------------------------------------------
        // THE THEME CHOOSER — `LEADER s t`. Two commands: the one that opens
        // the lists, and the one their `RET` runs. Moving a cursor PREVIEWS
        // the whole choice everywhere — a preview writes the file, and the
        // file is what reaches the other doors (`theme::show_everywhere`);
        // `theme-apply` is what makes it the choice `Esc` no longer undoes.
        // No `theme-next`: `check-keymap` refuses a command with no key, and a
        // key outside the list would cycle a palette nobody can see the name of.
        // ---------------------------------------------------------------
        // THE GLANCE. At the FIRST level, where `a`, `c` and `.` are, and
        // the owner chose it there: a glance exists to be instant, and at
        // three keys it competes with looking at a clock some other way.
        Command {
            id: "world-glance",
            label: key::CMD_WORLD_GLANCE,
        },
        Command {
            id: "theme-open",
            label: key::CMD_THEME_OPEN,
        },
        Command {
            id: "theme-apply",
            label: key::CMD_THEME_APPLY,
        },
        // THE CHROME KIOSK (`LEADER m b`, `crate::kiosk`): the door, what
        // its modeline, its view and the `,` group run — one table, three
        // doors (`kiosk_view::ROOT`) — and the switch of the modeline's
        // adaptation to it (`crate::adapt`).
        Command {
            id: "kiosk-open",
            label: key::CMD_KIOSK_OPEN,
        },
        Command {
            id: "kiosk-back",
            label: key::CMD_KIOSK_BACK,
        },
        Command {
            id: "kiosk-forward",
            label: key::CMD_KIOSK_FORWARD,
        },
        Command {
            id: "kiosk-reload",
            label: key::CMD_KIOSK_RELOAD,
        },
        Command {
            id: "kiosk-tab",
            label: key::CMD_KIOSK_TAB,
        },
        Command {
            id: "kiosk-tab-next",
            label: key::CMD_KIOSK_TAB_NEXT,
        },
        Command {
            id: "kiosk-tab-previous",
            label: key::CMD_KIOSK_TAB_PREVIOUS,
        },
        Command {
            id: "kiosk-tab-close",
            label: key::CMD_KIOSK_TAB_CLOSE,
        },
        Command {
            id: "kiosk-tab-left",
            label: key::CMD_KIOSK_TAB_LEFT,
        },
        Command {
            id: "kiosk-tab-right",
            label: key::CMD_KIOSK_TAB_RIGHT,
        },
        Command {
            id: "kiosk-tab-move",
            label: key::CMD_KIOSK_TAB_MOVE,
        },
        Command {
            id: "kiosk-new-tab",
            label: key::CMD_KIOSK_NEW_TAB,
        },
        Command {
            id: "kiosk-address",
            label: key::CMD_KIOSK_ADDRESS,
        },
        Command {
            id: "kiosk-zoom",
            label: key::CMD_KIOSK_ZOOM,
        },
        Command {
            id: "kiosk-zoom-in",
            label: key::CMD_KIOSK_ZOOM_IN,
        },
        Command {
            id: "kiosk-zoom-out",
            label: key::CMD_KIOSK_ZOOM_OUT,
        },
        Command {
            id: "kiosk-zoom-reset",
            label: key::CMD_KIOSK_ZOOM_RESET,
        },
        Command {
            id: "kiosk-chrome-ui",
            label: key::CMD_KIOSK_CHROME_UI,
        },
        Command {
            id: "kiosk-address-line",
            label: key::CMD_KIOSK_ADDRESS_LINE,
        },
        Command {
            id: "kiosk-more",
            label: key::CMD_KIOSK_MORE,
        },
        Command {
            id: "kiosk-tabs",
            label: key::CMD_KIOSK_TABS,
        },
        Command {
            id: "kiosk-extensions",
            label: key::CMD_KIOSK_EXTENSIONS,
        },
        Command {
            id: "kiosk-extension",
            label: key::CMD_KIOSK_EXTENSION,
        },
        Command {
            id: "kiosk-extension-options",
            label: key::CMD_KIOSK_EXTENSION_OPTIONS,
        },
        Command {
            id: "kiosk-extension-details",
            label: key::CMD_KIOSK_EXTENSION_DETAILS,
        },
        Command {
            id: "kiosk-release",
            label: key::CMD_KIOSK_RELEASE,
        },
        Command {
            id: "modeline-adapt-kiosk",
            label: key::CMD_MODELINE_ADAPT_KIOSK,
        },
        // THE BARS HIDDEN, one way or the other, or shown again — see
        // `crate::hidden`. By name only (`by_name`): the key of the
        // cycle is a tap of `Ctrl` at the leader's root.
        Command {
            id: "footer-hide-push",
            label: key::CMD_FOOTER_HIDE_PUSH,
        },
        Command {
            id: "footer-hide-cover",
            label: key::CMD_FOOTER_HIDE_COVER,
        },
        Command {
            id: "kiosk-list-down",
            label: key::CMD_KIOSK_LIST_DOWN,
        },
        Command {
            id: "kiosk-list-up",
            label: key::CMD_KIOSK_LIST_UP,
        },
        Command {
            id: "kiosk-list-enter",
            label: key::CMD_KIOSK_LIST_ENTER,
        },
        Command {
            id: "kiosk-tab-earlier",
            label: key::CMD_KIOSK_TAB_EARLIER,
        },
        Command {
            id: "kiosk-tab-later",
            label: key::CMD_KIOSK_TAB_LATER,
        },
        // THE STRIP'S THREE ABOUT THE LOUDEST NOTICE: put it away, put it off
        // by its first snooze, and do its first control — and, named, any
        // action a module offers (`brain-act <module> <action>`).
        Command {
            id: "brain-dismiss",
            label: key::CMD_BRAIN_DISMISS,
        },
        Command {
            id: "brain-snooze",
            label: key::CMD_BRAIN_SNOOZE,
        },
        Command {
            id: "brain-act",
            label: key::CMD_BRAIN_ACT,
        },
        // ── end of brain-ui ──
        Command {
            id: "leader-set-default",
            label: key::CMD_LEADER_SET_DEFAULT,
        },
        Command {
            id: "footer-bars-cycle",
            label: key::CMD_FOOTER_BARS_CYCLE,
        },
        Command {
            id: "leader-close",
            label: key::CMD_LEADER_CLOSE,
        },
        Command {
            id: "minibuffer-fold-open",
            label: key::CMD_MINIBUFFER_FOLD_OPEN,
        },
        Command {
            id: "minibuffer-fold-close",
            label: key::CMD_MINIBUFFER_FOLD_CLOSE,
        },
        Command {
            id: "alert-dismiss",
            label: key::CMD_ALERT_DISMISS,
        },
        // THE FOOTER'S OWN SETTINGS, `LEADER s r` — the bars' mode, what the
        // free minibuffer shows, and the kiosk's adapter (`footer_settings`).
        Command {
            id: "footer-settings-open",
            label: key::CMD_FOOTER_SETTINGS_OPEN,
        },
        Command {
            id: "footer-settings-window",
            label: key::CMD_FOOTER_SETTINGS_WINDOW,
        },
        Command {
            id: "footer-settings-up",
            label: key::CMD_FOOTER_SETTINGS_UP,
        },
        Command {
            id: "footer-settings-down",
            label: key::CMD_FOOTER_SETTINGS_DOWN,
        },
        Command {
            id: "footer-settings-enter",
            label: key::CMD_FOOTER_SETTINGS_ENTER,
        },
        Command {
            id: "footer-hours",
            label: key::CMD_FOOTER_HOURS,
        },
        Command {
            id: "footer-hide-header",
            label: key::CMD_FOOTER_HIDE_HEADER,
        },
        Command {
            id: "footer-hide-shortcuts",
            label: key::CMD_FOOTER_HIDE_SHORTCUTS,
        },
        Command {
            id: "window-shortcuts",
            label: key::CMD_WINDOW_SHORTCUTS,
        },
    ],
    // `(sequence, command, argument)`: see `bindings::Row` for why the
    // sequence leaves the leader out and the label is not here.
    bindings: &[
        // `hjkl` as in vi, which is the convention Doom inherits. Upper case
        // MOVES instead of focusing — same key, same direction, one gesture
        // more.
        // THE ARROWS ARE NOT HERE, and that is deliberate.
        //
        // They are CHORDS — the leader HELD while an arrow is pressed — and a
        // chord does not go through this keymap: the footer only receives the keyboard
        // AFTER the leader is released (see the top of this module). What
        // resolves a chord is the compositor.
        //
        // So that there are not two truths again, the compositor's chord binds
        // are GENERATED from this same command table — see `actions::chords`
        // and `nix/compositor-binds.nix`.
        ("w h", "window-left", None),
        ("w l", "window-right", None),
        ("w k", "window-up", None),
        ("w j", "window-down", None),
        ("w H", "window-move-left", None),
        ("w L", "window-move-right", None),
        ("w K", "window-move-up", None),
        ("w J", "window-move-down", None),
        ("w m", "window-maximize", None),
        ("w f", "window-screen-full", None),
        ("w c", "window-center", None),
        ("w a", "window-consume", None),
        ("w e", "window-expel", None),
        ("w t", "window-float", None),
        // THE WINDOW MENU, which is what the modeline's app name opens: `o`
        // for OPTIONS, the whole of `w` in one grid for whoever holds a mouse.
        ("w o", "window-menu", None),
        // AND THE BUFFER'S OWN, under the same letter, by the owner's
        // decision: a buffer IS a window of the compositor (invariant 4), and
        // "buffer" and "window" were two groups for one thing. `w w` switches
        // (the doubled letter, the convention of every prefix's own view),
        // `n` next, `d` closes — Doom's `SPC w d` —, and `s` chooses the state
        // (grafted at runtime, see `Footer::state_key`). The `b` they used to
        // hang off is the brain's now.
        ("w w", "buffer-switch", None),
        ("w n", "buffer-next", None),
        ("w d", "buffer-close", None),
        // AND CLOSING IS ALSO ONE KEY, the owner's call: it is what a hand
        // does most after opening, and `w d` stays for whoever reads `w` as
        // the place of everything about a window.
        ("q", "buffer-close", None),
        // ---------------------------------------------------------------
        // TERMINAL — `RET`, and not under `w`.
        // ---------------------------------------------------------------
        // `w` is about windows in general, and opening a TERMINAL is one
        // program, so it has a prefix of its own (the owner's decision). The
        // doubled key is the external one — Ghostty, the terminal a person
        // reaches for — and `/` is the one this distro draws itself.
        ("RET RET", "buffer-terminal-external", None),
        ("RET /", "buffer-terminal", None),
        // ---------------------------------------------------------------
        // WORKSPACE — `TAB`, and not `e`.
        //
        // The key is every system's window switcher, and the gesture is
        // the same: TAB again shows the overview. Each action has SEVERAL
        // keys on purpose — vi's `j`/`k`, `[`/`]` for whoever comes from the
        // browser, arrows for whoever memorised neither, and `.`/`,`
        // which are the same keys as the move submenu's. Redundancy here
        // costs one table line and saves one decision per use.
        // ---------------------------------------------------------------
        ("TAB TAB", "workspace-overview", None),
        // THE REST OF WHAT A WORKSPACE CAN DO FROM HERE, which the group did
        // not have: every workspace at once (`a`, the same as `LEADER 0`),
        // back to the one before (`b`), and choosing one by the label the
        // compositor draws on it (`s`).
        ("TAB a", "workspace-all", None),
        ("TAB b", "wm-workspace-back", None),
        ("TAB s", "wm-jump-workspaces", None),
        // THE OVERVIEW OF EVERY WORKSPACE AT THE TOP LEVEL, on `0`, beside
        // `1`…`9` going to one (the ranges below): the owner's keys, and the
        // zero is the one that shows them all rather than a tenth place. It
        // is `scale_workspaces`, and not the `TAB TAB` overview: that one fits
        // the windows of ONE workspace, and with a single window open it
        // changes nothing on screen — which is how the owner read it.
        ("0", "workspace-all", None),
        ("TAB j", "workspace-next", None),
        ("TAB k", "workspace-previous", None),
        ("TAB ]", "workspace-next", None),
        ("TAB [", "workspace-previous", None),
        ("TAB <down>", "workspace-next", None),
        ("TAB <up>", "workspace-previous", None),
        ("TAB .", "workspace-next", None),
        ("TAB ,", "workspace-previous", None),
        // THE SUBMENU FOR TAKING THE WINDOW ALONG, with the SAME keys as the go one.
        // Whoever learned `TAB 3` gets `TAB m 3` for free.
        ("TAB m j", "workspace-move-next", None),
        ("TAB m k", "workspace-move-previous", None),
        ("TAB m ]", "workspace-move-next", None),
        ("TAB m [", "workspace-move-previous", None),
        ("TAB m <down>", "workspace-move-next", None),
        ("TAB m <up>", "workspace-move-previous", None),
        ("TAB m .", "workspace-move-next", None),
        ("TAB m ,", "workspace-move-previous", None),
        // `a` for APPLICATIONS, at the first level. It was free, and an
        // application launcher is a first-level thing: if it costs two
        // keys, the person goes back to opening everything through the terminal.
        (APPS_KEYS, "apps-open", None),
        // `m` FOR MISCELLANEOUS — the drawer for what belongs to no other one.
        //
        // The owner asked for it, and it is what frees `c` for the clipboard
        // alone. Two things live here so far: the screenshot, which used to
        // share a group with the clipboard on a word rather than on a job, and
        // the three browsers, which keep their own prefix INSIDE it.
        //
        // `m i` and not three loose letters: `i n`, `i u` and `i l` are one
        // trio reading one lexicon (see the `ARCHITECTURE`), and a drawer for
        // odds and ends is exactly where a named trio would stop looking like
        // one.
        // `k` for KEYBOARD — the one on the screen, for a finger. `s k` is
        // the physical keyboards' settings, which are another thing.
        ("m k", "onscreen-keyboard", None),
        ("m g", "gestures", None),
        // `b` for BROWSER — the person's own Chrome as a kiosk, without its
        // bars. Everything it does is then under `,` while it has the focus.
        ("m b", "kiosk-open", None),
        // `LEADER s m` — the memory panel. The machine already had a policy
        // against freezing and no screen ever said so; this is where it
        // becomes something one can look at, and loosen.
        ("s k", "keyboard-open", None),
        ("s m", "memory-open", None),
        // `LEADER s f` — formatting a disk. Under `s` because it is about the
        // SYSTEM's hardware and sits beside the memory panel, and `f` is free
        // there: the file manager's `f` is a prefix of its own, one level up.
        ("s f", "format-open", None),
        // `LEADER b` IS THE BRAIN, at the first level, by the owner's
        // decision: the machine's own agenda is looked at often enough to
        // earn one letter, and the buffer commands that held `b` moved under
        // `w` — a buffer IS a window of the compositor (invariant 4), and the
        // two groups were one thing spelled twice.
        //
        // `b` ALONE, and that is the whole of the group: the owner asked for
        // `LEADER b` to open the brain with nothing in between, and a
        // sequence cannot be a command and a prefix at once —
        // `PrefixWithCommand` is the collision the keymap refuses at BUILD
        // time, not at run time.
        //
        // So the two that used to sit beside it — `b a` and `b p`, the band's
        // answers — are gone from here. They live in the buffer's own keymap
        // as `, a` and `, p`, which is where everything else this screen does
        // already lived, and the band now draws THOSE notations. The cost,
        // said plainly: somebody at the keyboard answers a countdown with
        // `LEADER b` and then `, a`, where it used to be `LEADER b a`. The
        // band keeps both buttons for the mouse.
        (BRAIN_KEYS, "brain-open", None),
        // `t` for theme, under `s` because a theme is the SYSTEM's skin and not
        // any one buffer's: the same drawer as memory, format and the brain.
        ("s t", "theme-open", None),
        // `g` for GLANCE, at the first level: it was `d`, for date, until the
        // owner gave `d` to the development group. Bound through the constant
        // the sentences teach it by, so the two cannot drift apart.
        (GLANCE_KEYS, "world-glance", None),
        // `s q` — THE WAYS OUT, under SYSTEM at the owner's word.
        //
        // They held a letter of their own at the first level, and what they do
        // is what the machine does to ITSELF — which is the sentence the `s`
        // group is defined by, and the same drawer the power view, the update
        // and the theme already live in.
        ("s q q", "session-quit", None),
        // `LEADER :` — `M-x`'s key in Doom, which is where the concept
        // comes from. It used to be `x`, which in Doom is the scratch buffer and not the locator.
        // One key only: with both, the list showed the same thing on two
        // lines, and the list is the scarce resource here.
        (":", "mx-open", None),
        // `s r` — THE FOOTER'S OWN SETTINGS, on a letter of `s` still free:
        // the doubled key is the footer's door, `w` the window's.
        ("s r r", "footer-settings-open", None),
        ("s r w", "footer-settings-window", None),
        // `/ b` — THE BUILD SWITCH, the development group's second tool beside
        // `/ r`'s resizer: build the OTHER of the machine's own and a distro
        // checkout's reference build, and switch to it. Written out, not
        // through a constant, so the check that every key reaches an arm of
        // `run_with` reads it.
        ("/ b", "build-toggle", None),
        // `/ p` — THE BUILD PAGE, the hub the right-click on the glyph opens
        // too. Written out, not through a constant, so the check that every
        // key reaches an arm of `run_with` reads it.
        ("/ p", "build-open", None),
    ],
    groups: &[
        ("w", key::GROUP_WINDOW),
        ("RET", key::GROUP_TERMINAL),
        ("TAB", key::GROUP_WORKSPACE),
        ("TAB m", key::GROUP_WORKSPACE_MOVE),
        // `m` FOR MISCELLANEOUS, and `m i` for the browsers inside it. See the
        // bindings for why the trio keeps a prefix of its own.
        ("m", key::GROUP_MISCELLANEOUS),
        // `/` FOR DEVELOPMENT: tools for whoever builds this distro — the
        // screen resizer today, and more to come. It was `d` (which held the
        // glance before that, gone to `g`); `d` and `SPC` at the root are the
        // fast access's now, and this group leaves them free.
        ("/", key::GROUP_DEV),
        // `s` for SYSTEM — what the machine is doing to itself, as opposed to
        // what one is doing with it: memory, disks, the update, the theme —
        // and the network and Syncthing, which moved in from the first level.
        ("s", key::GROUP_SYSTEM),
        ("s q", key::GROUP_QUIT),
        ("s r", key::GROUP_FOOTER_SETTINGS),
    ],
    by_name: &[
        // THE WINDOW MENU OPENED AT ONE LEVEL: the modeline's marks run these,
        // and under the leader the same levels ARE groups (`w g`, `w y`, `w v`).
        "drawer-open",
        "trail-open",
        "selection-open",
        "brain-abort",
        "brain-proceed",
        // THE LAUNCHER'S LAST ROW: `RET` or a click on "search to install" runs
        // it, and a key of its own would be a second way to the row it already is.
        "apps-search-to-install",
        "example-open",
        // CALIBRATING THE GESTURES happens once per camera placement, and the
        // gestures themselves are one key away: a second sequence for it would
        // be a line of the list spent on something done once.
        "gestures-calibrate",
        // THE KIOSK'S THREE THAT TAKE WHAT ONLY THE POINTER POINTS AT: a tab by
        // its id, a tab dragged to a place, an extension by its id. The keyboard
        // reaches the same deeds through the kiosk's view — `RET` on a tab's row,
        // `K`/`J` and `{`/`}`, `RET` on an extension's row — and `LEADER :` runs
        // each with its argument.
        "kiosk-tab",
        "kiosk-tab-move",
        "kiosk-extension",
        // THE LEADER'S THREE SETS. Their key is the LEADER itself, pressed again
        // with the list open, which is no sequence — the leader is the
        // compositor's bind, not a key of the tree — and they are commands so the
        // pointer can click the hint that teaches the press, and `LEADER :` can
        // open the leader in any set.
        "leader-set-window",
        "leader-set-full",
        // THE FOOTER'S HIDDEN MODES: their key is a TAP of `Ctrl` at the leader's
        // root (`crate::hidden::next_mode`), which no tree can hold — a modifier
        // alone never enters the tree. The root's list grafts a row that runs the
        // cycle (`hidden::bars_row`), and `LEADER :` reaches the cycle and each
        // mode by name.
        "footer-hide-push",
        "footer-hide-cover",
        // THE STRIP'S THREE ABOUT A NOTICE. The strip CLICKS them and `LEADER :`
        // reaches them by name; their keys are the brain's own `, n`, `, x` and
        // `, z`, for the reason `brain-abort` and `brain-proceed` are `, a` and
        // `, p` — a sequence under `LEADER b` cannot be a command and a prefix.
        "brain-dismiss",
        "brain-snooze",
        "brain-act",
        // ── end of brain-ui ──

        // THE LEADER'S DEFAULT SET, by name: the third set the leader pressed
        // again walks round to (`Set::next`), and the one the full set's hint
        // teaches as a click.
        "leader-set-default",
        // THE BARS' THREE MODES IN TURN — normal, push, cover — which is what a
        // tap of `Ctrl` at the leader's root does (`hidden::next_mode`). A
        // modifier alone never enters a tree, so the root's list grafts a row
        // that runs it (`hidden::bars_row`).
        "footer-bars-cycle",
        // THE LEADER CLOSED BY NAME, the way its `Esc` closes it — for what has
        // no keyboard to press it with: the frame benchmark opens the leader
        // over the socket again and again, and each opening has to be one
        // (`nix/bench-frames.nix`). The leader pressed again walks the sets now.
        "leader-close",
        // THE MINIBUFFER'S DRAWER, opened or walked down and walked up or shut —
        // the right `Ctrl` with an arrow while the footer has the keyboard
        // (`Footer::fold_key`), and the compositor's transient bind while a hint
        // too long for the line is on it (`Footer::keep_the_fold_keys`). No
        // sequence: a drawer opened by `LEADER` would open onto the leader.
        "minibuffer-fold-open",
        "minibuffer-fold-close",
        // THE ALERT BAR'S CLOSE BUTTON, for the keyboard — `LEADER :` — since
        // the bar's own button is the pointer's (`crate::alert`).
        "alert-dismiss",
        // THE HOURS OF THE FREE MINIBUFFER, on or off by name: a row of the
        // footer's settings (`LEADER s r`), whose `RET` is the view's own key.
        "footer-hours",
        // THE "NO HEADER" MODE AND ITS SHORTCUTS, on or off by name: two more
        // rows of the footer's settings (`LEADER s r`). The shortcuts toggle is
        // also the glyph beside the minibuffer's chevron (`crate::surface`).
        "footer-hide-header",
        "footer-hide-shortcuts",
        // THE FOCUSED WINDOW'S SHORTCUTS, opened by name as a tap of `Alt` at
        // the leader's root opens them (`LEADER Alt`) — its row's click and
        // `LEADER :` both reach it.
        "window-shortcuts",
    ],
    generated: base_generated,
    reached: base_reached,
};

/// The digits as `&'static str`, so the binding can carry them.
///
/// `format!` would give a `String`, and the binding needs `'static`. Nine
/// literals cost less than leaking memory to fit the type.
const DIGITS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// What the footer's keys GENERATE when the keymap is assembled.
fn base_generated() -> Generated {
    // THE BRAIN MODULES' ACTIONS, each as a named command — read at run time
    // from the brain's catalogue, the person's modules' too, and naming no
    // module: `<module>-<action>`, labelled by the module's own
    // `cmd-<module>-<action>` (see `a_module_action`). The catalogue is kept
    // for the process, so its words are `'static` as they are.
    let mut commands: Vec<Command> = ronoctua_brain::catalogue::catalogue()
        .actions()
        .map(|(_, action)| Command {
            id: action.command.as_str(),
            label: action.label.as_str(),
        })
        .collect();

    // THE COMPOSITOR'S DEEDS, each as a named command.
    //
    // "Every OS action has a command" (section 13), and what the compositor
    // knows how to do is part of the OS. The list is DECLARED by us
    // (`assets/compositor-deeds.nix` → `nix/compositor-deeds.nix`), because
    // this compositor enumerates nothing: its commands are free text spread
    // across four tables in C. What keeps the declaration honest is the
    // generator, which refuses a verb the PINNED compositor's own command
    // table does not have — so a verb that goes away is a build failure rather
    // than a key that answers a parse error nobody sees.
    //
    // Every one is reachable by NAME, which is what section 13 asks for. And
    // since the owner asked for it, every one the compositor's OWN shipped
    // configuration binds has that key here too — as a chord, or inside one
    // of its modes (`actions::scroll_chords`, `actions::modes`), with the
    // distro's older chords keeping their keys where the two tables met.
    commands.extend(ronoctua_scroll::deeds::DEEDS.iter().map(|d| Command {
        id: d.id,
        label: d.key,
    }));

    // THE 1-TO-9 RANGES, generated.
    //
    // Eighteen hand-written lines would be eighteen places to get a digit
    // wrong, and the mistake would be mute: the key would lead to the wrong workspace. Here the
    // key and the argument come from the SAME number.
    //
    // The `0` goes LAST: it is the keyboard's convention, where zero closes the
    // row after nine. Nine is what `TAB 1..9` names, and `workspace number N`
    // creates the one it names — see `nix/modules/scroll.nix` — so the last
    // one is the ninth.
    const LAST: &str = "9";
    let bindings = (1..=9)
        .map(|n| (format!("TAB {n}"), "workspace-go", Some(DIGITS[n - 1])))
        // AND AT THE TOP LEVEL, one key shorter — the owner's `LEADER 1`…`9`,
        // the same numbers the modeline draws for the workspaces that hold
        // something.
        .chain((1..=9).map(|n| (n.to_string(), "workspace-go", Some(DIGITS[n - 1]))))
        .chain((1..=9).map(|n| (format!("TAB m {n}"), "workspace-move", Some(DIGITS[n - 1]))))
        .chain([
            ("TAB 0".to_owned(), "workspace-go", Some(LAST)),
            ("TAB m 0".to_owned(), "workspace-move", Some(LAST)),
        ])
        .collect();

    // AND THE WINDOW MENU'S, generated from the menu itself — see
    // `window_menu::leader_bindings`: every item it offers, on its own key,
    // under `w`. What `w` already binds to the SAME command is the menu
    // adopting the group's key, and is not bound twice; a key the two gave
    // different commands is a collision, which the tree refuses.
    let (menu_bindings, menu_groups) = crate::window_menu::leader_bindings();
    Generated {
        commands,
        bindings,
        adopting: menu_bindings.clone(),
        groups: menu_groups.clone(),
    }
}

/// What the footer's views run with keys of their own, which no sequence of
/// the tree does: the launcher's, the theme chooser's, the kiosk's view and
/// its `,` group (one table, `kiosk_view::ROOT`, and its two lists), and the
/// footer settings' (`footer_settings::OF_THE_VIEW`).
fn base_reached() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = OF_LAUNCHER_VIEW.iter().map(|(_, id)| *id).collect();
    out.extend(OF_THEME_VIEW.iter().map(|(_, id)| *id));
    out.extend(crate::kiosk_view::reached());
    out.extend(crate::footer_settings::reached());
    out.extend(crate::build_page::reached());
    out
}

/// The glance's key, BESIDE its binding.
///
/// A sentence that needs to teach it asks `bindings::taught` for its command
/// — never the letters written out. A key spelt into a `.ftl` ages the day the
/// key moves, and `n h` taught `n h` for weeks after it became `n n`.
pub const GLANCE_KEYS: &str = "g";

/// The launcher's key, BESIDE its binding — the software view points at it
/// for the system's own programs.
pub const APPS_KEYS: &str = "a";

/// The brain's page, BESIDE its binding — where the automatic updates are.
pub const BRAIN_KEYS: &str = "b";

/// The key that turns every workspace at once on and off, BESIDE its binding:
/// the line under the modeline prints it while that view is on.
pub const EVERY_WORKSPACE_KEYS: &str = "0";

/// The key that zooms the workspace in front out and back, likewise.
pub const OVERVIEW_KEYS: &str = "TAB TAB";

/// How `brain-dismiss` and `brain-snooze` are typed — inside the brain's
/// buffer, beside `, n` for the notice's first control and for the reason the
/// countdown's two are `, a` and `, p`: the strip PRINTS these, and a key
/// written in the table and again in the drawing would be two answers to
/// "which key puts it away".
pub const BRAIN_DISMISS_KEYS: &str = ", x";

/// See [`BRAIN_DISMISS_KEYS`].
pub const BRAIN_SNOOZE_KEYS: &str = ", z";

/// Whether a command is a brain module's action (`<module>-<action>`, from
/// the brain's catalogue, `ronoctua_brain::catalogue`).
///
/// # Why they are commands, and reached by name only
///
/// A notice's button is an action, and an action is a COMMAND of the registry
/// rather than a special case inside the strip: `LEADER :` runs it by name,
/// the brain's `, n` and the strip's click reach it through `brain-act`, and a
/// question an action asks comes back as the command's argument. The list is
/// the catalogue's, so a module's folder — the person's too — adds its actions
/// here without this file naming it, or being compiled again — and none of
/// them has a sequence, for the reason the Emacs
/// watch's button gave first: a key for it would be a key that does nothing on
/// every machine where there is nothing to say, which is almost all of them,
/// almost all of the time. So both orphan checks (`check-keymap` and the test
/// beside it) excuse them the way they excuse the ones reached by name only
/// (`bindings::by_name_only`).
#[must_use]
pub fn a_module_action(id: &str) -> bool {
    ronoctua_brain::catalogue::catalogue().action(id).is_some()
}

/// The action a module action's command names, out of the catalogue — what
/// `run_with` sends when the footer holds no mind to find it in.
///
/// A COMMAND THE REGISTRY LISTS ANSWERS, mind or not: before the brain has
/// spoken (or with no brain at all) it fell through to "does nothing yet",
/// which is the one thing it is not. With this it is sent, and the door says
/// what came back — the brain's answer, or that the brain is not answering.
#[must_use]
pub fn module_action(id: &str) -> Option<crate::brainlink::Acting> {
    ronoctua_brain::catalogue::catalogue()
        .action(id)
        .map(|(module, action)| crate::brainlink::Acting {
            module: module.id.clone(),
            action: action.id.clone(),
            command: id.to_owned(),
            asks: action.asks.clone(),
        })
}

/// A module action's command is found without a mind, and a command that is
/// not one is not.
///
/// THE DEFECT THIS CATCHES: `LEADER : <module>-<action>` before the brain had
/// spoken — or with no brain — answered "does nothing yet", because only the
/// mind was asked what the command was. Every action of the build's
/// catalogue, and no id is written here: naming a module outside its folder
/// is what `brain-modules-are-sealed` refuses.
#[cfg(test)]
mod module_action_tests {
    #[test]
    fn every_generated_action_is_found_without_a_mind() {
        let catalogue = ronoctua_brain::catalogue::catalogue();
        assert!(
            catalogue.actions().next().is_some(),
            "no module declares an action, and this proves nothing"
        );
        for (module, action) in catalogue.actions() {
            let found = super::module_action(&action.command)
                .unwrap_or_else(|| panic!("`{}` was not found", action.command));
            assert_eq!(found.module, module.id);
            assert_eq!(found.action, action.id);
            assert_eq!(found.command, action.command);
            assert_eq!(found.asks, action.asks);
            assert!(super::a_module_action(&action.command));
        }
        assert!(super::module_action("files-open").is_none());
        assert!(!super::a_module_action("files-open"));
    }
}

/// The launcher's keys — its table lives in the view, `crate::apps::OF_THE_VIEW`.
pub const OF_LAUNCHER_VIEW: &[(&str, &str)] = crate::apps::OF_THE_VIEW;

// THE CLIPBOARD VIEW'S KEYS USED TO LIVE HERE, and they live in the view now.
//
// It was the only view whose table was kept in `leader.rs`, and that was not a
// tidy exception: it was the symptom. With no module of its own the clipboard
// had nowhere to put a table, so it had no `hint()` either, and its action
// strip was written by hand from the LONG `cmd-…` labels — which is what the
// owner photographed. See the clipboard view's `OF_THE_VIEW`.

/// The commands the THEME CHOOSER reaches, with each one's key — one. `RET`
/// applies the pair under the two cursors; `<up>`/`<down>` preview,
/// `<left>`/`<right>` cross between the themes and the desk, and `Esc` puts
/// the committed pair back — the view's own gestures rather than commands,
/// as in every other list.
pub const OF_THEME_VIEW: &[(&str, &str)] = &[("RET", "theme-apply")];

#[cfg(test)]
mod tests {

    use super::*;

    /// A footer coming up on another's display MUST tell the other to leave.
    ///
    /// # The defect this test locks out
    ///
    /// The old code removed the socket and opened its own over the top, asserting in a
    /// comment that another footer "would be wrong to exist" — without doing
    /// anything about it. The previous one stayed alive, with its surface on screen
    /// and the keyboard on the seat; if it had been replaced **with the leader
    /// open**, it was left with `keyboard-interactivity: exclusive` forever.
    /// From then on the new footer drew the list and the old one up_to the
    /// keys: "the list opens and no key works, not even for a submenu".
    ///
    /// The test is about the MECHANISM, and not about the message: it asserts that the
    /// first receives [`Order::Quit`], which is what actually makes it leave.
    #[test]
    fn the_second_footer_tells_the_first_to_leave() {
        use smithay_client_toolkit::reexports::calloop::channel;

        let dir = std::env::temp_dir().join(format!("ronoctua-teste-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        let path = dir.join("controle.sock");

        let (sender1, receiver1) = channel::channel::<Order>();
        assert!(
            listen_on(&path, sender1),
            "the first footer has to manage to open the socket"
        );

        // THE SECOND. It is the one that has to ask for the place.
        let (sender2, _receiver2) = channel::channel::<Order>();
        assert!(
            listen_on(&path, sender2),
            "the second footer has to manage to take over the socket"
        );

        // calloop's channel does not block, and the first one's control thread
        // passes the order along as soon as it reads it — but "as soon as" is not "already". A
        // short wait with an early exit measures the mechanism, not the machine.
        let up_to = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            match receiver1.try_recv() {
                Ok(Order::Quit) => break,
                Ok(otherwise_one) => {
                    panic!("the first one got {otherwise_one:?} instead of leaving")
                }
                Err(_) if std::time::Instant::now() < up_to => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => panic!(
                    "the first footer was NOT told it had lost the display.\n\
                     \n\
             It is still alive, with its surface on screen and the keyboard in\n\
                     the seat. If it has the leader open, it is the one that receives\n\
             the keys the new footer is waiting for."
                ),
            }
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    /// FAILS IF: the leader pressed again stops walking the three sets in
    /// order — default, windows, the whole tree, and round to the default —
    /// or a set's key stops being the leader once per press, or a set's
    /// command stops being registered and handled.
    #[test]
    fn the_leader_again_walks_the_sets_round() {
        assert_eq!(Set::Footer.next(), Set::Window);
        assert_eq!(Set::Window.next(), Set::Full);
        assert_eq!(Set::Full.next(), Set::Footer);
        assert_eq!(Set::Footer.key_with("SUPER"), None);
        assert_eq!(Set::Window.key_with("SUPER").as_deref(), Some("SUPER"));
        assert_eq!(Set::Full.key_with("SUPER").as_deref(), Some("SUPER SUPER"));
        for set in Set::ALL {
            let id = set.command();
            assert!(
                BASE.commands.iter().any(|c| c.id == id),
                "`{id}` is not registered"
            );
            assert!(BASE.by_name.contains(&id), "`{id}` has a sequence?");
        }
    }

    /// FAILS IF: a modifier held while the POINTER acts — a Ctrl-click on a
    /// hint, a Ctrl-wheel over the list, a finger down — counts as a tap
    /// when it comes up, or a tap after the pointer acted stops counting; or
    /// the on-screen keyboard's tap cycles the bars on another modifier than
    /// `Ctrl`.
    #[test]
    fn a_modifier_held_while_the_pointer_acts_is_no_tap() {
        use smithay_client_toolkit::seat::keyboard::Keysym;
        for sym in [Keysym::Control_L, Keysym::Control_R] {
            let mut t = Tap::default();
            t.pressed(sym);
            t.pointer_acted();
            assert_eq!(
                t.released(sym),
                Tapped::Nothing,
                "{sym:?}: a chord with the hand"
            );
        }
        // THE POINTER BEFORE THE PRESS takes nothing from the tap after it.
        let mut t = Tap::default();
        t.pointer_acted();
        t.pressed(Keysym::Control_L);
        assert_eq!(t.released(Keysym::Control_L), Tapped::Bars);
        // AND THE ON-SCREEN KEYBOARD'S TAP answers what the keys answer.
        use crate::osk::Modifier;
        assert!(osk_tap_cycles_the_bars(Modifier::Ctrl));
        for other in [
            Modifier::Alt,
            Modifier::Shift,
            Modifier::AltGr,
            Modifier::Super,
        ] {
            assert!(!osk_tap_cycles_the_bars(other), "{other:?}");
        }
    }

    /// FAILS IF: a tap stops being a tap — a chord counts, a release with no
    /// press counts, `AltGr` counts, a `Ctrl` under a held `Alt` counts, or
    /// `Control_R` held for the paging arrows cycles the bars when it is let
    /// go — or a clean tap stops meaning what it means: `Ctrl` the bars,
    /// `Alt` the window's shortcuts.
    #[test]
    fn a_clean_tap_of_ctrl_cycles_and_of_alt_opens_the_shortcuts() {
        use smithay_client_toolkit::seat::keyboard::Keysym;
        let tapped = |keys: &[(bool, Keysym)]| {
            let mut t = Tap::default();
            let mut last = Tapped::Nothing;
            for (down, sym) in keys {
                if *down {
                    t.pressed(*sym);
                } else {
                    last = t.released(*sym);
                }
            }
            last
        };
        // A CLEAN TAP OF `Ctrl`, either hand, cycles the bars.
        assert_eq!(
            tapped(&[(true, Keysym::Control_L), (false, Keysym::Control_L)]),
            Tapped::Bars
        );
        // THE RIGHT CTRL TAPPED ALONE is a tap too...
        assert_eq!(
            tapped(&[(true, Keysym::Control_R), (false, Keysym::Control_R)]),
            Tapped::Bars
        );
        // ...and HELD WITH AN ARROW it is the paging chord, and no tap.
        assert_eq!(
            tapped(&[
                (true, Keysym::Control_R),
                (true, Keysym::Right),
                (false, Keysym::Right),
                (false, Keysym::Control_R),
            ]),
            Tapped::Nothing
        );
        // A CLEAN TAP OF `Alt`, either hand, opens the window's shortcuts.
        for alt in [Keysym::Alt_L, Keysym::Alt_R] {
            assert_eq!(
                tapped(&[(true, alt), (false, alt)]),
                Tapped::Shortcuts,
                "{alt:?}"
            );
        }
        // A KEY IN BETWEEN is a chord.
        assert_eq!(
            tapped(&[
                (true, Keysym::Control_L),
                (true, Keysym::n),
                (false, Keysym::Control_L),
            ]),
            Tapped::Nothing
        );
        // A RELEASE WITH NO PRESS: the leader's own `Ctrl` and `Alt` in the
        // nested loop, pressed before the footer had the keyboard.
        assert_eq!(
            tapped(&[(false, Keysym::Control_L), (false, Keysym::Alt_L)]),
            Tapped::Nothing
        );
        // TWO MODIFIERS AT ONCE is a chord, whichever came first.
        assert_eq!(
            tapped(&[
                (true, Keysym::Control_L),
                (true, Keysym::Alt_L),
                (false, Keysym::Alt_L),
                (false, Keysym::Control_L),
            ]),
            Tapped::Nothing
        );
        assert_eq!(
            tapped(&[
                (true, Keysym::Alt_L),
                (true, Keysym::Control_L),
                (false, Keysym::Control_L),
            ]),
            Tapped::Nothing
        );
        // `AltGr` IS NOTHING HERE.
        assert_eq!(
            tapped(&[
                (true, Keysym::ISO_Level3_Shift),
                (false, Keysym::ISO_Level3_Shift),
            ]),
            Tapped::Nothing
        );
        // A REPEAT OF THE SAME MODIFIER is still one tap.
        assert_eq!(
            tapped(&[
                (true, Keysym::Control_L),
                (true, Keysym::Control_L),
                (false, Keysym::Control_L),
            ]),
            Tapped::Bars
        );
        // AND A TAP AFTER A TAP: each release answers its own.
        assert_eq!(
            tapped(&[
                (true, Keysym::Alt_L),
                (false, Keysym::Alt_L),
                (true, Keysym::Control_L),
                (false, Keysym::Control_L),
            ]),
            Tapped::Bars
        );
    }

    /// FAILS IF: the leader's hint at the root stops teaching the NEXT set as
    /// a click — the pointer has no other way to a set (invariant 19) — or
    /// teaches it with any key but the leader's own label, or a set the
    /// leader is already in.
    #[test]
    fn the_root_teaches_the_next_set_as_a_click() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("the base language");
        let keys = |set: Set| -> Vec<(String, Option<crate::chrome::Act>)> {
            set_hint(set, "SUPER", &texts)
                .into_iter()
                .filter(crate::chrome::Piece::is_a_key)
                .map(|p| (p.text, p.press))
                .collect()
        };
        use crate::chrome::Act;
        for (set, next) in [
            (Set::Footer, "leader-set-window"),
            (Set::Window, "leader-set-full"),
            (Set::Full, "leader-set-default"),
        ] {
            assert_eq!(
                keys(set),
                [("SUPER".to_owned(), Some(Act::Run(next)))],
                "{set:?}"
            );
        }
        // EVERY PIECE SAYS WHICH SHORTCUT IT IS, so the pointer resting on the
        // words reads the command's sentence too — and the words are a word.
        for set in Set::ALL {
            for p in set_hint(set, "SUPER", &texts) {
                let id = p.id.expect("a piece of a shortcut with no id");
                assert!(texts.has(&format!("cmd-{id}")), "`{id}` has no sentence");
                assert!(!p.text.contains('['), "a missing word: {}", p.text);
            }
        }
    }

    /// FAILS IF: the list waits with the bars visible or when the pointer
    /// opened the leader, or does not wait when a key opened it with the bars
    /// hidden.
    #[test]
    fn the_list_waits_only_for_a_key_with_the_bars_hidden() {
        assert_eq!(Patience::at_opening(true, true, 7), Patience::Waiting(7));
        assert_eq!(Patience::at_opening(true, false, 7), Patience::Shown);
        assert_eq!(Patience::at_opening(false, true, 7), Patience::Shown);
        assert_eq!(Patience::at_opening(false, false, 7), Patience::Shown);
        assert!(Patience::Waiting(1).withheld());
        assert!(!Patience::Shown.withheld());
    }

    /// FAILS IF: a key does not start the wait again, a timer armed before
    /// that key shows the list anyway, the right timer does not show it, or
    /// a shown list goes back to waiting on a key.
    #[test]
    fn the_wait_counts_from_the_last_key() {
        let mut p = Patience::at_opening(true, true, 1);
        assert_eq!(p.after_a_key(2), Some(2));
        // THE FIRST TIMER IS STALE: the key after it moved the count.
        assert!(!p.ran_out(1));
        assert!(p.withheld());
        assert!(p.ran_out(2));
        assert_eq!(p, Patience::Shown);
        // ONCE SHOWN, EVERY LEVEL AFTER IT IS SHOWN AT ONCE.
        assert_eq!(p.after_a_key(3), None);
        assert!(!p.ran_out(3));
        assert_eq!(p, Patience::Shown);
    }

    /// FAILS IF: a new leader opens in any set but the footer's, whatever set
    /// the last sequence ended in.
    #[test]
    fn every_opening_is_in_the_footers_set() {
        let mut l = Leader::default();
        assert_eq!(l.set_now(), Set::Footer);
        l.set = Set::Window;
        l.activate();
        assert_eq!(l.set_now(), Set::Footer);
    }

    /// FAILS IF: the base64 the askpass context is sent as is decoded with the
    /// wrong alphabet or bit order — the pair the helper encodes with
    /// (`nix/ssh-askpass.rs`) and this decodes with would then disagree — or a
    /// character the alphabet does not have is not refused.
    #[test]
    fn the_askpass_context_base64_decodes_as_the_helper_encodes_it() {
        // Hand-verified vectors covering the three remainders (0, 1, 2 bytes
        // over a triple) and a multibyte char, as the helper would emit them.
        assert_eq!(un_base64("").as_deref(), Some(""));
        assert_eq!(un_base64("TQ").as_deref(), Some("M"));
        assert_eq!(un_base64("TWE").as_deref(), Some("Ma"));
        assert_eq!(un_base64("TWFu").as_deref(), Some("Man"));
        // `·` is U+00B7 — the separator the helper puts between shell and tty.
        assert_eq!(un_base64("enNoIMK3").as_deref(), Some("zsh ·"));
        // Padding is tolerated, and a character off the alphabet is refused.
        assert_eq!(un_base64("TWE=").as_deref(), Some("Ma"));
        assert_eq!(un_base64("not base64!"), None);
    }
}
