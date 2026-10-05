//! The leader, the default keymap and the control channel — section 7.4.
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
use std::path::PathBuf;
use std::thread;

use anyhow::{Context as _, Result};
use ronoctua_cmd::{Command, Registry};
use ronoctua_i18n::key;
use ronoctua_keymap::{Binding, GroupNode, Tree, parse_sequence};
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

/// This session's socket path.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent. There is no fallback to `/tmp`: a control
/// socket in a shared directory is attack surface, and section 9
/// does not accept that even in a test.
pub fn socket_path() -> Result<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .context("XDG_RUNTIME_DIR is not set; without it there is nowhere to put the socket")?;
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".into());
    Ok(PathBuf::from(dir).join(format!("ronoctua-{display}.sock")))
}

/// A BUFFER's socket, one per process, named by PID.
///
/// # Why the PID, and not the window id
///
/// It is the only name both sides already know without being told. The footer
/// learns it from `ronoctua_compositor::Window::pid` on the focused window — the same
/// field the modeline's context segment uses to find the git branch — and the
/// buffer's process knows its own. Anything else would need a handshake, and a
/// handshake is a third thing that can be out of date.
///
/// It sits next to the footer's control socket, in `XDG_RUNTIME_DIR`, and speaks
/// the same line protocol. A second grammar in the same directory would be two
/// things to keep in step.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent — same reason as [`socket_path`]: there is no
/// fallback to `/tmp`.
pub fn buffer_socket_path(pid: u32) -> Result<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .context("XDG_RUNTIME_DIR is not set; without it there is nowhere to put the socket")?;
    Ok(PathBuf::from(dir).join(format!("ronoctua-buffer-{pid}.sock")))
}

/// Asks a buffer for its bindings: `(sequence, command id)` per entry.
///
/// Empty when there is no buffer listening on that PID, and that is a normal
/// answer rather than an error: most focused windows are not our buffers, and
/// `SPC m` has to be ABSENT for those instead of showing a failure.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent.
pub fn buffer_keymap(pid: u32) -> Result<Vec<(String, String)>> {
    buffer_keymap_at(&buffer_socket_path(pid)?)
}

/// The same, at a given path.
///
/// Split off for the same reason as [`listen_on`]: `XDG_RUNTIME_DIR` is
/// process-global, and a test that changes it races every other test in the
/// binary.
pub fn buffer_keymap_at(path: &std::path::Path) -> Result<Vec<(String, String)>> {
    let Ok(mut stream) = UnixStream::connect(path) else {
        return Ok(Vec::new());
    };
    // A buffer that stops answering must not take the footer's frame with it.
    // Everything on the other side is a `HashSet` lookup and a `writeln!`.
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
    if writeln!(stream, "keymap").is_err() {
        return Ok(Vec::new());
    }
    let mut answer = String::new();
    if std::io::Read::read_to_string(&mut stream, &mut answer).is_err() {
        return Ok(Vec::new());
    }
    // `rsplit_once`, and the test is what said so: the SEQUENCE carries spaces
    // (`m f s`) and the id does not, so what separates them is the LAST space.
    // Splitting on the first gave `("m", "f s save-file")` — a plausible
    // pair that would have listed one key and run nothing.
    Ok(answer
        .lines()
        .filter_map(|l| l.trim().rsplit_once(' '))
        .map(|(keys, command)| (keys.to_owned(), command.to_owned()))
        .collect())
}

/// What a buffer answers about its states: which one is in effect, and the list
/// of `(id, letter)` it declared.
pub type States = (String, ronoctua_protocol::Tone, Vec<(String, char)>);

/// A tone as the control socket and the states socket spell it — one word,
/// after the state's id.
#[must_use]
pub const fn tone_word(tone: ronoctua_protocol::Tone) -> &'static str {
    use ronoctua_protocol::Tone;
    match tone {
        Tone::Standard => "standard",
        Tone::Accent => "accent",
        Tone::Success => "success",
        Tone::Warning => "warning",
        Tone::Error => "error",
    }
}

/// [`tone_word`] read back. AN UNKNOWN WORD — or none, from a buffer older
/// than the tone on the wire — IS THE STANDARD ONE: the host's own rule
/// draws it, which is a colour that is never wrong, where dropping the state
/// would be a segment that vanished.
#[must_use]
pub fn tone_of(word: Option<&str>) -> ronoctua_protocol::Tone {
    use ronoctua_protocol::Tone;
    match word.map(str::trim) {
        Some("accent") => Tone::Accent,
        Some("success") => Tone::Success,
        Some("warning") => Tone::Warning,
        Some("error") => Tone::Error,
        _ => Tone::Standard,
    }
}

/// Asks a buffer which state it is in, and which ones it has.
///
/// Answers `(current, [(id, letter)])`, and `None` when there is no buffer
/// listening on that PID — which is the normal answer for most focused windows,
/// exactly as [`buffer_keymap`] returning empty is.
///
/// # Why the footer asks at all, when the buffer also pushes
///
/// The push covers every change while the window is focused, which is nearly all
/// of them. It does NOT cover a footer that restarted with buffers already open,
/// nor a buffer that changed state while another window had the focus. Asking on
/// each change of focus closes both, and costs one socket round trip per Alt-Tab.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent.
pub fn buffer_states(pid: u32) -> Result<Option<States>> {
    Ok(buffer_states_at(&buffer_socket_path(pid)?))
}

/// The same, at a given path. See [`buffer_keymap_at`].
#[must_use]
pub fn buffer_states_at(path: &std::path::Path) -> Option<States> {
    let Ok(mut stream) = UnixStream::connect(path) else {
        return None;
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
    if writeln!(stream, "states").is_err() {
        return None;
    }
    let mut answer = String::new();
    if std::io::Read::read_to_string(&mut stream, &mut answer).is_err() {
        return None;
    }
    let mut lines = answer.lines();
    // `<id> [tone]`: the tone is a word after the id, absent from a buffer
    // that predates it.
    let mut first = lines.next()?.split_whitespace();
    let now = first.next()?.to_owned();
    let tone = tone_of(first.next());
    let states = lines
        .filter_map(|l| {
            let (id, letter) = l.trim().split_once(' ')?;
            Some((id.to_owned(), letter.chars().next()?))
        })
        .collect();
    Some((now, tone, states))
}

/// Tells a buffer to go to a state, or to rotate.
///
/// # Errors
///
/// Nobody listening there, or the buffer refused the id.
pub fn buffer_state_at(path: &std::path::Path, order: &str) -> Result<()> {
    let mut stream = UnixStream::connect(path)
        .with_context(|| format!("no buffer listening at {}", path.display()))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
    writeln!(stream, "{order}").context("could not write to the buffer's socket")?;
    let mut answer = String::new();
    std::io::Read::read_to_string(&mut stream, &mut answer).context("the buffer did not answer")?;
    anyhow::ensure!(
        answer.trim() == "ok",
        "the buffer refused `{order}`: {}",
        answer.trim()
    );
    Ok(())
}

/// [`buffer_state_at`], by PID.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent, or nobody listening on that PID.
pub fn buffer_state(pid: u32, order: &str) -> Result<()> {
    buffer_state_at(&buffer_socket_path(pid)?, order)
}

/// Runs one of a buffer's own commands, in the buffer's process.
///
/// This is the half that makes listing honest: without it `SPC m` would be a
/// screen teaching keys that do nothing.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent, or nobody listening on that PID.
pub fn buffer_command(pid: u32, id: &str) -> Result<()> {
    buffer_command_at(&buffer_socket_path(pid)?, id)
}

/// The same, at a given path. See [`buffer_keymap_at`].
///
/// # Errors
///
/// Nobody listening there, or the buffer refused the id.
pub fn buffer_command_at(path: &std::path::Path, id: &str) -> Result<()> {
    let mut stream = UnixStream::connect(path)
        .with_context(|| format!("no buffer listening at {}", path.display()))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
    writeln!(stream, "command {id}").context("could not write to the buffer's socket")?;
    // THE ANSWER IS READ, and not assumed. The buffer refuses an id it did not
    // declare, and a caller that never looked would report success for a key
    // that did nothing — the exact failure this whole path exists to avoid.
    let mut answer = String::new();
    std::io::Read::read_to_string(&mut stream, &mut answer).context("the buffer did not answer")?;
    anyhow::ensure!(
        answer.trim() == "ok",
        "the buffer refused `{id}`: {}",
        answer.trim()
    );
    Ok(())
}

/// Sends an order to the daemon and exits. It is what `ronoctua-shell leader` does.
///
/// # Errors
///
/// There is no daemon listening, or the socket cannot be written to.
pub fn send_it(order: &str) -> Result<()> {
    let path = socket_path()?;
    let mut s = UnixStream::connect(&path).with_context(|| {
        format!(
            "there is no footer listening at {}.\n\
             \n\
             `ronoctua-shell leader` tells a footer that is ALREADY running; it does not start one.",
            path.display()
        )
    })?;
    writeln!(s, "{order}").context("could not write to the control socket")?;
    Ok(())
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

/// WHICH OF THE THREE TREES a sequence walks, chosen by pressing the LEADER
/// again while its list is open.
///
/// # Why there are three
///
/// The owner's request (2026-09-28): the key that used to open an app's
/// SUBMENU opens the app itself, in the footer — `LEADER s n` is the network
/// where it was `LEADER s n n`. The same key one set further opens the app's
/// WINDOW, and one further still the whole tree of before is there, every
/// submenu as it was. The two doors of an app were only ever a convention of
/// NAMES — the doubled letter is the footer, `w` the window — and that
/// convention is what decides which groups fold: see [`collapse`].
///
/// # Why the leader again, and no modifier (2026-09-30)
///
/// The sets were a TAP of `Alt` or `Ctrl` after the leader. The owner took
/// both away from them: a tap of `Ctrl` now cycles the bars' modes
/// (`crate::hidden::next_mode`), and `Alt` chooses nothing. The leader itself
/// is the one key every hand already has under it when the list is open, and
/// pressing it again walks the sets in order — default, windows, the whole
/// tree, and back to the default ([`Set::next`]) — so `LEADER LEADER s n` is
/// the network's window and `LEADER LEADER LEADER s n a` the aeroplane.
/// Holding a modifier while typing the letter was never an answer: `M-` is the
/// compositor's family (`AGENTS.md`), and in the nested loop the leader's own
/// `Ctrl` and `Alt` are HELD by construction when the sequence starts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Set {
    /// `LEADER` alone: a group with two doors opens its footer door.
    #[default]
    Footer,
    /// `LEADER LEADER`: the same key opens the app's window.
    Window,
    /// `LEADER LEADER LEADER`: the whole tree, every submenu as it was.
    Full,
}

/// The key a lone TAP of which cycles the bars' modes at the leader's root —
/// the grafted row of the root's list ([`crate::hidden::bars_row`]) and the
/// sentences that teach it.
///
/// A constant beside the tables and never text in a `.ftl`
/// (`keys-are-never-text`): it is a key's name, the same in every language,
/// as `SPC` is.
pub const BARS_TAP_KEY: &str = "CTRL";

/// The letter a group's SECOND door hangs off — `w`, for window, the letter
/// every view with a window uses (`. w`, `s n w`, `s u w`). The first door is
/// the group's own last key doubled. [`folds`] reads both.
pub const WINDOW_DOOR_KEY: &str = "w";

impl Set {
    /// How many times the leader is pressed AGAIN to reach this set from a
    /// fresh opening: none for the default.
    #[must_use]
    pub const fn presses(self) -> usize {
        match self {
            Self::Footer => 0,
            Self::Window => 1,
            Self::Full => 2,
        }
    }

    /// The key a sequence in this set is shown with after the leader's, when
    /// there is one — the leader again, once per [`Self::presses`], under the
    /// label `leader` it goes by (`SUPER`, or `CTRL ALT SPC` in the nested
    /// loop). The default set is the leader alone.
    #[must_use]
    pub fn key_with(self, leader: &str) -> Option<String> {
        (self.presses() > 0).then(|| vec![leader; self.presses()].join(" "))
    }

    /// The set the leader pressed again goes to: default, windows, the whole
    /// tree, and round to the default.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Footer => Self::Window,
            Self::Window => Self::Full,
            Self::Full => Self::Footer,
        }
    }

    /// The command that switches to this set — what the POINTER runs, since
    /// a click cannot press the leader (invariant 19). Its id is also the key
    /// of the short word the hint line draws beside the key.
    #[must_use]
    pub const fn command(self) -> &'static str {
        match self {
            Self::Footer => "leader-set-default",
            Self::Window => "leader-set-window",
            Self::Full => "leader-set-full",
        }
    }

    /// The three, in the order teaching looks: the default first.
    pub const ALL: [Self; 3] = [Self::Footer, Self::Window, Self::Full];
}

/// The three trees, built once and each verified.
///
/// Three [`Tree`]s and not one tree with a switch inside: each goes through
/// [`Tree::build_it`], so a collision the folding would create — a folded
/// group landing on a key that is a prefix of something — is a BUILD error
/// in the set it happens in, exactly as it is in the full one.
#[derive(Debug, Default)]
pub struct Trees {
    /// [`Set::Footer`]'s — the one a sequence starts in.
    pub footer: Tree,
    /// [`Set::Window`]'s.
    pub window: Tree,
    /// [`Set::Full`]'s: `BINDINGS` and `GROUPS` as written, unchanged.
    pub full: Tree,
}

impl Trees {
    /// The tree of one set.
    #[must_use]
    pub const fn of(&self, set: Set) -> &Tree {
        match set {
            Set::Footer => &self.footer,
            Set::Window => &self.window,
            Set::Full => &self.full,
        }
    }

    /// The three trees of one list of bindings and groups.
    ///
    /// # Errors
    ///
    /// EVERY collision of every set, each with the set it is in — the rule
    /// [`Tree::build_it`] keeps, of fixing a keymap in one build rather than
    /// in three.
    pub fn build(
        bindings: &[Binding],
        groups: &[GroupNode],
    ) -> std::result::Result<Self, Vec<(Set, ronoctua_keymap::Collision)>> {
        let mut failed = Vec::new();
        let mut one = |set: Set| {
            let (b, g) = collapse(bindings, groups, set);
            Tree::build_it(&b, &g)
                .map_err(|all| failed.extend(all.into_iter().map(|c| (set, c))))
                .ok()
        };
        let (footer, window, full) = (one(Set::Footer), one(Set::Window), one(Set::Full));
        match (footer, window, full) {
            (Some(footer), Some(window), Some(full)) if failed.is_empty() => Ok(Self {
                footer,
                window,
                full,
            }),
            _ => Err(failed),
        }
    }
}

/// A group that FOLDS into one key: its sequence, and its two doors.
#[derive(Debug, Clone)]
pub struct Fold {
    /// The group's own sequence — the key it becomes (`s n`).
    pub group: Vec<ronoctua_keymap::Key>,
    /// The footer's door: the group's last key doubled (`s n n`).
    pub footer: Binding,
    /// The window's door: [`WINDOW_DOOR_KEY`] under the group (`s n w`).
    pub window: Binding,
}

/// Every group that folds in [`Set::Footer`] and [`Set::Window`].
///
/// # The rule, and why it is read and not listed
///
/// A group G folds when BOTH `G <G's last key>` and `G w` are bindings, to two
/// DIFFERENT commands: that is the two-door convention, and it holds, with no
/// exception, for every app that has a footer and a window. A list of the
/// groups kept by hand beside the tables would be one more thing to forget
/// when an app gains a window; the test `the_collapsed_groups_are_these` pins
/// what the rule answers today, so a new group that folds is a decision
/// somebody makes on purpose.
///
/// `w` itself does not fold, and the rule says why without an exception: its
/// doubled key IS its `w` (`w w`), one binding and not two doors. A group
/// with only one door (`RET`, `TAB`, `s q`), or with none (`m`, `s`, `d`),
/// does not fold either.
///
/// # And nothing under `w` folds
///
/// `w` IS the window's group, and the groups under it are the window menu's
/// levels, generated from the menu with the menu's own letters
/// (`window_menu::leader_bindings`). There `w` is one more letter of a level
/// and never a second door: `w v v` selects the window and `w v w` its whole
/// workspace, two deeds of one level that the rule above, read alone, took
/// for a footer and a window — the pinned test caught it.
#[must_use]
pub fn folds(bindings: &[Binding], groups: &[GroupNode]) -> Vec<Fold> {
    let Ok(window_key) = ronoctua_keymap::Key::parse(WINDOW_DOOR_KEY) else {
        return Vec::new();
    };
    let bound = |seq: &[ronoctua_keymap::Key]| bindings.iter().find(|b| b.sequence == seq);
    let mut out: Vec<Fold> = Vec::new();
    for g in groups {
        let Some(last) = g.sequence.last().copied() else {
            continue;
        };
        if g.sequence.first() == Some(&window_key) || out.iter().any(|f| f.group == g.sequence) {
            continue;
        }
        let doubled: Vec<_> = g.sequence.iter().copied().chain([last]).collect();
        let windowed: Vec<_> = g.sequence.iter().copied().chain([window_key]).collect();
        if doubled == windowed {
            continue;
        }
        let (Some(footer), Some(window)) = (bound(&doubled), bound(&windowed)) else {
            continue;
        };
        if (footer.command, footer.argument) == (window.command, window.argument) {
            continue;
        }
        out.push(Fold {
            group: g.sequence.clone(),
            footer: footer.clone(),
            window: window.clone(),
        });
    }
    out
}

/// The bindings and groups of one set, derived from the full ones.
///
/// [`Set::Full`] is the input unchanged. In the other two, every group that
/// [`folds`] becomes ONE binding — to its footer's door in [`Set::Footer`],
/// to its window's in [`Set::Window`] — and everything under it, bindings and
/// groups, leaves that set: the other keys of a folded group (`s n a`, the
/// aeroplane) are still two more presses of the leader away. A fold inside a folded group
/// is the outer one's to take away.
#[must_use]
pub fn collapse(
    bindings: &[Binding],
    groups: &[GroupNode],
    set: Set,
) -> (Vec<Binding>, Vec<GroupNode>) {
    if set == Set::Full {
        return (bindings.to_vec(), groups.to_vec());
    }
    let all = folds(bindings, groups);
    // THE OUTERMOST ONLY: a fold under another fold is already gone.
    let outer: Vec<&Fold> = all
        .iter()
        .filter(|f| {
            !all.iter()
                .any(|o| o.group.len() < f.group.len() && f.group.starts_with(&o.group))
        })
        .collect();
    let under = |seq: &[ronoctua_keymap::Key]| outer.iter().any(|f| seq.starts_with(&f.group));
    let mut out: Vec<Binding> = bindings
        .iter()
        .filter(|b| !under(&b.sequence))
        .cloned()
        .collect();
    for f in &outer {
        let door = if set == Set::Footer {
            &f.footer
        } else {
            &f.window
        };
        out.push(Binding {
            sequence: f.group.clone(),
            ..door.clone()
        });
    }
    let kept = groups
        .iter()
        .filter(|g| !under(&g.sequence))
        .cloned()
        .collect();
    (out, kept)
}

/// The keys that run `command` — with `argument` — the shortest way from the
/// DEFAULT set, written as a sentence teaches them after `crate::leader()`.
///
/// The footer's own sequence when it has one (`t`, the queue); else the
/// window set's, after the leader again (`SUPER .`, the manager's window, so
/// `LEADER SUPER .` on screen); else the full tree's, after the leader twice
/// more (`SUPER SUPER s b y`, answering a pairing). A key taught any other way
/// is a key that does something else when typed — `t t` in the default set is
/// the queue and then a key that leads nowhere.
///
/// `None` for a command no sequence reaches.
#[must_use]
pub fn taught_keys(trees: &Trees, command: &str, argument: Option<&str>) -> Option<String> {
    taught_keys_with(trees, command, argument, &crate::leader())
}

/// [`taught_keys`] under a given leader label — the one the tests pin.
#[must_use]
pub fn taught_keys_with(
    trees: &Trees,
    command: &str,
    argument: Option<&str>,
    leader: &str,
) -> Option<String> {
    Set::ALL.into_iter().find_map(|set| {
        let keys = ronoctua_keymap::notation(&trees.of(set).sequence_of(command, argument)?);
        Some(match set.key_with(leader) {
            Some(k) => format!("{k} {keys}"),
            None => keys,
        })
    })
}

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

    /// A key came UP: `true` when it ends a clean tap of `Ctrl` — the one
    /// tap that means something ([`cycles_the_bars`]).
    pub fn released(&mut self, sym: smithay_client_toolkit::seat::keyboard::Keysym) -> bool {
        match self.armed {
            Some(held) if held == sym => {
                self.armed = None;
                cycles_the_bars(sym)
            }
            _ => false,
        }
    }
}

/// Whether a tap of the ON-SCREEN keyboard's modifier cycles the bars — the
/// same answer the physical keys give ([`cycles_the_bars`]): `Ctrl`, and
/// neither `Alt` nor `AltGr`.
#[must_use]
pub const fn osk_tap_cycles_the_bars(m: crate::osk::Modifier) -> bool {
    matches!(m, crate::osk::Modifier::Ctrl)
}

/// Whether this key, pressed alone, ARMS a tap: the two modifiers a tap was
/// ever made of. `Alt` arms and answers nothing when it comes up, so that a
/// `Ctrl` under a held `Alt` is a chord and not a tap.
fn arms(sym: smithay_client_toolkit::seat::keyboard::Keysym) -> bool {
    use smithay_client_toolkit::seat::keyboard::Keysym;
    matches!(
        sym,
        Keysym::Alt_L | Keysym::Alt_R | Keysym::Control_L | Keysym::Control_R
    )
}

/// Whether a clean tap of this key cycles the bars: either `Ctrl`.
fn cycles_the_bars(sym: smithay_client_toolkit::seat::keyboard::Keysym) -> bool {
    use smithay_client_toolkit::seat::keyboard::Keysym;
    matches!(sym, Keysym::Control_L | Keysym::Control_R)
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

/// The commands reachable by NAME and by name only.
///
/// # Why the list is here and not at the one place that reads it
///
/// Because two places read it: `check-keymap`, which is what `nix flake check`
/// runs, and the test beside it, which is what `cargo test` runs. They were
/// two literals, and the day they disagreed would be the day one of them
/// passed on a keymap the other refuses.
///
/// # Why each one is on it
///
/// `brain-abort` and `brain-proceed` are the band's two answers. They had
/// keys — `LEADER b a` and `LEADER b p` — until `LEADER b` became the brain
/// itself, and a sequence cannot be a command and the prefix of another. What
/// they do lives in the brain's buffer as `, a` and `, p`; they stay in the
/// registry because the band CLICKS them and `LEADER :` reaches them by name.
///
/// It is a LIST OF NAMES and not a prefix on purpose: `brain-` as a prefix
/// would exempt the next brain command somebody forgets to bind.
pub const BY_NAME_ONLY: &[&str] = &[
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
    // THE SOFTWARE VIEW'S QUESTIONS: rows of the view run them. The web
    // app's address and name open its door's line asking for its words
    // (`ask-<id>`) — the footer's minibuffer or the window's header — and the
    // answer comes back as the same command's argument; the icon's image and
    // the file to install open the view's file picker instead
    // (`software::Picking`), and a path given as the argument is the answer.
    "software-web-address",
    "software-web-name",
    "software-icon-path",
    "software-file-path",
    // AND WHAT A NOTIFICATION OFFERS once something was installed: open it.
    // A key of its own would open whatever was installed last, which is a
    // key that means something different every day.
    "software-launch-latest",
    "example-open",
    // THE MACHINE'S CLOCK, asked by `RET` on its rows in `LEADER s l`: each
    // opens its door's line asking for its words (`ask-<id>`), and the answer
    // comes back as the same command's argument. A key of their own would
    // ask for a date on a machine whose clock is kept automatically, which
    // the rows refuse on sight.
    "locale-system-date",
    "locale-system-time",
    "locale-system-zone",
    // THE CAMERA'S QUESTIONS: rows of `LEADER s c` ask them where the view is
    // open — the footer's minibuffer or the window's own line — and `M-x`
    // reaches them by name; each asks for its words (`ask-<id>`).
    "camera-scene-add",
    "camera-scene-rename",
    "camera-script-add",
    "camera-stream-url",
    "camera-stream-key",
    // THE PLAYER'S TWO SWITCHES: the transport draws them as buttons, the
    // view as rows, and a sequence of their own would be two keys for a
    // setting touched once in a while.
    "player-shuffle",
    "player-repeat",
    // PAIRING WITH ANOTHER MACHINE is a row of the machines' page (`p m`),
    // done once per machine: a key of its own would be a second way to the
    // row the page already offers.
    "player-pair",
    // SAVING WHAT PLAYS AS A SET, AND THE QUEUE: rows of the view's "More
    // features", each a name and a folder asked in turn — done now and then,
    // and a sequence each would be two keys for it.
    "player-save-set",
    "player-save-queue",
    // EVERY AMBIENT SOUND OFF: a row of their page (`p e`), where a person
    // sees what is on before silencing it — a sequence of its own would be a
    // key that acts on sounds nobody is looking at.
    "player-ambience-silence",
    // WHAT THE INTERNET SAYS OF WHAT PLAYS, and the page of what asks it:
    // rows of the view — the Metadata's, "More features" — each asking only
    // when a row naming its hosts is pressed; a sequence each would be four
    // keys for pages visited now and then (the plan's "by name only").
    "player-suggestions",
    "player-find-subtitles",
    "player-find-lyrics",
    "player-internet",
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
    // WHAT A DOOR OF THE SEARCH HANDS THE FOOTER (`fast_access_window::Out`):
    // each takes the argument a key could not give.
    "fast-access-launch",
    "fast-access-open-path",
    "fast-access-reveal",
    "fast-access-man",
    // THE REGISTRY LOGIN'S SECOND QUESTION (`containers::LOGIN_PASSWORD`):
    // the secret line the first answer opens, whose answer arrives as this
    // command's argument. A key of its own would ask for a password with no
    // login to give it to.
    "containers-login-password",
];

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

/// The sequence that opens the notification manager in the footer.
///
/// A constant for the reason the two above are, and the one every sequence a
/// sentence names keeps: a sentence once carried these keys as TEXT, and went
/// on teaching `n h` after the key became `n n`. What a sentence names is an
/// argument, never text (`keys-are-never-text`) — and since the leader has
/// three sets, it is [`taught_keys`]' answer and not the full tree's
/// sequence: in the default set `n` alone is the manager.
pub const NOTIF_OPEN_KEYS: &str = "n n";

/// The two sequences that answer a pairing question from anywhere, in the
/// full tree — the binding reads them.
///
/// The card that carries the question lives twenty seconds and the question
/// lives fifty-five, so the body names the keys — as ARGUMENTS, resolved
/// through `crate::leader()`, for the reason [`NOTIF_OPEN_KEYS`] gives. What
/// it names is [`taught_keys`]'s answer for the command and not this
/// constant: `s b` is the radio itself in the default set, so from there the
/// answer is the leader twice more and `s b y` (`SUPER SUPER s b y`).
pub const PAIRING_YES_KEYS: &str = "s b y";

/// The refusal's half of [`PAIRING_YES_KEYS`].
pub const PAIRING_NO_KEYS: &str = "s b n";

/// The default keymap and the commands it references.
///
/// # Why both together
///
/// A shortcut pointing at a non-existent command is the bug section 13
/// exists to prevent. Building both in the same function lets the
/// `keymap-consistente` check verify that **every binding has a command** — which
/// neither of the two structures guarantees on its own.
///
/// # Errors
///
/// A keymap collision, a malformed command id, or a binding pointing at a
/// command that does not exist. All three are programming errors.
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

pub fn default_of() -> Result<(Trees, Registry)> {
    // Labels are text KEYS (invariant 8), and they come from the CONSTANTS
    // generated from `assets/i18n/*.ftl`. Writing the string by hand would compile and
    // would lose the guarantee; with the constant, a key that does not exist is a compile
    // error.
    const COMMANDS: &[Command] = &[
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
        // `LEADER .` — the file manager, in the footer's expanded region.
        //
        // First level, and `.` because that is where Doom puts "find file". It
        // was free: `.` appears only at the second level (`TAB .`), and the
        // tree branches on the first key.
        //
        // A MANAGER AND NOT A PICKER, and `. .` IS this manager: the key
        // used to open the editor's file picker — hand a path over and stop
        // there — and it was given to the manager, which lists everything a
        // directory holds (permissions, owner, group, size, date, and a glyph
        // for what each thing is) and acts on it. This comment used to say the
        // key was still the picker's, eight hundred lines above the binding
        // that says otherwise.
        Command {
            id: "files-open",
            label: key::CMD_FILES_OPEN,
        },
        // The SAME manager, in a window of its own. The footer's region has a
        // ceiling in lines (`space.expansion`, invariant 14), which is right for
        // a footer and not for the job.
        Command {
            id: "files-window",
            label: key::CMD_FILES_WINDOW,
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
        // THE SCREENS — `LEADER s d`, and the gestures of the view itself,
        // which are commands like any other so the manual and the locator can
        // find them.
        Command {
            id: "display-open",
            label: key::CMD_DISPLAY_OPEN,
        },
        Command {
            id: "display-window",
            label: key::CMD_DISPLAY_WINDOW,
        },
        Command {
            id: "display-up",
            label: key::CMD_DISPLAY_UP,
        },
        Command {
            id: "display-down",
            label: key::CMD_DISPLAY_DOWN,
        },
        Command {
            id: "display-less",
            label: key::CMD_DISPLAY_LESS,
        },
        Command {
            id: "display-more",
            label: key::CMD_DISPLAY_MORE,
        },
        Command {
            id: "display-enter",
            label: key::CMD_DISPLAY_ENTER,
        },
        Command {
            id: "display-automatic",
            label: key::CMD_DISPLAY_AUTOMATIC,
        },
        Command {
            id: "display-forget",
            label: key::CMD_DISPLAY_FORGET,
        },
        Command {
            id: "display-arrange",
            label: key::CMD_DISPLAY_ARRANGE,
        },
        // `LEADER s a` — SOUND: what plays, what listens, and what each
        // program is doing with either. `s a a` opens it in the footer and
        // `s a w` in a window, the doubled letter and the `w` every view of
        // this shape uses.
        Command {
            id: "audio-open",
            label: key::CMD_AUDIO_OPEN,
        },
        Command {
            id: "audio-window",
            label: key::CMD_AUDIO_WINDOW,
        },
        // AND THE VIEW'S OWN GESTURES, reached through `audio::OF_THE_VIEW`.
        Command {
            id: "audio-up",
            label: key::CMD_AUDIO_UP,
        },
        Command {
            id: "audio-down",
            label: key::CMD_AUDIO_DOWN,
        },
        Command {
            id: "audio-enter",
            label: key::CMD_AUDIO_ENTER,
        },
        Command {
            id: "audio-less",
            label: key::CMD_AUDIO_LESS,
        },
        Command {
            id: "audio-more",
            label: key::CMD_AUDIO_MORE,
        },
        Command {
            id: "audio-mute",
            label: key::CMD_AUDIO_MUTE,
        },
        Command {
            id: "audio-default",
            label: key::CMD_AUDIO_DEFAULT,
        },
        // THE CAMERA STUDIO — `m c` opens the app (a picture, in a window of
        // its own, floating), `s c c` and `s c w` its settings. Every key of
        // the app is a command too, so `LEADER :` reaches a photo, a recording
        // or the next scene from anywhere without the app open.
        Command {
            id: "camera-open",
            label: key::CMD_CAMERA_OPEN,
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
        // THE SCREEN RESIZER, `/ r` — the first tool of the development
        // group: two curtains cut the screen, and everything is laid out in
        // what they leave (`crate::resizer`). On, or off when it is on.
        Command {
            id: "dev-resizer",
            label: key::CMD_DEV_RESIZER,
        },
        Command {
            id: "camera-settings-open",
            label: key::CMD_CAMERA_SETTINGS_OPEN,
        },
        Command {
            id: "camera-settings-window",
            label: key::CMD_CAMERA_SETTINGS_WINDOW,
        },
        Command {
            id: "camera-settings-up",
            label: key::CMD_CAMERA_SETTINGS_UP,
        },
        Command {
            id: "camera-settings-down",
            label: key::CMD_CAMERA_SETTINGS_DOWN,
        },
        Command {
            id: "camera-settings-less",
            label: key::CMD_CAMERA_SETTINGS_LESS,
        },
        Command {
            id: "camera-settings-more",
            label: key::CMD_CAMERA_SETTINGS_MORE,
        },
        Command {
            id: "camera-settings-enter",
            label: key::CMD_CAMERA_SETTINGS_ENTER,
        },
        Command {
            id: "camera-settings-remove",
            label: key::CMD_CAMERA_SETTINGS_REMOVE,
        },
        Command {
            id: "camera-photo",
            label: key::CMD_CAMERA_PHOTO,
        },
        Command {
            id: "camera-record",
            label: key::CMD_CAMERA_RECORD,
        },
        Command {
            id: "camera-effect-next",
            label: key::CMD_CAMERA_EFFECT_NEXT,
        },
        Command {
            id: "camera-effect-previous",
            label: key::CMD_CAMERA_EFFECT_PREVIOUS,
        },
        Command {
            id: "camera-background-next",
            label: key::CMD_CAMERA_BACKGROUND_NEXT,
        },
        Command {
            id: "camera-param-less",
            label: key::CMD_CAMERA_PARAM_LESS,
        },
        Command {
            id: "camera-param-more",
            label: key::CMD_CAMERA_PARAM_MORE,
        },
        Command {
            id: "camera-camera-next",
            label: key::CMD_CAMERA_CAMERA_NEXT,
        },
        Command {
            id: "camera-mirror",
            label: key::CMD_CAMERA_MIRROR,
        },
        Command {
            id: "camera-scene-next",
            label: key::CMD_CAMERA_SCENE_NEXT,
        },
        Command {
            id: "camera-scene-previous",
            label: key::CMD_CAMERA_SCENE_PREVIOUS,
        },
        Command {
            id: "camera-script",
            label: key::CMD_CAMERA_SCRIPT,
        },
        Command {
            id: "camera-next-cue",
            label: key::CMD_CAMERA_NEXT_CUE,
        },
        Command {
            id: "camera-virtual",
            label: key::CMD_CAMERA_VIRTUAL,
        },
        Command {
            id: "camera-stream",
            label: key::CMD_CAMERA_STREAM,
        },
        Command {
            id: "camera-scene-add",
            label: key::CMD_CAMERA_SCENE_ADD,
        },
        Command {
            id: "camera-scene-rename",
            label: key::CMD_CAMERA_SCENE_RENAME,
        },
        Command {
            id: "camera-script-add",
            label: key::CMD_CAMERA_SCRIPT_ADD,
        },
        Command {
            id: "camera-stream-url",
            label: key::CMD_CAMERA_STREAM_URL,
        },
        Command {
            id: "camera-stream-key",
            label: key::CMD_CAMERA_STREAM_KEY,
        },
        // THE PLAYER — `p p` in the footer, `p w` in a window, `p c` the
        // column on the right. The transport is under `p` too, so a song is
        // paused from anywhere without opening anything.
        Command {
            id: "player-open",
            label: key::CMD_PLAYER_OPEN,
        },
        Command {
            id: "player-window",
            label: key::CMD_PLAYER_WINDOW,
        },
        Command {
            id: "player-column",
            label: key::CMD_PLAYER_COLUMN,
        },
        Command {
            id: "player-play-file",
            label: key::CMD_PLAYER_PLAY_FILE,
        },
        Command {
            id: "player-enqueue",
            label: key::CMD_PLAYER_ENQUEUE,
        },
        // PLAYING TOGETHER: another video in step with this sound, asked as a
        // path or an address — the page's own row picks one from the files.
        Command {
            id: "player-together",
            label: key::CMD_PLAYER_TOGETHER,
        },
        // THE OTHER MACHINES' PAGE, and the pairing its row asks — which is
        // reached by name, the page being where it is done.
        Command {
            id: "player-machines",
            label: key::CMD_PLAYER_MACHINES,
        },
        Command {
            id: "player-pair",
            label: key::CMD_PLAYER_PAIR,
        },
        Command {
            id: "player-toggle",
            label: key::CMD_PLAYER_TOGGLE,
        },
        Command {
            id: "player-next",
            label: key::CMD_PLAYER_NEXT,
        },
        Command {
            id: "player-previous",
            label: key::CMD_PLAYER_PREVIOUS,
        },
        Command {
            id: "player-stop",
            label: key::CMD_PLAYER_STOP,
        },
        Command {
            id: "player-shuffle",
            label: key::CMD_PLAYER_SHUFFLE,
        },
        Command {
            id: "player-repeat",
            label: key::CMD_PLAYER_REPEAT,
        },
        Command {
            id: "player-up",
            label: key::CMD_PLAYER_UP,
        },
        Command {
            id: "player-down",
            label: key::CMD_PLAYER_DOWN,
        },
        Command {
            id: "player-less",
            label: key::CMD_PLAYER_LESS,
        },
        Command {
            id: "player-more",
            label: key::CMD_PLAYER_MORE,
        },
        Command {
            id: "player-enter",
            label: key::CMD_PLAYER_ENTER,
        },
        Command {
            id: "player-remove",
            label: key::CMD_PLAYER_REMOVE,
        },
        Command {
            id: "player-browse",
            label: key::CMD_PLAYER_BROWSE,
        },
        Command {
            id: "player-add",
            label: key::CMD_PLAYER_ADD,
        },
        Command {
            id: "player-only",
            label: key::CMD_PLAYER_ONLY,
        },
        Command {
            id: "player-play-now",
            label: key::CMD_PLAYER_PLAY_NOW,
        },
        Command {
            id: "player-clear",
            label: key::CMD_PLAYER_CLEAR,
        },
        Command {
            id: "player-quieter",
            label: key::CMD_PLAYER_QUIETER,
        },
        Command {
            id: "player-louder",
            label: key::CMD_PLAYER_LOUDER,
        },
        Command {
            id: "player-maximize",
            label: key::CMD_PLAYER_MAXIMIZE,
        },
        Command {
            id: "player-pip",
            label: key::CMD_PLAYER_PIP,
        },
        Command {
            id: "player-mute",
            label: key::CMD_PLAYER_MUTE,
        },
        // THE DOOR FOR THE OTHER — the view's `w`, the last button of the
        // transport and of the bar, and the glyph on the footer's "Now".
        Command {
            id: "player-swap",
            label: key::CMD_PLAYER_SWAP,
        },
        // THE MEDIA IN THE FILE — the view's `k` and the transport's glyph,
        // shown while the file offers a choice: the page of its sounds, its
        // pictures or its subtitles.
        Command {
            id: "player-tracks",
            label: key::CMD_PLAYER_TRACKS,
        },
        // THE QUEUE'S GROUPS FOLD — the view's `TAB` — and ONE OF THE FILE'S
        // MEDIA PLAYS TOO, `a` on its row: the view's keys, reached by name
        // like the others.
        Command {
            id: "player-fold",
            label: key::CMD_PLAYER_FOLD,
        },
        // AN ENTRY OF THE QUEUE ONE PLACE EARLIER OR LATER — the arrows on
        // it, taught in their own words: on a group they fold it.
        Command {
            id: "player-earlier",
            label: key::CMD_PLAYER_EARLIER,
        },
        Command {
            id: "player-later",
            label: key::CMD_PLAYER_LATER,
        },
        Command {
            id: "player-also",
            label: key::CMD_PLAYER_ALSO,
        },
        // ANOTHER NAME for an entry of the queue — the view's `r` — and the
        // player's own files: what plays saved as a set, the queue saved.
        Command {
            id: "player-rename",
            label: key::CMD_PLAYER_RENAME,
        },
        Command {
            id: "player-save-set",
            label: key::CMD_PLAYER_SAVE_SET,
        },
        Command {
            id: "player-save-queue",
            label: key::CMD_PLAYER_SAVE_QUEUE,
        },
        // THE SUBTITLES OVER THE PICTURE — `LEADER p u` and the view's `u` —
        // and SHOWING OR HIDING A LANE, the view's `v`, what a click on a
        // subtitle's chip does.
        Command {
            id: "player-subtitles",
            label: key::CMD_PLAYER_SUBTITLES,
        },
        Command {
            id: "player-lane-shown",
            label: key::CMD_PLAYER_LANE_SHOWN,
        },
        // THE AMBIENT SOUNDS — `LEADER p e`, their page — and every one of
        // them off, by name: the page has the row.
        Command {
            id: "player-ambience",
            label: key::CMD_PLAYER_AMBIENCE,
        },
        Command {
            id: "player-ambience-silence",
            label: key::CMD_PLAYER_AMBIENCE_SILENCE,
        },
        // THE TIMELINE — `LEADER p i` and the view's `t` — every lane of what
        // plays on one clock, a row a lane.
        Command {
            id: "player-timeline",
            label: key::CMD_PLAYER_TIMELINE,
        },
        // THE LAYOUT — `LEADER p y` and the view's `y` — how the pictures of
        // what plays share a screen; and WHAT PLAYS ON EVERY SCREEN it goes
        // to, the view's `M`.
        Command {
            id: "player-layout",
            label: key::CMD_PLAYER_LAYOUT,
        },
        Command {
            id: "player-every-screen",
            label: key::CMD_PLAYER_EVERY_SCREEN,
        },
        // THE METADATA — `LEADER p d` — what the file that plays says of
        // itself: the home's last section, the cursor on it.
        Command {
            id: "player-metadata",
            label: key::CMD_PLAYER_METADATA,
        },
        // WHAT THE INTERNET SAYS OF WHAT PLAYS — the suggestions for its
        // tags, subtitles, lyrics — and the page of what asks it and the
        // person's keys: by name.
        Command {
            id: "player-suggestions",
            label: key::CMD_PLAYER_SUGGESTIONS,
        },
        Command {
            id: "player-find-subtitles",
            label: key::CMD_PLAYER_FIND_SUBTITLES,
        },
        Command {
            id: "player-find-lyrics",
            label: key::CMD_PLAYER_FIND_LYRICS,
        },
        Command {
            id: "player-internet",
            label: key::CMD_PLAYER_INTERNET,
        },
        // THE DIRECTORIES — `LEADER p r`, `p v`, `p g` — the world's radio
        // and TV and the channels by theme; and their pages' keys: `/` to
        // search by name, `b` to keep among the favourites, and the globe's
        // three.
        Command {
            id: "player-radio",
            label: key::CMD_PLAYER_RADIO,
        },
        Command {
            id: "player-tv",
            label: key::CMD_PLAYER_TV,
        },
        Command {
            id: "player-channels",
            label: key::CMD_PLAYER_CHANNELS,
        },
        Command {
            id: "player-search",
            label: key::CMD_PLAYER_SEARCH,
        },
        Command {
            id: "player-favourite",
            label: key::CMD_PLAYER_FAVOURITE,
        },
        Command {
            id: "player-nearer",
            label: key::CMD_PLAYER_NEARER,
        },
        Command {
            id: "player-further",
            label: key::CMD_PLAYER_FURTHER,
        },
        Command {
            id: "player-world",
            label: key::CMD_PLAYER_WORLD,
        },
        // CLOSING IT EVERYWHERE — the daemon and every door of it; the
        // view's row asks first, and `p q` is the question already answered.
        Command {
            id: "player-quit",
            label: key::CMD_PLAYER_QUIT,
        },
        // MICE, TOUCHPADS AND EVERY CURSOR — `s i i` in the footer and `s i w`
        // in a window, the doubled letter and the `w` every view of this
        // shape uses. `i` for INPUT: `s p` is power and `s m` memory.
        Command {
            id: "pointers-open",
            label: key::CMD_POINTERS_OPEN,
        },
        Command {
            id: "pointers-window",
            label: key::CMD_POINTERS_WINDOW,
        },
        // AND THE VIEW'S OWN GESTURES, reached through `pointers::OF_THE_VIEW`.
        Command {
            id: "pointers-up",
            label: key::CMD_POINTERS_UP,
        },
        Command {
            id: "pointers-down",
            label: key::CMD_POINTERS_DOWN,
        },
        Command {
            id: "pointers-enter",
            label: key::CMD_POINTERS_ENTER,
        },
        Command {
            id: "pointers-less",
            label: key::CMD_POINTERS_LESS,
        },
        Command {
            id: "pointers-more",
            label: key::CMD_POINTERS_MORE,
        },
        Command {
            id: "pointers-forget",
            label: key::CMD_POINTERS_FORGET,
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
        // `LEADER s p` — POWER: the battery, the charger, the profile, sleep.
        // The view's own rows do the rest; these are the things worth a key
        // without opening it — a profile, the two locks, the four actions.
        Command {
            id: "power-open",
            label: key::CMD_POWER_OPEN,
        },
        Command {
            id: "power-window",
            label: key::CMD_POWER_WINDOW,
        },
        Command {
            id: "power-profile-performance",
            label: key::CMD_POWER_PROFILE_PERFORMANCE,
        },
        Command {
            id: "power-profile-balanced",
            label: key::CMD_POWER_PROFILE_BALANCED,
        },
        Command {
            id: "power-profile-saver",
            label: key::CMD_POWER_PROFILE_SAVER,
        },
        Command {
            id: "power-keep-awake",
            label: key::CMD_POWER_KEEP_AWAKE,
        },
        Command {
            id: "power-keep-lid",
            label: key::CMD_POWER_KEEP_LID,
        },
        Command {
            id: "power-thresholds",
            label: key::CMD_POWER_THRESHOLDS,
        },
        // THE FOUR THAT EMPTY THE SCREEN, under `q` beside `q q`. None acts
        // blind: each opens the view on its row, ARMED, and `RET` confirms —
        // one confirm mechanism, and nothing floats (invariant 14).
        Command {
            id: "power-suspend",
            label: key::CMD_POWER_SUSPEND,
        },
        Command {
            id: "power-hibernate",
            label: key::CMD_POWER_HIBERNATE,
        },
        Command {
            id: "power-reboot",
            label: key::CMD_POWER_REBOOT,
        },
        Command {
            id: "power-off",
            label: key::CMD_POWER_OFF,
        },
        // AND THE VIEW'S OWN GESTURES, reached through `power::OF_THE_VIEW`:
        // the hint line is generated from that table, and a key it teaches
        // has to be a command the manual can name.
        Command {
            id: "power-enter",
            label: key::CMD_POWER_ENTER,
        },
        Command {
            id: "power-less",
            label: key::CMD_POWER_LESS,
        },
        Command {
            id: "power-more",
            label: key::CMD_POWER_MORE,
        },
        Command {
            id: "power-up",
            label: key::CMD_POWER_UP,
        },
        Command {
            id: "power-down",
            label: key::CMD_POWER_DOWN,
        },
        // `LEADER s b` — THE RADIO, beside the machine's other hardware.
        //
        // `b` under `s` and not at the first level: what is under `s` is what
        // the MACHINE is doing (its memory, its disks, its power, its
        // network), and a radio with a keyboard on it is exactly that. The
        // group's own key is `s b b`, doubled the way `s n n`, `t t` and
        // `. .` are — a prefix cannot also be a command, and the tree refuses
        // it. That refusal is why the file manager's bare `.` had to become
        // `. .` the day `.` was asked to be a group. (`r r` was the network's,
        // and the same change that made this group killed the first-level
        // `r`.)
        // THE DEVICE PAGE'S THREE, which are the view's own arrows and the key
        // that crosses to the other half. They are in the registry for the
        // reason every view key is: the hint line is generated from the table
        // that names them, and a key the line teaches has to be a command the
        // manual can name.
        Command {
            id: "bluetooth-more",
            label: key::CMD_BLUETOOTH_MORE,
        },
        Command {
            id: "bluetooth-less",
            label: key::CMD_BLUETOOTH_LESS,
        },
        Command {
            id: "bluetooth-settings",
            label: key::CMD_BLUETOOTH_SETTINGS,
        },
        Command {
            id: "bluetooth-open",
            label: key::CMD_BLUETOOTH_OPEN,
        },
        Command {
            id: "bluetooth-window",
            label: key::CMD_BLUETOOTH_WINDOW,
        },
        Command {
            id: "bluetooth-scan",
            label: key::CMD_BLUETOOTH_SCAN,
        },
        Command {
            id: "bluetooth-power",
            label: key::CMD_BLUETOOTH_POWER,
        },
        Command {
            id: "bluetooth-discoverable",
            label: key::CMD_BLUETOOTH_DISCOVERABLE,
        },
        Command {
            id: "bluetooth-reconnect",
            label: key::CMD_BLUETOOTH_RECONNECT,
        },
        Command {
            id: "bluetooth-disconnect-all",
            label: key::CMD_BLUETOOTH_DISCONNECT_ALL,
        },
        Command {
            id: "bluetooth-accept",
            label: key::CMD_BLUETOOTH_ACCEPT,
        },
        Command {
            id: "bluetooth-refuse",
            label: key::CMD_BLUETOOTH_REFUSE,
        },
        Command {
            id: "bluetooth-device",
            label: key::CMD_BLUETOOTH_DEVICE,
        },
        Command {
            id: "bluetooth-connect",
            label: key::CMD_BLUETOOTH_CONNECT,
        },
        Command {
            id: "bluetooth-pair",
            label: key::CMD_BLUETOOTH_PAIR,
        },
        Command {
            id: "bluetooth-forget",
            label: key::CMD_BLUETOOTH_FORGET,
        },
        Command {
            id: "bluetooth-trust",
            label: key::CMD_BLUETOOTH_TRUST,
        },
        Command {
            id: "bluetooth-block",
            label: key::CMD_BLUETOOTH_BLOCK,
        },
        Command {
            id: "bluetooth-rename",
            label: key::CMD_BLUETOOTH_RENAME,
        },
        Command {
            id: "bluetooth-unnamed",
            label: key::CMD_BLUETOOTH_UNNAMED,
        },
        Command {
            id: "bluetooth-up",
            label: key::CMD_BLUETOOTH_UP,
        },
        Command {
            id: "bluetooth-down",
            label: key::CMD_BLUETOOTH_DOWN,
        },
        // (THE UPDATE'S THREE — the whole sequence by hand, and the view that
        // watches one in the footer and in a window — are its window's own
        // table, `update_window::COMMANDS`, registered with these below.)
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
            id: "screen-capture",
            label: key::CMD_SCREEN_CAPTURE,
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
        // THE NOTIFICATION MANAGER, and its second door.
        //
        // `notif-open`, the verb every other footer view answers to —
        // `memory-open`, `network-open`, `theme-open`. It was `notif-history`
        // for as long as its key was `n h`; the owner moved the key to `n n`
        // (the doubled letter every prefix's own view uses) and the id followed.
        Command {
            id: "notif-open",
            label: key::CMD_NOTIF_OPEN,
        },
        // The same manager in a window of its own — the same `View`, and the
        // same reason the file manager has two doors: the footer's region has a
        // ceiling in lines (invariant 14), which is right for a footer and not
        // for reading a fortnight of history.
        Command {
            id: "notif-window",
            label: key::CMD_NOTIF_WINDOW,
        },
        Command {
            id: "notif-read-all",
            label: key::CMD_NOTIF_READ_ALL,
        },
        // THE THREE THAT ACT ON THE FOCUSED ROW, and that is why they have no
        // key in the tree: outside the manager there is no focused row to act
        // on. They come in through `notifications::OF_THE_VIEW`, which is what
        // the manual and the orphan check read — the same arrangement the
        // network list already has.
        Command {
            id: "notif-read",
            label: key::CMD_NOTIF_READ,
        },
        Command {
            id: "notif-forget",
            label: key::CMD_NOTIF_FORGET,
        },
        Command {
            id: "notif-act",
            label: key::CMD_NOTIF_ACT,
        },
        // `RET` ON A ROW opens the record whole — body wrapped, buttons drawn,
        // facts beside it. It is a name of its own for `notif-adjust`'s
        // reason: the same key runs the FOCUSED BUTTON a moment later, and one
        // id with two meanings is one label that lies at one of the two.
        Command {
            id: "notif-see",
            label: key::CMD_NOTIF_SEE,
        },
        Command {
            id: "notif-filters",
            label: key::CMD_NOTIF_FILTERS,
        },
        Command {
            id: "notif-new",
            label: key::CMD_NOTIF_NEW,
        },
        Command {
            id: "notif-drop",
            label: key::CMD_NOTIF_DROP,
        },
        Command {
            id: "notif-share",
            label: key::CMD_NOTIF_SHARE,
        },
        Command {
            id: "notif-share-answer",
            label: key::CMD_NOTIF_SHARE_ANSWER,
        },
        Command {
            id: "notif-draft-type",
            label: key::CMD_NOTIF_DRAFT_TYPE,
        },
        Command {
            id: "notif-draft-add",
            label: key::CMD_NOTIF_DRAFT_ADD,
        },
        Command {
            id: "notif-draft-drop",
            label: key::CMD_NOTIF_DRAFT_DROP,
        },
        Command {
            id: "notif-draft-save",
            label: key::CMD_NOTIF_DRAFT_SAVE,
        },
        // `RET` ON THE CATALOGUE: the entry's sheet. A command of its own for
        // `notif-restore`'s reason — the same key means the other thing on
        // the other column, and one id with two meanings is one label that
        // lies at one of the two levels.
        Command {
            id: "notif-adjust",
            label: key::CMD_NOTIF_ADJUST,
        },
        // AND THE SIX THAT WALK IT. They are gestures — arrows, `TAB` — and
        // they are in the registry for the reason the three above are: the
        // manager's hint line is GENERATED from `notifications::OF_THE_VIEW`,
        // and a key the line teaches is a key the manual and the orphan check
        // have to be able to name.
        Command {
            id: "notif-up",
            label: key::CMD_NOTIF_UP,
        },
        Command {
            id: "notif-down",
            label: key::CMD_NOTIF_DOWN,
        },
        Command {
            id: "notif-pane-next",
            label: key::CMD_NOTIF_PANE_NEXT,
        },
        Command {
            id: "notif-pane-previous",
            label: key::CMD_NOTIF_PANE_PREVIOUS,
        },
        Command {
            id: "notif-knob-next",
            label: key::CMD_NOTIF_KNOB_NEXT,
        },
        Command {
            id: "notif-knob-previous",
            label: key::CMD_NOTIF_KNOB_PREVIOUS,
        },
        // `r` in the sheet: back to the distro's default. A command of its own
        // because the same letter on a list marks everything read, and one id
        // with two meanings is one label that lies at one of the two levels.
        Command {
            id: "notif-restore",
            label: key::CMD_NOTIF_RESTORE,
        },
        // THE TRANSFER QUEUE. `transfers-open` has a key in the tree
        // (`LEADER t t`); the keys that act on a row live INSIDE the view
        // (`transfers::OF_THE_VIEW`), the same arrangement the file manager
        // uses — they navigate a list rather than being commands of the
        // registry.
        Command {
            id: "transfers-open",
            label: key::CMD_TRANSFERS_OPEN,
        },
        // AND THE SAME QUEUE IN A WINDOW OF ITS OWN (`LEADER t w`), which is a
        // wasm buffer and not a second implementation: it subscribes to the
        // same socket and draws the same rows with room to breathe. Its own
        // keys are in ITS descriptor, under `SPC m`, and are resolved by its
        // host while it holds the keyboard — not here.
        Command {
            id: "transfers-window",
            label: key::CMD_TRANSFERS_WINDOW,
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
        // AND THE EIGHT THAT PUT SOMETHING IN. What they read is the clipboard,
        // because an address is copied and never typed — see
        // `Footer::queue_the_clipboard`.
        //
        // FOUR AND FOUR, and the pairing is the point: everything that can be
        // done through the browser's extension can be done here, and every one
        // of them exists twice — anonymous, and as the person. The app does not
        // depend on the extension for anything.
        Command {
            id: "transfers-add",
            label: key::CMD_TRANSFERS_ADD,
        },
        Command {
            id: "transfers-video",
            label: key::CMD_TRANSFERS_VIDEO,
        },
        Command {
            id: "transfers-audio",
            label: key::CMD_TRANSFERS_AUDIO,
        },
        Command {
            id: "transfers-subtitles",
            label: key::CMD_TRANSFERS_SUBTITLES,
        },
        Command {
            id: "transfers-add-as-me",
            label: key::CMD_TRANSFERS_ADD_AS_ME,
        },
        Command {
            id: "transfers-video-as-me",
            label: key::CMD_TRANSFERS_VIDEO_AS_ME,
        },
        Command {
            id: "transfers-audio-as-me",
            label: key::CMD_TRANSFERS_AUDIO_AS_ME,
        },
        Command {
            id: "transfers-subtitles-as-me",
            label: key::CMD_TRANSFERS_SUBTITLES_AS_ME,
        },
        // THE CLIPBOARD. `clipboard-open` has a key in the tree (`LEADER c c`); the
        // others have keys INSIDE the view, and that is why they go into
        // `crate::clipboard_view::OF_THE_VIEW` — otherwise the orphan-command check flags them, and
        // rightly: a command with no path at all is a command nobody uses.
        Command {
            id: "clipboard-open",
            label: key::CMD_CLIPBOARD_OPEN,
        },
        Command {
            id: "clipboard-copy-of-new",
            label: key::CMD_CLIPBOARD_COPY_OF_NEW,
        },
        Command {
            id: "clipboard-remove",
            label: key::CMD_CLIPBOARD_REMOVE,
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
        Command {
            id: "clipboard-pin",
            label: key::CMD_CLIPBOARD_PIN,
        },
        Command {
            id: "clipboard-qrcode",
            label: key::CMD_CLIPBOARD_QRCODE,
        },
        Command {
            id: "clipboard-clear",
            label: key::CMD_CLIPBOARD_CLEAR,
        },
        // THE CLIPBOARD'S OWN GESTURES. `clipboard-sync` replaces what was
        // `clipboard-clipcascade-configure`, a command that existed to say the
        // integration did not — honest, and a placeholder. It does something
        // now.
        Command {
            id: "clipboard-down",
            label: key::CMD_CLIPBOARD_DOWN,
        },
        Command {
            id: "clipboard-up",
            label: key::CMD_CLIPBOARD_UP,
        },
        Command {
            id: "clipboard-sync",
            label: key::CMD_CLIPBOARD_SYNC,
        },
        Command {
            id: "notif-dismiss",
            label: key::CMD_NOTIF_DISMISS,
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
        // THE PERSON'S OWN PROGRAMS (`LEADER i`): the footer's view and its
        // window, the four levels a key opens directly, the five questions
        // its rows ask on their door's line (the footer's minibuffer or the
        // window's header), what a notification offers, and the view's own
        // keys (`software::OF_THE_VIEW`).
        Command {
            id: "software-open",
            label: key::CMD_SOFTWARE_OPEN,
        },
        Command {
            id: "software-window",
            label: key::CMD_SOFTWARE_WINDOW,
        },
        Command {
            id: "software-trials",
            label: key::CMD_SOFTWARE_TRIALS,
        },
        Command {
            id: "software-maintenance",
            label: key::CMD_SOFTWARE_MAINTENANCE,
        },
        Command {
            id: "software-android",
            label: key::CMD_SOFTWARE_ANDROID,
        },
        Command {
            id: "software-web-new",
            label: key::CMD_SOFTWARE_WEB_NEW,
        },
        Command {
            id: "software-web-address",
            label: key::CMD_SOFTWARE_WEB_ADDRESS,
        },
        Command {
            id: "software-web-name",
            label: key::CMD_SOFTWARE_WEB_NAME,
        },
        Command {
            id: "software-icon-path",
            label: key::CMD_SOFTWARE_ICON_PATH,
        },
        Command {
            id: "software-file-path",
            label: key::CMD_SOFTWARE_FILE_PATH,
        },
        Command {
            id: "software-launch-latest",
            label: key::CMD_SOFTWARE_LAUNCH_LATEST,
        },
        Command {
            id: "software-down",
            label: key::CMD_SOFTWARE_DOWN,
        },
        Command {
            id: "software-up",
            label: key::CMD_SOFTWARE_UP,
        },
        Command {
            id: "software-enter",
            label: key::CMD_SOFTWARE_ENTER,
        },
        Command {
            id: "software-next-source",
            label: key::CMD_SOFTWARE_NEXT_SOURCE,
        },
        Command {
            id: "software-previous-source",
            label: key::CMD_SOFTWARE_PREVIOUS_SOURCE,
        },
        Command {
            id: "software-less",
            label: key::CMD_SOFTWARE_LESS,
        },
        Command {
            id: "software-more",
            label: key::CMD_SOFTWARE_MORE,
        },
        Command {
            id: "software-install",
            label: key::CMD_SOFTWARE_INSTALL,
        },
        Command {
            id: "software-try-it",
            label: key::CMD_SOFTWARE_TRY_IT,
        },
        Command {
            id: "software-launch",
            label: key::CMD_SOFTWARE_LAUNCH,
        },
        Command {
            id: "software-update",
            label: key::CMD_SOFTWARE_UPDATE,
        },
        Command {
            id: "software-edit",
            label: key::CMD_SOFTWARE_EDIT,
        },
        Command {
            id: "software-remove",
            label: key::CMD_SOFTWARE_REMOVE,
        },
        Command {
            id: "software-stop",
            label: key::CMD_SOFTWARE_STOP,
        },
        Command {
            id: "software-paste",
            label: key::CMD_SOFTWARE_PASTE,
        },
        // THE MANUAL. It exists because the chords left the tap list:
        // what one holds has to appear somewhere, and the tap
        // list was not that place — see `manual.rs`.
        //
        // AND IT IS A LIST: `LEADER h` opens the manuals — the shortcuts', the
        // gestures' — and no manual has a key of its own (the owner's rule).
        // Each opens from the list, here or in a window. See `manual::Which`.
        Command {
            id: "help",
            label: key::CMD_HELP,
        },
        // ---------------------------------------------------------------
        // NETWORK — the SEVENTH view of the expanded region.
        //
        // `network-open` is the list; the others act with no list because there is nothing
        // to choose. `network-connect` and `network-forget` do NOT have a key in the
        // tree: they act on the focused row, and that is why their key
        // is from INSIDE the view — they come in through `network::OF_THE_VIEW`.
        // ---------------------------------------------------------------
        // THE AEROPLANE, which is the one thing on the way-out marks that
        // is a switch rather than a door: `s n a`, and a click on the mark.
        Command {
            id: "network-airplane",
            label: key::CMD_NETWORK_AIRPLANE,
        },
        Command {
            id: "network-open",
            label: key::CMD_NETWORK_OPEN,
        },
        Command {
            id: "network-window",
            label: key::CMD_NETWORK_WINDOW,
        },
        Command {
            id: "network-scan",
            label: key::CMD_NETWORK_SCAN,
        },
        Command {
            id: "network-connect",
            label: key::CMD_NETWORK_CONNECT,
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
        // THE LANGUAGE AND THE REGION. Under `s` because what it changes
        // is what the MACHINE says back — the same group as the theme, the
        // screens and the clock's zone, and not something one does WITH the
        // machine.
        Command {
            id: "geo-open",
            label: key::CMD_GEO_OPEN,
        },
        Command {
            id: "geo-window",
            label: key::CMD_GEO_WINDOW,
        },
        Command {
            id: "geo-up",
            label: key::CMD_GEO_UP,
        },
        Command {
            id: "geo-down",
            label: key::CMD_GEO_DOWN,
        },
        Command {
            id: "geo-enter",
            label: key::CMD_GEO_ENTER,
        },
        // THE TWO THE LINE AND THE QUESTION ANSWER. `geo-find` runs twice in
        // the footer, like the language view's questions: once to ask, once
        // to be told — and a window's line answers under the same id.
        Command {
            id: "geo-find",
            label: key::CMD_GEO_FIND,
        },
        Command {
            id: "geo-remove",
            label: key::CMD_GEO_REMOVE,
        },
        Command {
            id: "locale-open",
            label: key::CMD_LOCALE_OPEN,
        },
        Command {
            id: "locale-window",
            label: key::CMD_LOCALE_WINDOW,
        },
        Command {
            id: "locale-up",
            label: key::CMD_LOCALE_UP,
        },
        Command {
            id: "locale-down",
            label: key::CMD_LOCALE_DOWN,
        },
        Command {
            id: "locale-enter",
            label: key::CMD_LOCALE_ENTER,
        },
        Command {
            id: "locale-less",
            label: key::CMD_LOCALE_LESS,
        },
        Command {
            id: "locale-more",
            label: key::CMD_LOCALE_MORE,
        },
        // THE QUESTIONS OF THE LANGUAGE VIEW, and the ids their answers come
        // back under — the footer's minibuffer runs them twice, once to ask
        // and once to be told, and a window's line answers under the same id
        // into the same `locale::took`. Reachable by name like every other
        // command.
        Command {
            id: "locale-zone",
            label: key::CMD_LOCALE_ZONE,
        },
        Command {
            id: "locale-place",
            label: key::CMD_LOCALE_PLACE,
        },
        Command {
            id: "locale-where",
            label: key::CMD_LOCALE_WHERE,
        },
        // THE MACHINE'S THREE, asked by `RET` on their rows. They were asked
        // under ids no command had: the footer drew the raw id as the
        // question, and the answer ran nothing.
        Command {
            id: "locale-system-date",
            label: key::CMD_LOCALE_SYSTEM_DATE,
        },
        Command {
            id: "locale-system-time",
            label: key::CMD_LOCALE_SYSTEM_TIME,
        },
        Command {
            id: "locale-system-zone",
            label: key::CMD_LOCALE_SYSTEM_ZONE,
        },
        Command {
            id: "theme-open",
            label: key::CMD_THEME_OPEN,
        },
        Command {
            id: "theme-apply",
            label: key::CMD_THEME_APPLY,
        },
        Command {
            id: "network-disconnect",
            label: key::CMD_NETWORK_DISCONNECT,
        },
        Command {
            id: "network-forget",
            label: key::CMD_NETWORK_FORGET,
        },
        Command {
            id: "network-wifi-toggle",
            label: key::CMD_NETWORK_WIFI_TOGGLE,
        },
        Command {
            id: "network-link",
            label: key::CMD_NETWORK_LINK,
        },
        // AND THE VIEW'S OWN GESTURES, reached through `network::OF_THE_VIEW`:
        // the hint line is generated from that table, and a key it teaches
        // has to be a command the manual can name.
        Command {
            id: "network-up",
            label: key::CMD_NETWORK_UP,
        },
        Command {
            id: "network-down",
            label: key::CMD_NETWORK_DOWN,
        },
        Command {
            id: "network-lan",
            label: key::CMD_NETWORK_LAN,
        },
        Command {
            id: "network-import",
            label: key::CMD_NETWORK_IMPORT,
        },
        // THE SECTION RING. Reachable by name (`LEADER :`) like every other
        // command of a view, and taught on the hint line because it is in
        // `network::OF_THE_VIEW` — which is also what makes the orphan check
        // cover it.
        Command {
            id: "network-section-next",
            label: key::CMD_NETWORK_SECTION_NEXT,
        },
        Command {
            id: "network-section-previous",
            label: key::CMD_NETWORK_SECTION_PREVIOUS,
        },
        // THE SHELL'S HISTORY — the first screen this distro has ever had for
        // it. `atuin` was installed and configured by a rebuild, and choosing
        // between two search modes is not a decision the size of one.
        Command {
            id: "atuin-open",
            label: key::CMD_ATUIN_OPEN,
        },
        Command {
            id: "atuin-window",
            label: key::CMD_ATUIN_WINDOW,
        },
        Command {
            id: "atuin-sync",
            label: key::CMD_ATUIN_SYNC,
        },
        Command {
            id: "atuin-down",
            label: key::CMD_ATUIN_DOWN,
        },
        Command {
            id: "atuin-up",
            label: key::CMD_ATUIN_UP,
        },
        Command {
            id: "atuin-enter",
            label: key::CMD_ATUIN_ENTER,
        },
        Command {
            id: "clipboard-more",
            label: key::CMD_CLIPBOARD_MORE,
        },
        Command {
            id: "clipboard-less",
            label: key::CMD_CLIPBOARD_LESS,
        },
        Command {
            id: "atuin-more",
            label: key::CMD_ATUIN_MORE,
        },
        Command {
            id: "atuin-less",
            label: key::CMD_ATUIN_LESS,
        },
        Command {
            id: "atuin-back",
            label: key::CMD_ATUIN_BACK,
        },
        // TOR'S SHEET. Its keys live in `network::OF_THE_VIEW` like the rest
        // of the view's gestures — they are taught there, the manual names
        // them here, and the orphan check covers them because that table is
        // what `reachable` reads.
        // THE WAY STRAIGHT TO TOR. `s n n` opens the view at the top and the
        // section ring is ten long, so reaching Tor meant counting `TAB`s —
        // and the number is not a constant: a machine with no radio and no
        // cable has fewer sections. `fresh_at` puts the section's TITLE at the
        // top, which is what the modeline marks already do.
        Command {
            id: "network-tor-open",
            label: key::CMD_NETWORK_TOR_OPEN,
        },
        Command {
            id: "network-tor-more",
            label: key::CMD_NETWORK_TOR_MORE,
        },
        Command {
            id: "network-tor-less",
            label: key::CMD_NETWORK_TOR_LESS,
        },
        Command {
            id: "network-tor-back",
            label: key::CMD_NETWORK_TOR_BACK,
        },
        // SYNCTHING. It ships with the distro (nix/modules/syncthing.nix) and
        // for a long time offered four keys, one of which opened a WEB PAGE in
        // a browser: adding a folder, letting a machine in or changing any of
        // its hundred and twenty-one options meant leaving the system for a
        // page, which is the outside chrome invariant 14 refuses. Now it is a
        // view with two doors, and the page stays one key away because it has
        // graphs a list of labels and values does not replace.
        //
        // `pause` and `resume` are `-all` because the VIEW has a `p` that
        // pauses what the cursor is on: two different ideas, and an id may
        // only mean one of them.
        Command {
            id: "syncthing-open",
            label: key::CMD_SYNCTHING_OPEN,
        },
        Command {
            id: "syncthing-window",
            label: key::CMD_SYNCTHING_WINDOW,
        },
        Command {
            id: "syncthing-web",
            label: key::CMD_SYNCTHING_WEB,
        },
        Command {
            id: "syncthing-state",
            label: key::CMD_SYNCTHING_STATE,
        },
        Command {
            id: "syncthing-pause-all",
            label: key::CMD_SYNCTHING_PAUSE_ALL,
        },
        Command {
            id: "syncthing-resume-all",
            label: key::CMD_SYNCTHING_RESUME_ALL,
        },
        // THE VIEW'S OWN GESTURES. They carry no sequence — a key inside a
        // view is not a leader key — but they are registered, because the hint
        // line is generated from `OF_THE_VIEW` and reads a `cmd-<id>` label
        // for every one of them.
        Command {
            id: "syncthing-up",
            label: key::CMD_SYNCTHING_UP,
        },
        Command {
            id: "syncthing-down",
            label: key::CMD_SYNCTHING_DOWN,
        },
        Command {
            id: "syncthing-less",
            label: key::CMD_SYNCTHING_LESS,
        },
        Command {
            id: "syncthing-more",
            label: key::CMD_SYNCTHING_MORE,
        },
        Command {
            id: "syncthing-enter",
            label: key::CMD_SYNCTHING_ENTER,
        },
        Command {
            id: "syncthing-pause",
            label: key::CMD_SYNCTHING_PAUSE,
        },
        Command {
            id: "syncthing-rescan",
            label: key::CMD_SYNCTHING_RESCAN,
        },
        Command {
            id: "syncthing-add",
            label: key::CMD_SYNCTHING_ADD,
        },
        Command {
            id: "syncthing-forget",
            label: key::CMD_SYNCTHING_FORGET,
        },
        // ADDING ONE, FROM THE TREE OR FROM THE VIEW. The command that asks is
        // the command that answers — `s n v` and `s b y` both work this way,
        // and the minibuffer's question is the command's own `ask-<id>`.
        //
        // They are leader keys in their own right because they are worth one:
        // pairing a machine is something a person does holding a phone, and
        // walking into a view first to press `a` is a step for nothing.
        Command {
            id: "syncthing-add-device",
            label: key::CMD_SYNCTHING_ADD_DEVICE,
        },
        Command {
            id: "syncthing-add-folder",
            label: key::CMD_SYNCTHING_ADD_FOLDER,
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
        // `crate::hidden`. By name only (`BY_NAME_ONLY`): the key of the
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
        // THE ICONS APP (`crate::icons`): the footer's region and the window,
        // a screen each, and the Lucide colour's question.
        Command {
            id: "icons-open",
            label: key::CMD_ICONS_OPEN,
        },
        Command {
            id: "icons-window",
            label: key::CMD_ICONS_WINDOW,
        },
        Command {
            id: "icons-nerd-fonts",
            label: key::CMD_ICONS_NERD_FONTS,
        },
        Command {
            id: "icons-lucide",
            label: key::CMD_ICONS_LUCIDE,
        },
        Command {
            id: "icons-unicode",
            label: key::CMD_ICONS_UNICODE,
        },
        Command {
            id: "icons-colour",
            label: key::CMD_ICONS_COLOUR,
        },
        // THE CALCULATOR, `LEADER =` — the footer's door and the window's
        // (`calc`, `calc_window`).
        Command {
            id: "calc-open",
            label: key::CMD_CALC_OPEN,
        },
        Command {
            id: "calc-window",
            label: key::CMD_CALC_WINDOW,
        },
        // THE PLAYER'S DOORS (2026-09-30): the seeks beside the clock, the
        // picture folded away, the queue saved again, the countries' order,
        // the lyrics translated — keys of the view, reached by name — and
        // the website page (`LEADER p h`).
        Command {
            id: "player-rewind",
            label: key::CMD_PLAYER_REWIND,
        },
        Command {
            id: "player-forward",
            label: key::CMD_PLAYER_FORWARD,
        },
        Command {
            id: "player-picture-fold",
            label: key::CMD_PLAYER_PICTURE_FOLD,
        },
        Command {
            id: "player-save-queue-again",
            label: key::CMD_PLAYER_SAVE_QUEUE_AGAIN,
        },
        Command {
            id: "player-countries-order",
            label: key::CMD_PLAYER_COUNTRIES_ORDER,
        },
        Command {
            id: "player-lyrics-translate",
            label: key::CMD_PLAYER_LYRICS_TRANSLATE,
        },
        Command {
            id: "player-website",
            label: key::CMD_PLAYER_WEBSITE,
        },
        // A LISTING OF THE CATALOGUE'S KEYS (`player::OF_THE_VIEW`,
        // `crate::player_catalog`).
        Command {
            id: "player-listing-earlier",
            label: key::CMD_PLAYER_LISTING_EARLIER,
        },
        Command {
            id: "player-listing-later",
            label: key::CMD_PLAYER_LISTING_LATER,
        },
        Command {
            id: "player-listing-first",
            label: key::CMD_PLAYER_LISTING_FIRST,
        },
        Command {
            id: "player-listing-last",
            label: key::CMD_PLAYER_LISTING_LAST,
        },
        Command {
            id: "player-listing-full",
            label: key::CMD_PLAYER_LISTING_FULL,
        },
        Command {
            id: "player-listing-layout",
            label: key::CMD_PLAYER_LISTING_LAYOUT,
        },
        Command {
            id: "player-listing-source",
            label: key::CMD_PLAYER_LISTING_SOURCE,
        },
        Command {
            id: "player-listing-letter",
            label: key::CMD_PLAYER_LISTING_LETTER,
        },
        Command {
            id: "player-listing-order",
            label: key::CMD_PLAYER_LISTING_ORDER,
        },
        // `LEADER v` — THE VIDEO CATALOGUE (`video_metrics`): its two doors, a
        // scan now, the view's keys and the values it asks to be typed.
        Command {
            id: "video-metrics-open",
            label: key::CMD_VIDEO_METRICS_OPEN,
        },
        Command {
            id: "video-metrics-window",
            label: key::CMD_VIDEO_METRICS_WINDOW,
        },
        Command {
            id: "video-metrics-scan",
            label: key::CMD_VIDEO_METRICS_SCAN,
        },
        Command {
            id: "video-metrics-up",
            label: key::CMD_VIDEO_METRICS_UP,
        },
        Command {
            id: "video-metrics-down",
            label: key::CMD_VIDEO_METRICS_DOWN,
        },
        Command {
            id: "video-metrics-less",
            label: key::CMD_VIDEO_METRICS_LESS,
        },
        Command {
            id: "video-metrics-more",
            label: key::CMD_VIDEO_METRICS_MORE,
        },
        Command {
            id: "video-metrics-enter",
            label: key::CMD_VIDEO_METRICS_ENTER,
        },
        Command {
            id: "video-metrics-page-back",
            label: key::CMD_VIDEO_METRICS_PAGE_BACK,
        },
        Command {
            id: "video-metrics-page-on",
            label: key::CMD_VIDEO_METRICS_PAGE_ON,
        },
        Command {
            id: "video-metrics-layout",
            label: key::CMD_VIDEO_METRICS_LAYOUT,
        },
        Command {
            id: "video-metrics-search",
            label: key::CMD_VIDEO_METRICS_SEARCH,
        },
        Command {
            id: "video-metrics-player",
            label: key::CMD_VIDEO_METRICS_PLAYER,
        },
        Command {
            id: "video-metrics-kiosk",
            label: key::CMD_VIDEO_METRICS_KIOSK,
        },
        Command {
            id: "video-metrics-download",
            label: key::CMD_VIDEO_METRICS_DOWNLOAD,
        },
        Command {
            id: "video-metrics-hide",
            label: key::CMD_VIDEO_METRICS_HIDE,
        },
        Command {
            id: "video-metrics-delete",
            label: key::CMD_VIDEO_METRICS_DELETE,
        },
        Command {
            id: "video-metrics-add-channel",
            label: key::CMD_VIDEO_METRICS_ADD_CHANNEL,
        },
        Command {
            id: "video-metrics-channel-score",
            label: key::CMD_VIDEO_METRICS_CHANNEL_SCORE,
        },
        Command {
            id: "video-metrics-points",
            label: key::CMD_VIDEO_METRICS_POINTS,
        },
        Command {
            id: "video-metrics-days",
            label: key::CMD_VIDEO_METRICS_DAYS,
        },
        Command {
            id: "video-metrics-band",
            label: key::CMD_VIDEO_METRICS_BAND,
        },
        Command {
            id: "video-metrics-words",
            label: key::CMD_VIDEO_METRICS_WORDS,
        },
        Command {
            id: "video-metrics-second-tags",
            label: key::CMD_VIDEO_METRICS_SECOND_TAGS,
        },
        Command {
            id: "video-metrics-weight",
            label: key::CMD_VIDEO_METRICS_WEIGHT,
        },
        // THE SECURITY CENTRE (`crate::security`): the view's two doors, the
        // leader's toggles and checks, the view's keys, and the questions its rows
        // ask — each an arm of `run_with`.
        Command {
            id: "security-open",
            label: key::CMD_SECURITY_OPEN,
        },
        Command {
            id: "security-window",
            label: key::CMD_SECURITY_WINDOW,
        },
        Command {
            id: "security-relax",
            label: key::CMD_SECURITY_RELAX,
        },
        Command {
            id: "security-shields",
            label: key::CMD_SECURITY_SHIELDS,
        },
        Command {
            id: "security-firewall",
            label: key::CMD_SECURITY_FIREWALL,
        },
        Command {
            id: "security-check-password",
            label: key::CMD_SECURITY_CHECK_PASSWORD,
        },
        Command {
            id: "security-check-email",
            label: key::CMD_SECURITY_CHECK_EMAIL,
        },
        Command {
            id: "security-check-file",
            label: key::CMD_SECURITY_CHECK_FILE,
        },
        Command {
            id: "security-down",
            label: key::CMD_SECURITY_DOWN,
        },
        Command {
            id: "security-up",
            label: key::CMD_SECURITY_UP,
        },
        Command {
            id: "security-enter",
            label: key::CMD_SECURITY_ENTER,
        },
        Command {
            id: "security-left",
            label: key::CMD_SECURITY_LEFT,
        },
        Command {
            id: "security-right",
            label: key::CMD_SECURITY_RIGHT,
        },
        Command {
            id: "security-rule-add",
            label: key::CMD_SECURITY_RULE_ADD,
        },
        Command {
            id: "security-rule-delete",
            label: key::CMD_SECURITY_RULE_DELETE,
        },
        Command {
            id: "security-enrol",
            label: key::CMD_SECURITY_ENROL,
        },
        Command {
            id: "security-password",
            label: key::CMD_SECURITY_PASSWORD,
        },
        Command {
            id: "security-rule",
            label: key::CMD_SECURITY_RULE,
        },
        Command {
            id: "security-vt-key",
            label: key::CMD_SECURITY_VT_KEY,
        },
        Command {
            id: "security-vt-forget",
            label: key::CMD_SECURITY_VT_FORGET,
        },
        Command {
            id: "security-home",
            label: key::CMD_SECURITY_HOME,
        },
        // THE PERSON'S CONTAINERS, `LEADER s o` (`crate::containers`): the
        // two doors, and the view's own keys (`containers::OF_THE_VIEW`).
        Command {
            id: "containers-open",
            label: key::CMD_CONTAINERS_OPEN,
        },
        Command {
            id: "containers-window",
            label: key::CMD_CONTAINERS_WINDOW,
        },
        Command {
            id: "containers-down",
            label: key::CMD_CONTAINERS_DOWN,
        },
        Command {
            id: "containers-up",
            label: key::CMD_CONTAINERS_UP,
        },
        Command {
            id: "containers-enter",
            label: key::CMD_CONTAINERS_ENTER,
        },
        Command {
            id: "containers-left",
            label: key::CMD_CONTAINERS_LEFT,
        },
        Command {
            id: "containers-right",
            label: key::CMD_CONTAINERS_RIGHT,
        },
        Command {
            id: "containers-next-kind",
            label: key::CMD_CONTAINERS_NEXT_KIND,
        },
        Command {
            id: "containers-previous-kind",
            label: key::CMD_CONTAINERS_PREVIOUS_KIND,
        },
        Command {
            id: "containers-next-tab",
            label: key::CMD_CONTAINERS_NEXT_TAB,
        },
        Command {
            id: "containers-previous-tab",
            label: key::CMD_CONTAINERS_PREVIOUS_TAB,
        },
        Command {
            id: "containers-start-stop",
            label: key::CMD_CONTAINERS_START_STOP,
        },
        Command {
            id: "containers-restart",
            label: key::CMD_CONTAINERS_RESTART,
        },
        Command {
            id: "containers-remove",
            label: key::CMD_CONTAINERS_REMOVE,
        },
        Command {
            id: "containers-logs",
            label: key::CMD_CONTAINERS_LOGS,
        },
        Command {
            id: "containers-shell",
            label: key::CMD_CONTAINERS_SHELL,
        },
        Command {
            id: "containers-inspect",
            label: key::CMD_CONTAINERS_INSPECT,
        },
        Command {
            id: "containers-pull",
            label: key::CMD_CONTAINERS_PULL,
        },
        Command {
            id: "containers-build",
            label: key::CMD_CONTAINERS_BUILD,
        },
        Command {
            id: "containers-login",
            label: key::CMD_CONTAINERS_LOGIN,
        },
        Command {
            id: "containers-login-password",
            label: key::CMD_CONTAINERS_LOGIN_PASSWORD,
        },
        Command {
            id: "containers-prune",
            label: key::CMD_CONTAINERS_PRUNE,
        },
        Command {
            id: "containers-reveal",
            label: key::CMD_CONTAINERS_REVEAL,
        },
        // THE BELONGINGS, `LEADER s v` (`crate::belongings`).
        Command {
            id: "belongings-open",
            label: key::CMD_BELONGINGS_OPEN,
        },
        Command {
            id: "belongings-window",
            label: key::CMD_BELONGINGS_WINDOW,
        },
        Command {
            id: "belongings-sync",
            label: key::CMD_BELONGINGS_SYNC,
        },
        Command {
            id: "belongings-up",
            label: key::CMD_BELONGINGS_UP,
        },
        Command {
            id: "belongings-down",
            label: key::CMD_BELONGINGS_DOWN,
        },
        Command {
            id: "belongings-less",
            label: key::CMD_BELONGINGS_LESS,
        },
        Command {
            id: "belongings-more",
            label: key::CMD_BELONGINGS_MORE,
        },
        Command {
            id: "belongings-enter",
            label: key::CMD_BELONGINGS_ENTER,
        },
        Command {
            id: "belongings-keep-local",
            label: key::CMD_BELONGINGS_KEEP_LOCAL,
        },
        Command {
            id: "belongings-take-remote",
            label: key::CMD_BELONGINGS_TAKE_REMOTE,
        },
        Command {
            id: "belongings-keep-both",
            label: key::CMD_BELONGINGS_KEEP_BOTH,
        },
        Command {
            id: "belongings-apply",
            label: key::CMD_BELONGINGS_APPLY,
        },
        Command {
            id: "belongings-abandon",
            label: key::CMD_BELONGINGS_ABANDON,
        },
        Command {
            id: "belongings-all-local",
            label: key::CMD_BELONGINGS_ALL_LOCAL,
        },
        Command {
            id: "belongings-all-remote",
            label: key::CMD_BELONGINGS_ALL_REMOTE,
        },
        Command {
            id: "belongings-repository",
            label: key::CMD_BELONGINGS_REPOSITORY,
        },
        Command {
            id: "belongings-add",
            label: key::CMD_BELONGINGS_ADD,
        },
        Command {
            id: "belongings-auth",
            label: key::CMD_BELONGINGS_AUTH,
        },
        Command {
            id: "belongings-keygen",
            label: key::CMD_BELONGINGS_KEYGEN,
        },
        Command {
            id: "belongings-copy-key",
            label: key::CMD_BELONGINGS_COPY_KEY,
        },
        Command {
            id: "belongings-token",
            label: key::CMD_BELONGINGS_TOKEN,
        },
        Command {
            id: "belongings-user",
            label: key::CMD_BELONGINGS_USER,
        },
        Command {
            id: "belongings-passphrase",
            label: key::CMD_BELONGINGS_PASSPHRASE,
        },
        Command {
            id: "belongings-period",
            label: key::CMD_BELONGINGS_PERIOD,
        },
        // THE SEARCH OVER EVERYTHING, in its four doors.
        Command {
            id: "fast-access-open",
            label: key::CMD_FAST_ACCESS_OPEN,
        },
        Command {
            id: "fast-access-window",
            label: key::CMD_FAST_ACCESS_WINDOW,
        },
        Command {
            id: "fast-access-column",
            label: key::CMD_FAST_ACCESS_COLUMN,
        },
        Command {
            id: "fast-access-float",
            label: key::CMD_FAST_ACCESS_FLOAT,
        },
        // WHAT A DOOR OF THE SEARCH ASKS THE FOOTER TO OPEN, which outlives
        // the window: by name only, each with its argument.
        Command {
            id: "fast-access-launch",
            label: key::CMD_FAST_ACCESS_LAUNCH,
        },
        Command {
            id: "fast-access-open-path",
            label: key::CMD_FAST_ACCESS_OPEN_PATH,
        },
        Command {
            id: "fast-access-reveal",
            label: key::CMD_FAST_ACCESS_REVEAL,
        },
        Command {
            id: "fast-access-man",
            label: key::CMD_FAST_ACCESS_MAN,
        },
        // THE RANKING and its keys (`crate::player_ranking`).
        Command {
            id: "player-ranking",
            label: key::CMD_PLAYER_RANKING,
        },
        Command {
            id: "player-ranking-more",
            label: key::CMD_PLAYER_RANKING_MORE,
        },
        Command {
            id: "player-ranking-earlier",
            label: key::CMD_PLAYER_RANKING_EARLIER,
        },
        Command {
            id: "player-ranking-later",
            label: key::CMD_PLAYER_RANKING_LATER,
        },
        Command {
            id: "player-ranking-first",
            label: key::CMD_PLAYER_RANKING_FIRST,
        },
        Command {
            id: "player-ranking-last",
            label: key::CMD_PLAYER_RANKING_LAST,
        },
        Command {
            id: "player-ranking-layout",
            label: key::CMD_PLAYER_RANKING_LAYOUT,
        },
        // THE SCREENS' VERTICAL GESTURES (`S-<up>`/`S-<down>` on the
        // arrangement): a screen above or below its neighbours.
        Command {
            id: "display-rise",
            label: key::CMD_DISPLAY_RISE,
        },
        Command {
            id: "display-sink",
            label: key::CMD_DISPLAY_SINK,
        },
        // THE CLIPBOARD'S WINDOW: the second screen of the footer's history.
        Command {
            id: "clipboard-window",
            label: key::CMD_CLIPBOARD_WINDOW,
        },
    ];

    // `(sequence, command)`. The sequence does NOT include the leader: it has already
    // been consumed by the compositor when this process is told.
    //
    // THE LABEL IS NOT HERE. It used to be the third column, and a line copied
    // with the id changed and the key forgotten showed the neighbouring command's
    // text — while compiling. Now it comes from the REGISTRY, by id: one
    // piece of information, one place.
    const BINDINGS: &[(&str, &str, Option<&str>)] = &[
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
        // `c` IS THE CLIPBOARD AND NOTHING ELSE, and it is a COMMAND again.
        //
        // It was a group called "capture", holding the clipboard and the
        // screenshot: the clipboard captures what you copy, the screen
        // captures what you see. The owner unmade it — one word covering two
        // unrelated jobs is a group that reads well and answers badly, and it
        // cost the clipboard, which is used all day, a second keystroke to
        // house a screenshot, which is not. The screenshot went to `m`.
        //
        // `c` IS A GROUP AGAIN, of the clipboard's two doors: `c c` the
        // footer's region and `c w` the window — the `s n n` / `s n w` shape,
        // and a prefix with no command of its own, which is what the
        // `PrefixWithCommand` collision asks.
        ("c c", "clipboard-open", None),
        // `a` for APPLICATIONS, at the first level. It was free, and an
        // application launcher is a first-level thing: if it costs two
        // keys, the person goes back to opening everything through the terminal.
        (APPS_KEYS, "apps-open", None),
        // `LEADER t` IS THE TRANSFER QUEUE. `t t` opens the panel in the
        // footer's region, doubling the key the way `s n n` and `. .` already do
        // — because `t` is a PREFIX, and hanging a command off a prefix is the
        // collision `PrefixWithCommand` exists to flag.
        (TRANSFERS_KEYS, "transfers-open", None),
        // `t w` — the same queue, in a window. `w` for window, and it is the
        // second door of one thing, exactly like the file manager's `. w`.
        ("t w", "transfers-window", None),
        // `t a` — add whatever was copied: a link, a magnet, a `.torrent`.
        ("t a", "transfers-add", None),
        // AND THE THREE THAT SAY WHAT TO TAKE OFF A PAGE. `v` for video, `u`
        // for the middle letter of áudio — `a` is already the plain add —, and
        // `l` for legenda.
        ("t v", "transfers-video", None),
        ("t u", "transfers-audio", None),
        ("t l", "transfers-subtitles", None),
        // THE SAME FOUR, AS THE PERSON. Age-restricted, private, a members'
        // club and 1080p Premium need an account; everything else is better
        // without one.
        ("t i a", "transfers-add-as-me", None),
        ("t i v", "transfers-video-as-me", None),
        ("t i u", "transfers-audio-as-me", None),
        ("t i l", "transfers-subtitles-as-me", None),
        // `LEADER .` IS THE MANAGER'S PREFIX, and `.` is Doom's own key for
        // reaching a file.
        //
        // It held the footer's door DIRECTLY and the two doors also hung off a
        // group called `f`, so one thing was reachable by three sequences and
        // the letter `f` said nothing that `.` did not. The owner asked for the
        // one shape: *"não quero que `LEADER f` exista; quero que `LEADER .`
        // seja um submenu"*. `f` is free now.
        //
        // THE DOUBLED KEY IS THE CONVENTION, and it is the one `s n n`, `t t`
        // and `n n` already keep: the group's own first door repeats the key
        // that opened the group. It costs one keystroke more than the bare `.`
        // did and buys the second door a home that is not a second group.
        //
        // A MANAGER AND NOT A PICKER, which is what `f f` used to open: that
        // was reported from the screen as "a different and inferior file
        // manager", and the reading was right — whoever reaches for files is
        // looking for files, not for a flat list of names that can do one thing
        // with the one you choose.
        (". .", "files-open", None),
        // `w` for WINDOW — the same manager, in one of its own, because the
        // footer's region has a ceiling in lines (`space.expansion`,
        // invariant 14) and that is right for a footer and not for the job
        // people open a manager to do.
        //
        // It was `m` for "manager" and the letter was never written down; being
        // asked "why m?" is what earned it these lines — and the answer was that
        // `w` says which of the two DOORS it is, which is the actual difference.
        (". w", "files-window", None),
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
        // `p` FOR THE PLAYER: its three doors, the three questions (play,
        // queue, a video together), two of its pages, and the transport.
        ("p p", "player-open", None),
        ("p w", "player-window", None),
        ("p c", "player-column", None),
        ("p o", "player-play-file", None),
        ("p a", "player-enqueue", None),
        // `j` FOR JOINING a video to this sound: played together, in step.
        ("p j", "player-together", None),
        // `l` FOR THE LIBRARY: the files, from the music folder.
        ("p l", "player-browse", None),
        // `m` FOR MACHINES: the page of the other machines on the tailnet,
        // where pairing with one is a row.
        ("p m", "player-machines", None),
        // `e` FOR THE AMBIENT SOUNDS' page (the plan's key table), and `u`
        // for the sUbtitles over the picture — `s` is stop.
        ("p e", "player-ambience", None),
        ("p u", "player-subtitles", None),
        // `i` FOR THE TIMELINE (the plan's key table): `t` is the toggle.
        ("p i", "player-timeline", None),
        // `y` FOR THE LAYOUT of the pictures (the plan's key table).
        ("p y", "player-layout", None),
        // `d` FOR THE DATA ABOUT WHAT PLAYS — the metadata (the plan's key
        // table).
        ("p d", "player-metadata", None),
        // `r` FOR THE WORLD'S RADIO, `v` for its TV (`t` is the toggle) and
        // `g` for the channels grouped by theme (the plan's key table).
        ("p r", "player-radio", None),
        ("p v", "player-tv", None),
        ("p g", "player-channels", None),
        ("p t", "player-toggle", None),
        ("p n", "player-next", None),
        ("p b", "player-previous", None),
        ("p s", "player-stop", None),
        // `q` FOR QUIT: the player closed everywhere — the daemon, the
        // window, the column, the view.
        ("p q", "player-quit", None),
        ("m s", "screen-capture", None),
        ("m c", "camera-open", None),
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
        // `LEADER i` — THE PERSON'S OWN PROGRAMS: install, try, update and
        // remove, for this user only. `i` because the verb begins with it in
        // both languages, and it was free; `a` stays the bare launcher. `i i` is
        // the footer (the doubled letter every view here uses) and `i w` the
        // window; the other four open the view on one of its levels, and
        // `i b` — `b` for browser, since `w` is the window's letter — asks
        // for a web app's address first.
        (SOFTWARE_KEYS, "software-open", None),
        ("i w", "software-window", None),
        (SOFTWARE_TRIALS_KEYS, "software-trials", None),
        ("i b", "software-web-new", None),
        ("i u", "software-maintenance", None),
        ("i a", "software-android", None),
        // (`LEADER s u` — THE UPDATE, its view, its window and the sequence
        // now — is its window's own table, `update_window::BINDINGS`, taken
        // whole below.)
        // `LEADER s d` — THE SCREENS. `s d d` opens the view (the doubled
        // letter, the convention `s p p`, `s n n`, `t t` and `. .` already
        // use) and `s d w` the window; everything about ONE screen is a letter
        // INSIDE the view, where the row under the cursor says which screen
        // the letter is about.
        ("s d d", "display-open", None),
        ("s d w", "display-window", None),
        // `LEADER s p` — POWER, a group under SYSTEM. `s p p` opens the view
        // (the doubled letter) and `s p w` the window; the three profiles are
        // `e` (pErformance — `p` is the prefix itself), `b` balanced and `s`
        // saver; `k` keeps the machine awake, `l` ignores the lid, `t` opens
        // the view on the thresholds.
        ("s a a", "audio-open", None),
        ("s a w", "audio-window", None),
        ("s c c", "camera-settings-open", None),
        ("s c w", "camera-settings-window", None),
        ("s i i", "pointers-open", None),
        ("s i w", "pointers-window", None),
        ("s p p", "power-open", None),
        ("s p w", "power-window", None),
        ("s p e", "power-profile-performance", None),
        ("s p b", "power-profile-balanced", None),
        ("s p s", "power-profile-saver", None),
        ("s p k", "power-keep-awake", None),
        ("s p l", "power-keep-lid", None),
        ("s p t", "power-thresholds", None),
        // THE RADIO. Nine keys, and the nine are the ones that act on the
        // machine rather than on a row: everything about ONE device is a
        // letter inside the view, where the row under the cursor says which
        // device the letter is about.
        ("s b b", "bluetooth-open", None),
        ("s b w", "bluetooth-window", None),
        ("s b s", "bluetooth-scan", None),
        ("s b p", "bluetooth-power", None),
        ("s b d", "bluetooth-discoverable", None),
        ("s b c", "bluetooth-reconnect", None),
        ("s b x", "bluetooth-disconnect-all", None),
        // THE TWO ANSWERS, reachable from anywhere: a pairing question can
        // arrive while one is somewhere else entirely, and the two keys that
        // answer it are the brain's `b a`/`b p` arrangement.
        (PAIRING_YES_KEYS, "bluetooth-accept", None),
        (PAIRING_NO_KEYS, "bluetooth-refuse", None),
        // `t` for theme, under `s` because a theme is the SYSTEM's skin and not
        // any one buffer's: the same drawer as memory, format and the brain.
        ("s t", "theme-open", None),
        // `s l` — LANGUAGE, and the letter is free under `s`.
        // `w` for WINDOW, the letter every view with a second door uses.
        ("s l l", "locale-open", None),
        ("s l w", "locale-window", None),
        // `s g` — GEOGRAPHY, and the letter was free under `s`: the taken ones
        // are `a b d f h k l m n p q s t u w`. What it holds is data about the
        // MACHINE — which regions' streets are on this disk — which is the
        // sentence the `s` group is defined by.
        ("s g g", "geo-open", None),
        ("s g w", "geo-window", None),
        // `g` for GLANCE, at the first level: it was `d`, for date, until the
        // owner gave `d` to the development group. Bound through the constant
        // the sentences teach it by, so the two cannot drift apart.
        (GLANCE_KEYS, "world-glance", None),
        // `/` FOR DEVELOPMENT, the tools for whoever builds this distro, and
        // `r` resizes the screen. It was `d`, which the owner gave to the fast
        // access (2026-09-30): a key the person reaches for every day, where
        // `/` is one a builder of the distro learns once. Written out, and not
        // through a constant, so the check that every key reaches an arm of
        // `run_with` reads it.
        ("/ r", "dev-resizer", None),
        // `s q` — THE WAYS OUT, under SYSTEM at the owner's word.
        //
        // They held a letter of their own at the first level, and what they do
        // is what the machine does to ITSELF — which is the sentence the `s`
        // group is defined by, and the same drawer the power view, the update
        // and the theme already live in.
        ("s q q", "session-quit", None),
        // THE MACHINE'S FOUR WAYS OUT, beside the session's own. Each opens
        // the power view on its row, armed — `RET` confirms, `Esc` cancels.
        ("s q s", "power-suspend", None),
        ("s q h", "power-hibernate", None),
        ("s q r", "power-reboot", None),
        ("s q p", "power-off", None),
        // `LEADER :` — `M-x`'s key in Doom, which is where the concept
        // comes from. It used to be `x`, which in Doom is the scratch buffer and not the locator.
        // One key only: with both, the list showed the same thing on two
        // lines, and the list is the scarce resource here.
        (":", "mx-open", None),
        // NOTIFICATIONS. Both were socket orders with no key at all: the
        // history only opened through `ronoctua-shell history`, typed in a
        // terminal. A feature with no path is a feature nobody
        // finds.
        //
        // `n n` — the doubled letter, which is what `s n n`, `t t` and `. .`
        // already do for the view that belongs to a prefix. It was `n h` for
        // "history", and the owner asked for the convention instead.
        (NOTIF_OPEN_KEYS, "notif-open", None),
        ("n d", "notif-dismiss", None),
        // `w` for WINDOW, the same letter every view with a second door uses
        // — see the note over `s u w`.
        ("n w", "notif-window", None),
        ("n r", "notif-read-all", None),
        // `n f` — THE FILTERS, which had no door at all. They are the third
        // column of the manager and the only thing that named them was that
        // column's own title, so the feature existed for whoever pressed `TAB`
        // twice inside a screen they had already opened.
        ("n f", "notif-filters", None),
        // `h` for help, as in Doom — and a COMMAND, not a group: it opens the
        // list of manuals, which is where the help's content lives now, each
        // manual chosen there and none with a key of its own (the owner's rule).
        ("h", "help", None),
        // ---------------------------------------------------------------
        // NETWORK, under SYSTEM — `s n`, by the owner's decision: the radio
        // is the machine's, beside its memory, its disks and its power, and
        // the first-level `r` it used to hold was one more letter to learn.
        // `s n n` opens the list, and the others act directly.
        //
        // `s n n` and not `s n` alone because `s n` is a PREFIX: hanging a
        // command off it is having the collision `PrefixWithCommand` exists
        // to flag. Doubling the key is the convention `TAB TAB` already uses.
        // `s` scans, `r` is the RADIO — `w` was Wi-Fi, but `w` is the second
        // door's letter everywhere else in this keymap.
        // ---------------------------------------------------------------
        // THE SHELL'S HISTORY. `s h` was the one letter of the `s` group still
        // free, and history is the machine doing something to ITSELF — which is
        // the sentence that defines the group.
        ("s h h", "atuin-open", None),
        ("s h w", "atuin-window", None),
        ("s h s", "atuin-sync", None),
        ("s n n", "network-open", None),
        // AND `w` IS THE WINDOW, as it is for the files, the notifications,
        // the power and the radio: the same view, through the other door.
        ("s n w", "network-window", None),
        ("s n a", "network-airplane", None),
        ("s n s", "network-scan", None),
        ("s n r", "network-wifi-toggle", None),
        ("s n d", "network-disconnect", None),
        // `i` for INFORMATION. It shows band, rate and signal in the minibuffer —
        // it is the answer to "why is this slow?" without leaving the screen.
        ("s n i", "network-link", None),
        // `l` FOR THE OTHERS ON THIS NETWORK. It takes a second on a thread
        // and the row says so while it runs.
        ("s n l", "network-lan", None),
        // `v` FOR A VPN FROM A FILE. It reads the file, says what it would do,
        // and only then offers to bring it in.
        ("s n t", "network-tor-open", None),
        ("s n v", "network-import", None),
        // SYNCTHING is `s s` — its own prefix under SYSTEM, beside the
        // network it rides on, and no longer inside it.
        //
        // THE DOUBLED LETTER IS THE FOOTER AND `w` IS THE WINDOW, as it is for
        // the files, the notifications, the power, the radio, the network and
        // the sound. `s s s` used to be the one-line status echo; that moved
        // to `s s t`, because the doubled letter belongs to the door.
        ("s s s", "syncthing-open", None),
        ("s s w", "syncthing-window", None),
        ("s s a", "syncthing-web", None),
        ("s s t", "syncthing-state", None),
        ("s s p", "syncthing-pause-all", None),
        ("s s r", "syncthing-resume-all", None),
        ("s s d", "syncthing-add-device", None),
        ("s s f", "syncthing-add-folder", None),
        // `s r` — THE FOOTER'S OWN SETTINGS, on a letter of `s` still free:
        // the doubled key is the footer's door, `w` the window's.
        ("s r r", "footer-settings-open", None),
        ("s r w", "footer-settings-window", None),
        // `i` for ICONS, and not `g` for glyphs: the group holds the three
        // collections. `m i i` and `m i w` are the two doors, so the group
        // folds into `LEADER m i`; a screen of its own is `m i n|l|u`, in the
        // whole tree.
        ("m i i", "icons-open", None),
        ("m i w", "icons-window", None),
        ("m i n", "icons-nerd-fonts", None),
        ("m i l", "icons-lucide", None),
        ("m i u", "icons-unicode", None),
        // `=` — THE CALCULATOR, the key a sum ends with: the doubled key is
        // the footer's door, `w` the window's.
        ("= =", "calc-open", None),
        ("= w", "calc-window", None),
        // `h` FOR A PAGE OF THE WEB — a video from a website, by its
        // address (`crate::player_website`).
        ("p h", "player-website", None),
        // `v` — THE VIDEO CATALOGUE, on a top-level letter the design left free:
        // the doubled key is the footer's door, `w` the window's, `s` a scan now.
        ("v v", "video-metrics-open", None),
        ("v w", "video-metrics-window", None),
        ("v s", "video-metrics-scan", None),
        // `s e` — THE SECURITY CENTRE: the doubled key the footer's door, `w`
        // the window's; then the window without a password, the shields, the
        // firewall, and the three online checks.
        ("s e e", "security-open", None),
        ("s e w", "security-window", None),
        ("s e r", "security-relax", None),
        ("s e b", "security-shields", None),
        ("s e f", "security-firewall", None),
        ("s e p", "security-check-password", None),
        ("s e m", "security-check-email", None),
        ("s e v", "security-check-file", None),
        // `s o` — THE PERSON'S CONTAINERS, on a letter of `s` still free: the
        // doubled key is the footer's door, `w` the window's.
        ("s o o", "containers-open", None),
        ("s o w", "containers-window", None),
        // THE BELONGINGS: the view in the footer, in a window, and a sync
        // from anywhere — the passphrase asked on the door's own line.
        ("s v v", "belongings-open", None),
        ("s v w", "belongings-window", None),
        ("s v s", "belongings-sync", None),
        // `d` — THE SEARCH OVER EVERYTHING: the doubled key the footer's door,
        // `w` the window's, `c` the column's; and `SPC`, the window floating
        // over the whole room — the one key a search should be.
        ("d d", "fast-access-open", None),
        ("d w", "fast-access-window", None),
        ("d c", "fast-access-column", None),
        ("SPC", "fast-access-float", None),
        // `k` FOR THE RANKING of the video catalogue — its videos, best
        // first, a click from the queue (`crate::player_ranking`).
        ("p k", "player-ranking", None),
        // THE CLIPBOARD, TWO DOORS: `c c` the footer, `c w` the window — and
        // folded, `LEADER c` is still the footer and `LEADER LEADER c` the
        // window (`folds`).
        ("c w", "clipboard-window", None),
    ];

    /// The digits as `&'static str`, so the binding can carry them.
    ///
    /// `format!` would give a `String`, and the binding needs `'static`. Nine
    /// literals cost less than leaking memory to fit the type.
    const DIGITS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

    const GROUPS: &[(&str, &str)] = &[
        // `.` FOR FILES, and it is the only prefix in this table that is not a
        // letter.
        //
        // It was `f`, which said nothing `.` does not and left one thing
        // reachable three ways; the owner asked for the group to be `.` itself.
        // A prefix is a prefix whatever character it is — the tree is built
        // from the sequences and `TAB .` already put a `.` in one — and what a
        // group owes is a LABEL, which this one has.
        (".", key::GROUP_FILE),
        ("w", key::GROUP_WINDOW),
        ("RET", key::GROUP_TERMINAL),
        ("TAB", key::GROUP_WORKSPACE),
        ("TAB m", key::GROUP_WORKSPACE_MOVE),
        // `m` FOR MISCELLANEOUS, and `m i` for the browsers inside it. See the
        // bindings for why the trio keeps a prefix of its own.
        ("m", key::GROUP_MISCELLANEOUS),
        ("p", key::GROUP_PLAYER),
        ("m i", key::GROUP_ICONS),
        // `t` FOR TRANSFERS. It held the screen capture alone and gave the
        // letter up: a queue that everything long goes through earns a
        // first-level prefix, and a single screenshot does not.
        ("t", key::GROUP_TRANSFER),
        // `t i` — AS ME. A prefix of its own and not a switch kept somewhere
        // else: what it costs is anonymity, and a cost paid by a setting is a
        // cost nobody notices paying. Four keys under it, one per thing that
        // can be asked for, so the sequence itself says which of the two
        // happened.
        ("t i", key::GROUP_IDENTIFIED),
        ("n", key::GROUP_NOTIFICATION),
        // `i` FOR THE PERSON'S OWN PROGRAMS — see the bindings.
        ("i", key::GROUP_SOFTWARE),
        // `/` FOR DEVELOPMENT: tools for whoever builds this distro — the
        // screen resizer today, and more to come. It was `d` (which held the
        // glance before that, gone to `g`); `d` and `SPC` at the root are the
        // fast access's now, and this group leaves them free.
        ("/", key::GROUP_DEV),
        // `s` for SYSTEM — what the machine is doing to itself, as opposed to
        // what one is doing with it: memory, disks, the update, the theme —
        // and the network and Syncthing, which moved in from the first level.
        ("s", key::GROUP_SYSTEM),
        ("s h", key::GROUP_ATUIN),
        ("s n", key::GROUP_NETWORK),
        ("s s", key::GROUP_SYNCTHING),
        ("s d", key::GROUP_DISPLAY),
        ("s a", key::GROUP_AUDIO),
        ("s c", key::GROUP_CAMERA),
        ("s i", key::GROUP_POINTERS),
        ("s p", key::GROUP_POWER),
        ("s b", key::GROUP_BLUETOOTH),
        ("s l", key::GROUP_LOCALE),
        ("s g", key::GROUP_GEO),
        ("s u", key::GROUP_UPDATE),
        ("s q", key::GROUP_QUIT),
        ("s r", key::GROUP_FOOTER_SETTINGS),
        ("=", key::GROUP_CALC),
        ("v", key::GROUP_VIDEO_METRICS),
        ("s e", key::GROUP_SECURITY),
        ("s o", key::GROUP_CONTAINERS),
        ("s v", key::GROUP_BELONGINGS),
        ("d", key::GROUP_FAST_ACCESS),
        ("c", key::GROUP_CLIPBOARD),
    ];

    let mut registry = Registry::fresh();
    // THE UPDATE'S COMMANDS ARE ITS WINDOW'S (`update_window::COMMANDS`), and
    // they are registered here like every other: the leader spells none of
    // them, and the window is the one file outside the update's module that
    // may.
    for c in COMMANDS.iter().chain(crate::update_window::COMMANDS) {
        registry.register(*c)?;
    }

    // THE BRAIN MODULES' ACTIONS, each as a named command — generated, and
    // naming no module: `<module>-<action>`, labelled by the module's own
    // `cmd-<module>-<action>` (see `a_module_action`).
    for (_, action) in ronoctua_brain::compiled::ACTIONS {
        registry.register(Command {
            id: action.command,
            label: action.label,
        })?;
    }

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
    for d in ronoctua_scroll::deeds::DEEDS.iter() {
        registry.register(Command {
            id: d.id,
            label: d.key,
        })?;
    }

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
    let ranges = (1..=9)
        .map(|n| (format!("TAB {n}"), "workspace-go", DIGITS[n - 1]))
        // AND AT THE TOP LEVEL, one key shorter — the owner's `LEADER 1`…`9`,
        // the same numbers the modeline draws for the workspaces that hold
        // something.
        .chain((1..=9).map(|n| (n.to_string(), "workspace-go", DIGITS[n - 1])))
        .chain((1..=9).map(|n| (format!("TAB m {n}"), "workspace-move", DIGITS[n - 1])))
        .chain([
            ("TAB 0".to_owned(), "workspace-go", LAST),
            ("TAB m 0".to_owned(), "workspace-move", LAST),
        ]);

    // AND THE WINDOW MENU'S, generated from the menu itself — see
    // `window_menu::leader_bindings`: every item it offers, on its own key,
    // under `w`. What `w` already binds to the SAME command is the menu
    // adopting the group's key, and is not bound twice; a key the two gave
    // different commands is a collision, which the tree refuses below.
    let (menu_bindings, menu_groups) = crate::window_menu::leader_bindings();
    let written: Vec<(String, &str, Option<&str>)> = BINDINGS
        .iter()
        // AND THE UPDATE'S, from its window's table — see the commands above.
        .chain(crate::update_window::BINDINGS)
        .map(|(s, c, a)| ((*s).to_owned(), *c, *a))
        .chain(ranges.map(|(s, c, a)| (s, c, Some(a))))
        .collect();
    let generated = menu_bindings
        .iter()
        .filter(|g| !written.contains(g))
        .cloned();
    let mut bindings = Vec::with_capacity(written.len() + menu_bindings.len());
    for (seq, command, argument) in written.iter().cloned().chain(generated) {
        let cmd = registry.search(command).ok_or_else(|| {
            anyhow::anyhow!(
                "the binding `{seq}` points at the command `{command}`, which does not exist"
            )
        })?;
        // A COMMAND WITH AN ARGUMENT ONLY ACCEPTS A BINDING WITH AN ARGUMENT, and the
        // converse too: `workspace-go` with no number does not know where to go, and
        // `buffer-close` with a number receives something it silently ignores.
        anyhow::ensure!(
            crate::actions::with_argument(command).is_some() == argument.is_some(),
            "the binding `{seq}` and the command `{command}` disagree about the argument"
        );
        bindings.push(Binding {
            sequence: parse_sequence(&seq)?,
            command,
            label: cmd.label,
            argument,
        });
    }

    let groups: Result<Vec<_>, _> = GROUPS
        .iter()
        .map(|(seq, label)| (*seq, *label))
        .chain(
            menu_groups
                .iter()
                .map(|(seq, label)| (seq.as_str(), *label)),
        )
        .map(|(seq, label)| parse_sequence(seq).map(|sequence| GroupNode { sequence, label }))
        .collect();

    // THE THREE SETS, each built and each verified: `BINDINGS` and `GROUPS`
    // are the full tree as written, and the other two are derived from it
    // (`collapse`) — see `Set` for why there are three.
    let trees = Trees::build(&bindings, &groups?).map_err(|collisions| {
        let list = collisions
            .iter()
            .map(|(set, c)| format!("  - in the {set:?} set: {c}"))
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::anyhow!("the default keymap has a collision:\n{list}")
    })?;

    Ok((trees, registry))
}

/// The launcher's own key: put the focused application on the file manager's
/// sidebar.
///
/// `C-` and not a bare letter, for the reason the manager's own table gives at
/// length: in a list of NAMES the letter is query, so anything that is a
/// command has to be something a name never is. In the manager's MENU the same
/// idea is a bare `s`, because a menu row is not a list of names.
/// The glance's key, BESIDE its binding.
///
/// A sentence that needs to teach it interpolates this plus `crate::leader()`
/// — never the letters written out. A key spelt into a `.ftl` ages the day the
/// key moves, and `n h` taught `n h` for weeks after it became `n n`.
pub const GLANCE_KEYS: &str = "g";

/// The software view's keys in the footer, BESIDE its binding.
pub const SOFTWARE_KEYS: &str = "i i";

/// The software view's trials, BESIDE its binding — for a sentence that
/// names where the trials are (the modeline's trial mark leads there too).
pub const SOFTWARE_TRIALS_KEYS: &str = "i t";

/// The launcher's key, BESIDE its binding — the software view points at it
/// for the system's own programs.
pub const APPS_KEYS: &str = "a";

/// The transfer queue's footer panel, BESIDE its binding, in the full tree.
///
/// What a sentence teaches is NOT this constant but [`taught_keys`]'s answer
/// for `transfers-open`: `t` is the queue itself in the default set, and
/// `t t` typed there is the queue and then a key that leads nowhere.
pub const TRANSFERS_KEYS: &str = "t t";

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
/// `ronoctua_brain::compiled::ACTIONS`).
///
/// # Why they are commands, and reached by name only
///
/// A notice's button is an action, and an action is a COMMAND of the registry
/// rather than a special case inside the strip: `LEADER :` runs it by name,
/// the brain's `, n` and the strip's click reach it through `brain-act`, and a
/// question an action asks comes back as the command's argument. The list is
/// the generated one, so a module's folder adds its actions here without this
/// file naming it — and none of them has a sequence, for the reason the Emacs
/// watch's button gave first: a key for it would be a key that does nothing on
/// every machine where there is nothing to say, which is almost all of them,
/// almost all of the time. So both orphan checks (`check-keymap` and the test
/// beside it) excuse them the way they excuse `BY_NAME_ONLY`.
#[must_use]
pub fn a_module_action(id: &str) -> bool {
    ronoctua_brain::compiled::ACTIONS
        .iter()
        .any(|(_, action)| action.command == id)
}

/// The action a module action's command names, out of the generated list —
/// what `run_with` sends when the footer holds no mind to find it in.
///
/// A COMMAND THE REGISTRY LISTS ANSWERS, mind or not: before the brain has
/// spoken (or with no brain at all) it fell through to "does nothing yet",
/// which is the one thing it is not. With this it is sent, and the door says
/// what came back — the brain's answer, or that the brain is not answering.
#[must_use]
pub fn module_action(id: &str) -> Option<crate::brain::Acting> {
    ronoctua_brain::compiled::ACTIONS
        .iter()
        .find(|(_, action)| action.command == id)
        .map(|(module, action)| crate::brain::Acting {
            module: (*module).to_owned(),
            action: action.id.to_owned(),
            command: id.to_owned(),
            asks: action.asks.map(str::to_owned),
        })
}

/// A module action's command is found without a mind, and a command that is
/// not one is not.
///
/// THE DEFECT THIS CATCHES: `LEADER : <module>-<action>` before the brain had
/// spoken — or with no brain — answered "does nothing yet", because only the
/// mind was asked what the command was. Every generated action, and no id is
/// written here: naming a module outside its folder is what
/// `brain-modules-are-sealed` refuses.
#[cfg(test)]
mod module_action_tests {
    #[test]
    fn every_generated_action_is_found_without_a_mind() {
        assert!(
            !ronoctua_brain::compiled::ACTIONS.is_empty(),
            "no module declares an action, and this proves nothing"
        );
        for (module, action) in ronoctua_brain::compiled::ACTIONS {
            let found = super::module_action(action.command)
                .unwrap_or_else(|| panic!("`{}` was not found", action.command));
            assert_eq!(found.module, *module);
            assert_eq!(found.action, action.id);
            assert_eq!(found.command, action.command);
            assert_eq!(found.asks.as_deref(), action.asks);
            assert!(super::a_module_action(action.command));
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
// owner photographed. See `crate::clipboard_view::OF_THE_VIEW`.

/// The commands the THEME CHOOSER reaches, with each one's key — one. `RET`
/// applies the pair under the two cursors; `<up>`/`<down>` preview,
/// `<left>`/`<right>` cross between the themes and the desk, and `Esc` puts
/// the committed pair back — the view's own gestures rather than commands,
/// as in every other list.
pub const OF_THEME_VIEW: &[(&str, &str)] = &[("RET", "theme-apply")];

/// Every command SOME key in this system reaches.
///
/// # Why it is not the keymap alone
///
/// The keymap resolves SEQUENCES, and sequences are not the only source of
/// keys here. The **chords** live in the compositor's config (the footer never
/// sees them — §7.4); the **emergency** binds live outside the leader on
/// purpose; and every VIEW that takes the keyboard has a table of its own,
/// because `d` in a list removes and is not `LEADER d`.
///
/// So this unions ten sources: the tree, the chords, the emergency binds, the
/// launcher's, the clipboard's, the theme chooser's, and the five views that
/// declare an `OF_THE_VIEW` — the notifications manager, its sheet, the power
/// view, the radio and the network. Asking "does this have a shortcut?" of the
/// keymap alone answers wrongly for most of them, which is what the orphan
/// check would then report.
///
/// The header used to say "the three places" and sat, with no blank line, on
/// top of two other items' documentation — so `crate::clipboard_view::OF_THE_VIEW` and this
/// function were both undocumented and a reader met the wrong text over each.
#[must_use]
pub fn reachable(tree: &Tree) -> std::collections::BTreeSet<&'static str> {
    let mut out: std::collections::BTreeSet<&'static str> =
        tree.reachable_commands().into_iter().collect();
    out.extend(crate::actions::full_chords().into_iter().map(|a| a.command));
    out.extend(
        crate::actions::emergency()
            .into_iter()
            .map(|(_, _, id)| id)
            // The alternative leader is not a command: it fires the sequence.
            .filter(|id| *id != crate::actions::LEADER_ALTERNATE),
    );
    out.extend(OF_LAUNCHER_VIEW.iter().map(|(_, id)| *id));
    out.extend(crate::history::OF_THE_VIEW.iter().map(|(_, id)| *id));
    out.extend(crate::clipboard_view::OF_THE_VIEW.iter().map(|(_, id)| *id));
    out.extend(
        crate::notifications::OF_THE_VIEW
            .iter()
            .map(|(_, id, _)| *id),
    );
    out.extend(crate::notifications::OF_THE_SHEET.iter().map(|(_, id)| *id));
    out.extend(crate::notifications::OF_THE_OPEN.iter().map(|(_, id)| *id));
    out.extend(crate::notifications::OF_THE_DRAFT.iter().map(|(_, id)| *id));
    out.extend(
        crate::notifications::OF_THE_QUESTION
            .iter()
            .map(|(_, id)| *id),
    );
    out.extend(crate::power::OF_THE_VIEW.iter().map(|(_, id)| *id));
    out.extend(crate::audio::OF_THE_VIEW.iter().map(|(_, id, _)| *id));
    out.extend(crate::pointers::OF_THE_VIEW.iter().map(|(_, id, _)| *id));
    out.extend(
        crate::camera_settings::OF_THE_VIEW
            .iter()
            .map(|(_, id, _)| *id),
    );
    // THE CAMERA APP'S KEYS, but the three that are the WINDOW's own (the
    // sound drawer, fit or fill, pinning the controls): those mean nothing
    // anywhere else and are not commands of the registry.
    out.extend(
        crate::camera::OF_THE_VIEW
            .iter()
            .map(|(_, id, _)| *id)
            .filter(|id| !crate::camera::LOCAL.contains(id)),
    );
    out.extend(crate::syncthing::OF_THE_VIEW.iter().map(|(_, id, _)| *id));
    out.extend(crate::player::OF_THE_VIEW.iter().map(|(_, id, _)| *id));
    // THE KEYBOARD'S TOP ROW, which the compositor binds directly — the same
    // arrangement as the four emergency keys just above.
    out.extend(crate::actions::media().into_iter().map(|(_, _, id)| id));
    out.extend(crate::locale::OF_THE_VIEW.iter().map(|(_, id)| *id));
    out.extend(crate::geo::OF_THE_VIEW.iter().map(|(_, id)| *id));
    out.extend(crate::display::OF_THE_VIEW.iter().map(|(_, id, _)| *id));
    out.extend(crate::bluetooth::OF_THE_VIEW.iter().map(|(_, id, _)| *id));
    out.extend(crate::network::OF_THE_VIEW.iter().map(|(_, id, _)| *id));
    out.extend(crate::software::OF_THE_VIEW.iter().map(|(_, id, _)| *id));
    out.extend(OF_THEME_VIEW.iter().map(|(_, id)| *id));
    // THE KIOSK'S VIEW AND ITS `,` GROUP, one table (`kiosk_view::ROOT`) and
    // its two lists.
    out.extend(crate::kiosk_view::reached());

    // THE FOOTER SETTINGS' KEYS, one table (`footer_settings::OF_THE_VIEW`).
    out.extend(crate::footer_settings::reached());

    // THE ICONS APP: its question, reached from its page's colour row.
    out.extend(crate::icons::reached());

    // THE VIDEO CATALOGUE'S KEYS AND QUESTIONS, one table each
    // (`video_metrics::OF_THE_VIEW`, `video_metrics::Ask`).
    out.extend(crate::video_metrics::reached());

    // THE SECURITY CENTRE'S KEYS, ROWS AND QUESTIONS (`security::reached`).
    out.extend(crate::security::reached());

    // THE CONTAINERS' KEYS, one table (`containers::OF_THE_VIEW`).
    out.extend(crate::containers::reached());

    // THE BELONGINGS' KEYS AND ROWS, one table each (`belongings::OF_THE_VIEW`,
    // `OF_THE_ROWS`).
    out.extend(crate::belongings::reached());

    // THE SEARCH'S COMMANDS: its doors, and what a door of it hands the
    // footer (`fast_access::COMMANDS`).
    out.extend(crate::fast_access::reached());

    out
}

#[cfg(test)]
mod tests {
    /// THE STATE PROTOCOL, both ways, against a socket that answers like a buffer.
    ///
    /// Two hand-rolled parsers face each other here — the buffer writes the
    /// answer in `publish_keymap` and the footer reads it in `buffer_states_at`
    /// — and nothing but this ties them together. The shape is deliberately
    /// unlike the keymap's: the FIRST line is the current state and the rest are
    /// the list, so a reader that treated every line the same would come back
    /// with a state called `normal n` and a menu missing its first row.
    #[test]
    fn the_footer_reads_a_buffers_states_and_asks_for_one() {
        use std::io::{BufRead as _, Write as _};

        let path =
            std::env::temp_dir().join(format!("ronoctua-states-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind");

        let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let seen = std::sync::Arc::clone(&asked);
        let served = std::thread::spawn(move || {
            for _ in 0..3 {
                let Ok(mut stream) = listener.accept().map(|(s, _)| s) else {
                    return;
                };
                let mut line = String::new();
                let clone = stream.try_clone().expect("clone");
                std::io::BufReader::new(clone)
                    .read_line(&mut line)
                    .expect("read");
                let reply = match line.trim() {
                    "states" => "filter warning\nnormal n\nfilter f\nsheet s".to_owned(),
                    order => {
                        seen.lock().expect("lock").push(order.to_owned());
                        "ok".to_owned()
                    }
                };
                writeln!(stream, "{reply}").expect("reply");
            }
        });

        let (now, tone, listed) = super::buffer_states_at(&path).expect("read the states");
        // THE CURRENT ONE IS THE FIRST LINE and is NOT in the list — that is the
        // whole reason the shape differs from the keymap's. Its tone rides on
        // the same line.
        assert_eq!(now, "filter");
        assert_eq!(tone, ronoctua_protocol::Tone::Warning);
        assert_eq!(
            listed,
            vec![
                ("normal".to_owned(), 'n'),
                ("filter".to_owned(), 'f'),
                ("sheet".to_owned(), 's'),
            ]
        );

        super::buffer_state_at(&path, "state sheet").expect("ask for the state");
        super::buffer_state_at(&path, "state-next").expect("cycle");
        served.join().expect("the buffer's thread");
        assert_eq!(
            *asked.lock().expect("lock"),
            vec!["state sheet".to_owned(), "state-next".to_owned()]
        );
        let _ = std::fs::remove_file(&path);
    }

    /// NOBODY LISTENING IS `None`, and not an error.
    ///
    /// Most focused windows are not our buffers, and this runs on every event
    /// the compositor sends. An error here would put a warning in the journal
    /// per window that is not ours, forever — and would teach whoever reads it
    /// to ignore the ones that matter.
    #[test]
    fn a_window_that_is_not_ours_answers_nothing() {
        let path = std::env::temp_dir().join("ronoctua-does-not-exist.sock");
        let _ = std::fs::remove_file(&path);
        assert!(super::buffer_states_at(&path).is_none());
    }

    /// THE TWO DIRECTIONS, against a socket that answers like a buffer.
    ///
    /// It is one test and not two on purpose: listing without running is the
    /// defect this whole path exists to avoid, so the pair is the unit.
    #[test]
    fn the_footer_reads_a_buffer_keymap_and_runs_one_of_its_commands() {
        use std::io::{BufRead as _, Write as _};

        let path = std::env::temp_dir().join(format!("ronoctua-teste-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind");

        // A stand-in for the buffer's own thread, answering the same protocol.
        let ran = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let seen = std::sync::Arc::clone(&ran);
        let served = std::thread::spawn(move || {
            for _ in 0..2 {
                let Ok(mut stream) = listener.accept().map(|(s, _)| s) else {
                    return;
                };
                let mut line = String::new();
                let clone = stream.try_clone().expect("clone");
                std::io::BufReader::new(clone)
                    .read_line(&mut line)
                    .expect("read");
                let reply = match line.trim() {
                    "keymap" => "m f s save-file\nm f r reload-file".to_owned(),
                    order => {
                        seen.lock().expect("lock").push(order.to_owned());
                        "ok".to_owned()
                    }
                };
                writeln!(stream, "{reply}").expect("reply");
            }
        });

        let keymap = super::buffer_keymap_at(&path).expect("read the keymap");
        assert_eq!(
            keymap,
            vec![
                ("m f s".to_owned(), "save-file".to_owned()),
                ("m f r".to_owned(), "reload-file".to_owned()),
            ],
            "the sequence carries spaces, so only the FIRST one separates the id from it"
        );

        super::buffer_command_at(&path, "save-file").expect("run the command");
        served.join().expect("the fake buffer's thread");
        assert_eq!(
            *ran.lock().expect("lock"),
            vec!["command save-file".to_owned()],
            "the command has to arrive on the buffer's side, not just be sent"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// No buffer listening is a NORMAL answer, and not an error: most focused
    /// windows are not ours, and `SPC m` has to be absent there rather than
    /// showing a failure.
    #[test]
    fn no_buffer_listening_gives_an_empty_keymap() {
        let path = std::env::temp_dir().join("ronoctua-really-does-not-exist.sock");
        let _ = std::fs::remove_file(&path);
        assert!(
            super::buffer_keymap_at(&path)
                .expect("it cannot be an error")
                .is_empty()
        );
    }

    use super::*;
    use ronoctua_keymap::{Key, Sym};

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

    /// When the CASE is the reason, the message has to say so.
    ///
    /// `CTRL ALT SPC I is not bound` is true and is useless: it shows the
    /// consequence and hides the cause, and sends the person looking for a shortcut
    /// they have just pressed. Whoever pressed Shift by accident only finds out
    /// if the screen tells them that the other case is the one that exists.
    #[test]
    fn the_message_points_at_the_other_case() {
        let (tree, _) = default_of().expect("the default keymap has to close");
        let mut l = Leader::default();
        l.activate();
        // `M` and not `I`: the letter is only an EXAMPLE of a first-level
        // binding, and the icons moved under `m` when `m` was made the drawer
        // for what belongs nowhere else.
        match l.key(&tree, Key::simple(Sym::Char('M'))) {
            Effect::Close {
                other_case: Some(t),
            } => assert_eq!(
                t.sym,
                Sym::Char('m'),
                "the suggestion has to be the lower-case one"
            ),
            otherwise => panic!("`M` should end pointing at `m`; got {otherwise:?}"),
        }
    }

    #[test]
    fn with_no_other_case_there_is_no_suggestion() {
        // The suggestion must not become noise: a key that exists in no case at
        // all ends without pointing at anything, and one that HAS no case (a digit,
        // punctuation) too.
        let (tree, _) = default_of().expect("the default keymap has to close");
        // `¬` AND NOT `=`, which the calculator took (`LEADER =`).
        for c in ['ç', '¬'] {
            let mut l = Leader::default();
            l.activate();
            assert!(
                matches!(
                    l.key(&tree, Key::simple(Sym::Char(c))),
                    Effect::Close { other_case: None }
                ),
                "`{c}` exists in no case at all; there is nothing to suggest"
            );
        }
    }

    /// `CTRL ALT SPC M-n is not bound` — the owner's second screen.
    ///
    /// The `M-` is Alt. In the nested loop the leader **is** `Ctrl+Alt+Space`, so Ctrl
    /// and Alt are held down by construction when the sequence begins; and after
    /// a host shortcut (`Super+Space`) the nested compositor may miss the key's
    /// release and announce it forever. In both cases every binding
    /// stops matching at once.
    #[test]
    fn the_message_points_at_the_key_with_no_modifier() {
        use ronoctua_keymap::Mods;
        let (tree, _) = default_of().expect("the default keymap has to close");
        let with_alt = Key {
            mods: Mods {
                ctrl: false,
                alt: true,
                shift: false,
                logo: false,
            },
            sym: Sym::Char('m'),
        };
        // The premise: `M-m` is not bound to anything, and `m` is. The letter
        // is only an example of a first-level binding; the icons' `i` was it
        // until `m` became the drawer they moved into.
        assert!(matches!(
            tree.footer.resolve(&[with_alt]),
            ronoctua_keymap::Resolution::Nothing
        ));

        let mut l = Leader::default();
        l.activate();
        match l.key(&tree, with_alt) {
            Effect::Close {
                other_case: Some(t),
            } => {
                assert_eq!(t.sym, Sym::Char('m'));
                assert!(
                    !t.mods.alt,
                    "the suggestion has to be the key WITHOUT the Alt"
                );
            }
            otherwise => panic!("`M-m` should end pointing at `m`; got {otherwise:?}"),
        }
    }

    #[test]
    fn the_other_case_runs_nothing() {
        // THE SUGGESTION IS NOT A FALLBACK. `w H` moves the window and `w h` focuses: falling
        // from one to the other would make the shortcut do SOMETHING ELSE, which is worse than
        // doing nothing. Here `w` followed by a non-existent key ends the sequence —
        // never `Run`.
        let (tree, _) = default_of().expect("the default keymap has to close");
        let mut l = Leader::default();
        l.activate();
        assert!(matches!(
            l.key(&tree, Key::simple(Sym::Char('w'))),
            Effect::Continue { .. }
        ));
        // `T` is not in `w`, but `t` is (`window-float`).
        assert!(
            matches!(
                l.key(&tree, Key::simple(Sym::Char('T'))),
                Effect::Close {
                    other_case: Some(_)
                }
            ),
            "it has to end pointing at `t`, and NOT run `window-float`"
        );
    }

    #[test]
    fn the_default_keymap_does_not_collide() {
        // This is milestone 5's "keymap collision check". It runs in
        // `nix flake check` through `rust-tests`, and also through the
        // `check-keymap` subcommand, which exists for whoever edits their own keymap.
        // It asserts the INVARIANT, and not a fixed count: a hard-coded number
        // breaks on every new command and teaches people to update the test without reading
        // what it says — which is how a check stops verifying.
        let (tree, registry) = default_of().expect("the default keymap has to close");

        assert!(tree.full.amount() > 0, "an empty keymap");

        // NO ORPHAN COMMAND: every command in the registry has SOME key path
        // to it.
        //
        // "Path" used to be only the sequence. It is not any more: a command can be
        // reached by SEQUENCE (`LEADER w h`), by CHORD (`LEADER+←`, which
        // the compositor resolves and the keymap never sees) or by EMERGENCY
        // BIND (`Ctrl+Shift+L`, which exists outside the leader precisely
        // for when it does not respond). Counting only the first would flag an orphan
        // where there is a shortcut, and the fix would be inventing a sequence
        // nobody asked for.
        //
        // What still holds, and is what matters: no command with no
        // path at all. Being reachable by `M-x` alone is not enough — a command that only
        // gets typed is a command only whoever read the code knows exists.
        let reachable = super::reachable(&tree.full);
        for c in registry.all_ones() {
            assert!(!c.id.is_empty(), "a command with no id");
            assert!(!c.label.is_empty(), "a command with no label key");
            // THE COMPOSITOR'S DEEDS ARE THE EXCEPTION, and the only one.
            //
            // They exist to be reachable by NAME (section 13); giving keys to
            // all of them would spend the whole keymap on what is almost never
            // used. The exception is recognisable by its prefix, so it cannot
            // grow without someone writing `wm-` on purpose.
            if c.id.starts_with("wm-") || BY_NAME_ONLY.contains(&c.id) || a_module_action(c.id) {
                continue;
            }
            assert!(
                reachable.contains(&c.id),
                "`{}` is OUR command and no key reaches it: \
             no sequence, no chord, no emergency bind",
                c.id
            );
        }

        // And the converse: nothing points at a command that does not exist. `default_of()`
        // already refuses to build in that case; this proves the refusal is wired up.
        for id in reachable {
            assert!(
                registry.search(id).is_some(),
                "there is a key pointing at `{id}`, which is not in the registry"
            );
        }
    }

    #[test]
    fn esc_always_ends_it() {
        // Even with the sequence half-typed. Without an unconditional exit, a
        // lost user has no way back.
        let (tree, _) = default_of().unwrap();
        let mut l = Leader::default();
        l.activate();
        // `s` and not `b`: `b` was a prefix until `LEADER b` became the brain
        // itself, and a command ENDS the sequence, which is the opposite of
        // what this is testing.
        assert!(matches!(
            l.key(&tree, Key::simple(Sym::Char('s'))),
            Effect::Continue { .. }
        ));
        assert!(matches!(
            l.key(&tree, Key::simple(Sym::Esc)),
            Effect::Close { .. }
        ));
        assert!(!l.active_one());
    }

    #[test]
    fn a_key_with_no_binding_ends_instead_of_hanging() {
        let (tree, _) = default_of().unwrap();
        let mut l = Leader::default();
        l.activate();
        assert!(matches!(
            l.key(&tree, Key::simple(Sym::Char('ç'))),
            Effect::Close { .. }
        ));
        assert!(!l.active_one());
    }

    #[test]
    fn a_complete_sequence_runs_and_leaves() {
        let (tree, _) = default_of().unwrap();
        let mut l = Leader::default();
        l.activate();
        // `s q h` — THREE keys, which is also what this test wants: a sequence
        // that runs has to be walked to the end, and a two-key one cannot tell
        // "the last key ran it" from "any key after a group runs it". `s q`
        // has one door, so it does not fold in the set a leader starts in.
        l.key(&tree, Key::simple(Sym::Char('s')));
        l.key(&tree, Key::simple(Sym::Char('q')));
        assert_eq!(
            l.key(&tree, Key::simple(Sym::Char('h'))),
            Effect::Run("power-hibernate", None)
        );
        // Leaving the leader after executing is what gives the keyboard back to the
        // buffers. Staying active would leave the system with no input.
        assert!(!l.active_one());
    }

    /// FAILS IF: the file manager stops being a submenu of `.` in the full
    /// tree, stops being `.` itself in the other two, or `f` comes back.
    ///
    /// The owner asked for the shape in as many words — *"não quero que
    /// `LEADER f` exista; quero que `LEADER .` seja um submenu; `LEADER . .`
    /// deve abrir o filemanager no rodapé; e `LEADER . w` deve abrir a versão
    /// janela"*. Before it, one thing was reachable by `.`, by `f f` and — the
    /// window — by `f w`.
    ///
    /// AND THEN FOR THE THREE SETS (2026-09-28): the submenu is still there,
    /// whole, in the full set (the leader pressed twice more); the leader
    /// alone runs the footer's door on `.`, and pressed again the same `.` is
    /// the window. In the full
    /// tree `.` ALONE still runs nothing — a prefix that also executed would
    /// run a command while somebody is still typing, which is the rule the
    /// keymap refuses in writing.
    #[test]
    fn the_manager_is_a_submenu_of_the_dot() {
        let (trees, _) = default_of().unwrap();
        let walk = |set: Set, seq: &[char]| {
            let mut l = Leader::default();
            l.activate();
            assert!(l.set(set), "the set could not be chosen at the root");
            let mut last = Effect::Close { other_case: None };
            for c in seq {
                last = l.key(&trees, Key::simple(Sym::Char(*c)));
            }
            last
        };
        assert_eq!(
            walk(Set::Full, &['.', '.']),
            Effect::Run("files-open", None)
        );
        assert_eq!(
            walk(Set::Full, &['.', 'w']),
            Effect::Run("files-window", None)
        );
        // `.` ALONE IS A GROUP in the full tree, so it runs nothing there.
        assert!(
            !matches!(walk(Set::Full, &['.']), Effect::Run(..)),
            "`.` executes in the full tree, so it cannot honestly be a prefix there"
        );
        // AND IS EACH DOOR in the other two.
        assert_eq!(walk(Set::Footer, &['.']), Effect::Run("files-open", None));
        assert_eq!(walk(Set::Window, &['.']), Effect::Run("files-window", None));
        // `f` LEADS NOWHERE, in any set — which is what "I do not want it to
        // exist" means: not a group with nothing in it, not a key that runs
        // the manager by another road.
        for set in Set::ALL {
            assert!(
                !matches!(walk(set, &['f']), Effect::Run(..)),
                "`f` runs something in the {set:?} set"
            );
            for c in ['f', 'w'] {
                assert!(
                    !matches!(walk(set, &['f', c]), Effect::Run(..)),
                    "`f {c}` still reaches a command in the {set:?} set"
                );
            }
        }
    }

    #[test]
    fn the_numbers_carry_the_workspace_in_the_argument() {
        // NINE KEYS, ONE COMMAND. The alternative would be `workspace-ir-1`…`-9`:
        // nine names in the locator saying the same thing and nine text
        // keys to translate, just to carry a number.
        let (tree, _) = default_of().unwrap();
        for n in 1..=9u8 {
            let mut l = Leader::default();
            l.activate();
            l.key(&tree, Key::simple(Sym::Tab));
            let d = char::from_digit(u32::from(n), 10).unwrap();
            match l.key(&tree, Key::simple(Sym::Char(d))) {
                Effect::Run(id, Some(arg)) => {
                    assert_eq!(id, "workspace-go");
                    assert_eq!(
                        arg.parse::<u8>().unwrap(),
                        n,
                        "the key and the argument diverged"
                    );
                }
                otherwise => panic!("`TAB {d}` did not lead to workspace {n}: {otherwise:?}"),
            }
        }
    }

    /// FAILS IF: `LEADER 1`…`9` stop going to their workspace, or `LEADER 0`
    /// stops opening the overview — the keys the modeline's numbers teach.
    #[test]
    fn a_digit_at_the_top_goes_to_its_workspace_and_zero_shows_them_all() {
        let (tree, _) = default_of().unwrap();
        for n in 1..=9u32 {
            let mut l = Leader::default();
            l.activate();
            let d = char::from_digit(n, 10).unwrap();
            match l.key(&tree, Key::simple(Sym::Char(d))) {
                Effect::Run("workspace-go", Some(arg)) => {
                    assert_eq!(arg, n.to_string(), "`LEADER {d}`");
                }
                otherwise => panic!("`LEADER {d}` did not go to workspace {n}: {otherwise:?}"),
            }
        }
        let mut l = Leader::default();
        l.activate();
        assert_eq!(
            l.key(&tree, Key::simple(Sym::Char('0'))),
            Effect::Run("workspace-all", None)
        );
    }

    /// FAILS IF: the keys the line under the modeline prints to leave an
    /// overview stop being the keys that do it — in ANY of the three sets:
    /// the line prints them after the leader alone, and `TAB` folds in none.
    #[test]
    fn the_keys_printed_to_leave_an_overview_are_bound() {
        let (trees, _) = default_of().unwrap();
        for set in Set::ALL {
            for (keys, wanted) in [
                (EVERY_WORKSPACE_KEYS, "workspace-all"),
                (OVERVIEW_KEYS, "workspace-overview"),
                ("TAB a", "workspace-all"),
            ] {
                let seq = parse_sequence(keys).unwrap();
                assert!(
                    matches!(trees.of(set).resolve(&seq), ronoctua_keymap::Resolution::Command(id, None) if id == wanted),
                    "`{keys}` is not `{wanted}` in the {set:?} set"
                );
            }
        }
    }

    /// FAILS IF: `LEADER /` stops being the development group with the
    /// resizer at `/ r`, the development group holds `d` again — the fast
    /// access's (2026-09-30) — or the glance leaves `g`, or the constant its
    /// sentences teach it by stops being the key that runs it.
    #[test]
    fn the_dev_group_is_slash_and_the_glance_is_g() {
        // THE DEFAULT SET, which is where a sequence starts; `/` folds in none.
        let (trees, _) = default_of().unwrap();
        let tree = &trees.footer;
        let runs = |keys: &str| match tree.resolve(&parse_sequence(keys).unwrap()) {
            ronoctua_keymap::Resolution::Command(id, None) => Some(id),
            _ => None,
        };
        assert_eq!(runs("/ r"), Some("dev-resizer"));
        assert_eq!(runs("g"), Some("world-glance"));
        assert_eq!(GLANCE_KEYS, "g");
        assert_eq!(runs(GLANCE_KEYS), Some("world-glance"));
        assert_eq!(runs("/"), None, "`/` runs something, so it is no group");
        let ronoctua_keymap::Resolution::Prefix(root) = tree.resolve(&[]) else {
            panic!("the root is not a list");
        };
        let dev = root
            .iter()
            .find(|e| e.key == "/")
            .expect("no `/` at the root");
        assert_eq!(dev.command, None, "`/` is a command and not a group");
        assert_eq!(dev.label, key::GROUP_DEV);
        // `d` IS NOT THE DEVELOPMENT GROUP'S ANY MORE, in any set: whatever
        // the fast access binds there, the resizer is not under it.
        for set in Set::ALL {
            let t = trees.of(set);
            assert!(
                !matches!(
                    t.resolve(&parse_sequence("d r").unwrap()),
                    ronoctua_keymap::Resolution::Command("dev-resizer", _)
                ),
                "`d r` still runs the resizer in the {set:?} set"
            );
            let ronoctua_keymap::Resolution::Prefix(root) = t.resolve(&[]) else {
                panic!("the root of the {set:?} set is not a list");
            };
            assert!(
                !root
                    .iter()
                    .any(|e| e.key == "d" && e.label == key::GROUP_DEV),
                "`d` is still the development group in the {set:?} set"
            );
        }
    }

    /// FAILS IF: the root list draws the nine digits as nine rows again, or
    /// the condensed row's click stops opening the workspace group.
    #[test]
    fn the_nine_digits_are_one_row_whose_click_opens_the_workspaces() {
        let (tree, _) = default_of().unwrap();
        let ronoctua_keymap::Resolution::Prefix(root) = tree.footer.resolve(&[]) else {
            panic!("the root is not a list");
        };
        let shown = condensed_root(root.clone());
        assert_eq!(root.len() - shown.len(), 8, "nine rows became one");
        let row = shown
            .iter()
            .find(|e| e.key == RANGE_ROW)
            .expect("no `1…9` row");
        assert_eq!(row.command.as_deref(), Some("workspace-go"));
        assert!(!shown.iter().any(|e| e.key == "3"));
        // `0` is another thing — every workspace at once — and keeps its row.
        assert!(shown.iter().any(|e| e.key == "0"));
        // THE CLICK PRESSES `sym`, and `sym` opens the group where each
        // workspace is a row of its own.
        let mut l = Leader::default();
        l.activate();
        match l.key(&tree, row.sym) {
            Effect::Continue { inputs } => {
                assert!(
                    inputs
                        .iter()
                        .any(|e| e.key == "3" && e.command.as_deref() == Some("workspace-go"))
                );
            }
            otherwise => panic!("the `1…9` row did not open the workspaces: {otherwise:?}"),
        }
    }

    #[test]
    fn the_zero_goes_to_the_last_workspace() {
        // The keyboard's convention: zero closes the row after nine. With
        // nine fixed workspaces in the compositor, the last is the ninth.
        let (tree, _) = default_of().unwrap();
        let mut l = Leader::default();
        l.activate();
        l.key(&tree, Key::simple(Sym::Tab));
        assert_eq!(
            l.key(&tree, Key::simple(Sym::Char('0'))),
            Effect::Run("workspace-go", Some("9"))
        );
    }

    #[test]
    fn moving_the_window_has_the_same_keys_as_going() {
        // THE REQUEST: whoever learned `TAB 3` gets `TAB m 3` for free. A
        // key that exists in one submenu and is missing from the other is an exception one only
        // discovers in use.
        let (tree, _) = default_of().unwrap();
        let ir = |seq: &[Key]| {
            let mut l = Leader::default();
            l.activate();
            let mut last = Effect::Close { other_case: None };
            for t in seq {
                last = l.key(&tree, *t);
            }
            last
        };
        let tab = Key::simple(Sym::Tab);
        let m = Key::simple(Sym::Char('m'));
        for c in ['j', 'k', '[', ']', '.', ',', '0', '1', '9'] {
            let key = Key::simple(Sym::Char(c));
            assert!(
                matches!(ir(&[tab, key]), Effect::Run(..)),
                "`TAB {c}` leads to no command at all"
            );
            assert!(
                matches!(ir(&[tab, m, key]), Effect::Run(..)),
                "`TAB m {c}` leads to no command at all"
            );
        }
        // And the arrows, in both.
        for sym in [Sym::Up, Sym::Down] {
            let key = Key::simple(sym);
            assert!(
                matches!(ir(&[tab, key]), Effect::Run(..)),
                "{sym:?} after TAB"
            );
            assert!(
                matches!(ir(&[tab, m, key]), Effect::Run(..)),
                "{sym:?} after TAB m"
            );
        }
    }

    #[test]
    fn the_prefix_is_dooms_notation() {
        let (tree, _) = default_of().unwrap();
        let mut l = Leader::default();
        l.activate();
        assert_eq!(l.prefix("SPC"), "SPC-");
        assert_eq!(l.raw_prefix("SPC"), "SPC");
        // `s` and not `b`, for the reason `esc_always_ends_it` gives: `b` is a
        // command now and a command ends the sequence.
        l.key(&tree, Key::simple(Sym::Char('s')));
        assert_eq!(l.prefix("SPC"), "SPC s-");
        // Without the ellipsis: in an error message it would lie, because
        // there the sequence has ended and there is no continuation.
        assert_eq!(l.raw_prefix("SPC"), "SPC s");
    }

    /// FAILS IF: the chord on screen stops saying which set the next key
    /// walks — as the leader pressed again, `SUPER SUPER-`, `SUPER SUPER SUPER
    /// s n-` — or the default set grows a word of its own.
    #[test]
    fn the_prefix_names_the_set() {
        let (trees, _) = default_of().unwrap();
        let mut l = Leader::default();
        l.activate();
        assert!(l.set(Set::Window));
        assert_eq!(l.prefix("SUPER"), "SUPER SUPER-");
        assert_eq!(l.raw_prefix("SUPER"), "SUPER SUPER");
        // THE LAST SWITCH WINS, while nothing has been typed.
        assert!(l.set(Set::Full));
        l.key(&trees, Key::simple(Sym::Char('s')));
        l.key(&trees, Key::simple(Sym::Char('n')));
        assert_eq!(l.prefix("SUPER"), "SUPER SUPER SUPER s n-");
        assert_eq!(l.raw_prefix("SUPER"), "SUPER SUPER SUPER s n");
        // AND THE NESTED LOOP'S LEADER, three keys long, is repeated whole.
        l.activate();
        assert!(l.set(Set::Window));
        assert_eq!(l.prefix("CTRL ALT SPC"), "CTRL ALT SPC CTRL ALT SPC-");
        // AND THE DEFAULT IS THE LEADER ALONE, from every opening.
        l.activate();
        assert_eq!(l.set_now(), Set::Footer);
        assert_eq!(l.prefix("SUPER"), "SUPER-");
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
        let (_, registry) = default_of().unwrap();
        for set in Set::ALL {
            let id = set.command();
            assert!(registry.search(id).is_some(), "`{id}` is not registered");
            assert!(BY_NAME_ONLY.contains(&id), "`{id}` has a sequence?");
        }
    }

    /// FAILS IF: the set can be switched anywhere but the root of an open
    /// leader — halfway through `s n` the keys already typed would change
    /// meaning under the hand that typed them.
    #[test]
    fn the_set_switches_only_at_the_root() {
        let (trees, _) = default_of().unwrap();
        let mut l = Leader::default();
        assert!(!l.set(Set::Window), "a closed leader took a set");
        assert_eq!(l.set_now(), Set::Footer);
        l.activate();
        assert!(l.set(Set::Window));
        assert_eq!(l.set_now(), Set::Window);
        assert!(matches!(
            l.key(&trees, Key::simple(Sym::Char('s'))),
            Effect::Continue { .. }
        ));
        assert!(!l.set(Set::Full), "the set switched with `s` typed");
        assert_eq!(l.set_now(), Set::Window);
        // AND IT IS WHAT THE NEXT KEY WALKS: `s n` in the window set is the
        // network's window.
        assert_eq!(
            l.key(&trees, Key::simple(Sym::Char('n'))),
            Effect::Run("network-window", None)
        );
        // ENDING THE SEQUENCE puts the default back.
        assert_eq!(l.set_now(), Set::Footer);
    }

    /// A binding for the rule's own tests — a CALL and not a tuple, so the
    /// made-up ids here are not read as keys of the real table by the checks
    /// that sweep this file for `("seq", "id"` rows.
    fn bind(seq: &str, command: &'static str) -> Binding {
        Binding {
            sequence: parse_sequence(seq).unwrap(),
            command,
            label: "x",
            argument: None,
        }
    }

    fn group(seq: &str) -> GroupNode {
        GroupNode {
            sequence: parse_sequence(seq).unwrap(),
            label: "x",
        }
    }

    /// FAILS IF: the rule folds a group it should not — one door, one
    /// binding answering for both, the same command twice — or keeps one it
    /// should fold, or a fold inside a fold survives its outer one.
    #[test]
    fn a_group_folds_when_it_has_two_different_doors() {
        let bindings = [
            bind("a a", "a-footer"),
            bind("a w", "a-window"),
            bind("a x", "a-other"),
            // `w`'s doubled key IS its `w`: one binding, not two doors.
            bind("w w", "w-both"),
            // AND UNDER `w` THE LETTER `w` IS NO DOOR: a level of the window
            // menu with two deeds that happen to sit on `v` and `w`.
            bind("w v v", "v-one"),
            bind("w v w", "v-other"),
            // ONE DOOR.
            bind("b b", "b-footer"),
            bind("b x", "b-other"),
            // TWO KEYS, ONE COMMAND.
            bind("c c", "c-same"),
            bind("c w", "c-same"),
            // A FOLD INSIDE A FOLD.
            bind("d d", "d-footer"),
            bind("d w", "d-window"),
            bind("d e e", "e-footer"),
            bind("d e w", "e-window"),
        ];
        let groups = [
            group("a"),
            group("w"),
            group("w v"),
            group("b"),
            group("c"),
            group("d"),
            group("d e"),
        ];
        let folded: Vec<String> = folds(&bindings, &groups)
            .iter()
            .map(|f| ronoctua_keymap::notation(&f.group))
            .collect();
        assert_eq!(folded, ["a", "d", "d e"]);

        let tree = |set| {
            let (b, g) = collapse(&bindings, &groups, set);
            Tree::build_it(&b, &g).expect("a folded tree collides")
        };
        let runs = |t: &Tree, keys: &str| t.resolve(&parse_sequence(keys).unwrap());
        let (footer, window, full) = (tree(Set::Footer), tree(Set::Window), tree(Set::Full));
        use ronoctua_keymap::Resolution::{Command, Nothing, Prefix};
        assert_eq!(runs(&footer, "a"), Command("a-footer", None));
        assert_eq!(runs(&window, "a"), Command("a-window", None));
        // EVERYTHING UNDER A FOLD LEAVES THE SET, and the fold inside it with it.
        assert_eq!(runs(&footer, "a x"), Nothing);
        assert_eq!(runs(&footer, "d"), Command("d-footer", None));
        assert_eq!(runs(&window, "d"), Command("d-window", None));
        assert_eq!(runs(&footer, "d e"), Nothing);
        // WHAT DOES NOT FOLD IS THE SAME IN ALL THREE.
        for t in [&footer, &window, &full] {
            assert_eq!(runs(t, "w w"), Command("w-both", None));
            assert_eq!(runs(t, "w v w"), Command("v-other", None));
            assert_eq!(runs(t, "b b"), Command("b-footer", None));
            assert_eq!(runs(t, "c w"), Command("c-same", None));
            assert!(matches!(runs(t, "b"), Prefix(_)));
        }
        // AND THE FULL SET IS THE TABLE AS WRITTEN.
        assert!(matches!(runs(&full, "a"), Prefix(_)));
        assert_eq!(runs(&full, "a x"), Command("a-other", None));
        assert_eq!(runs(&full, "d e e"), Command("e-footer", None));
    }

    /// FAILS IF: a group folds that did not, or stops folding — the rule's
    /// answer over the real tables, pinned. A group added tomorrow that keeps
    /// the two-door convention lands here on purpose, and whoever adds it
    /// reads why: `LEADER` alone then runs its footer door and `LEADER
    /// LEADER` its window.
    ///
    /// `w` is NOT here (its doubled key is its `w`), nor any level of the
    /// window menu under it (`w v` has `w v v` and `w v w`, two deeds and no
    /// doors), nor `RET`, `TAB`, `TAB m` or `s q` (one door each), nor `m`,
    /// `m i`, `s`, `d` or `t i` (no door of their own).
    #[test]
    fn the_collapsed_groups_are_these() {
        use ronoctua_keymap::Resolution;
        let (trees, _) = default_of().unwrap();
        // EVERY GROUP OF THE FULL TREE, walked from the root.
        let mut groups: Vec<Vec<Key>> = Vec::new();
        let mut edge: Vec<Vec<Key>> = vec![Vec::new()];
        while let Some(at) = edge.pop() {
            let Resolution::Prefix(entries) = trees.full.resolve(&at) else {
                continue;
            };
            for e in entries.iter().filter(|e| e.is_group) {
                let mut seq = at.clone();
                seq.push(e.sym);
                groups.push(seq.clone());
                edge.push(seq);
            }
        }
        let command = |set: Set, seq: &[Key]| match trees.of(set).resolve(seq) {
            Resolution::Command(id, None) => Some(id),
            _ => None,
        };
        let mut folded: Vec<(String, &str, &str)> = groups
            .iter()
            .filter_map(|g| {
                Some((
                    ronoctua_keymap::notation(g),
                    command(Set::Footer, g)?,
                    command(Set::Window, g)?,
                ))
            })
            .collect();
        folded.sort_unstable();
        let mut wanted: Vec<(String, &str, &str)> = [
            (".", "files-open", "files-window"),
            ("p", "player-open", "player-window"),
            ("t", "transfers-open", "transfers-window"),
            ("n", "notif-open", "notif-window"),
            ("i", "software-open", "software-window"),
            ("s h", "atuin-open", "atuin-window"),
            ("s n", "network-open", "network-window"),
            ("s s", "syncthing-open", "syncthing-window"),
            ("s d", "display-open", "display-window"),
            ("s a", "audio-open", "audio-window"),
            ("s c", "camera-settings-open", "camera-settings-window"),
            ("s i", "pointers-open", "pointers-window"),
            ("s p", "power-open", "power-window"),
            ("s b", "bluetooth-open", "bluetooth-window"),
            ("s l", "locale-open", "locale-window"),
            ("s g", "geo-open", "geo-window"),
            (
                "s u",
                crate::update_window::OPEN,
                crate::update_window::WATCH,
            ),
            ("s r", "footer-settings-open", "footer-settings-window"),
            ("m i", "icons-open", "icons-window"),
            ("=", "calc-open", "calc-window"),
            ("v", "video-metrics-open", "video-metrics-window"),
            ("s e", "security-open", "security-window"),
            ("s o", "containers-open", "containers-window"),
            ("s v", "belongings-open", "belongings-window"),
            ("d", "fast-access-open", "fast-access-window"),
            ("c", "clipboard-open", "clipboard-window"),
        ]
        .into_iter()
        .map(|(g, f, w)| (g.to_owned(), f, w))
        .collect();
        wanted.sort_unstable();
        assert_eq!(folded, wanted);
        // A GROUP FOLDS IN BOTH SETS OR IN NEITHER: a key that is a command
        // in one and a list in the other would be a third rule nobody wrote.
        for g in &groups {
            assert_eq!(
                command(Set::Footer, g).is_some(),
                command(Set::Window, g).is_some(),
                "`{}` folds in one set only",
                ronoctua_keymap::notation(g)
            );
        }
    }

    /// FAILS IF: the full set stops being the whole tree, or a folded key
    /// keeps its submenu in the default set.
    #[test]
    fn the_three_trees_are_the_three_sets() {
        use ronoctua_keymap::Resolution;
        let (trees, _) = default_of().unwrap();
        let resolve = |set: Set, keys: &str| trees.of(set).resolve(&parse_sequence(keys).unwrap());
        assert_eq!(
            resolve(Set::Footer, "s n"),
            Resolution::Command("network-open", None)
        );
        assert_eq!(
            resolve(Set::Window, "s n"),
            Resolution::Command("network-window", None)
        );
        assert!(matches!(resolve(Set::Full, "s n"), Resolution::Prefix(_)));
        // THE REST OF A FOLDED GROUP is two presses of the leader away, and
        // only there.
        assert_eq!(
            resolve(Set::Full, "s n a"),
            Resolution::Command("network-airplane", None)
        );
        assert_eq!(resolve(Set::Footer, "s n a"), Resolution::Nothing);
        // WHAT DOES NOT FOLD IS EVERYWHERE — the window's own letter included.
        for set in Set::ALL {
            assert_eq!(
                resolve(set, "w f"),
                Resolution::Command("window-screen-full", None)
            );
            assert_eq!(
                resolve(set, "RET RET"),
                Resolution::Command("buffer-terminal-external", None)
            );
            assert!(matches!(resolve(set, "s"), Resolution::Prefix(_)));
        }
        // THE FULL SET IS STRICTLY BIGGER: it lost nothing to the folding.
        assert!(trees.full.amount() > trees.footer.amount());
        assert!(trees.full.amount() > trees.window.amount());
    }

    /// FAILS IF: a sentence would teach a key that does something else when
    /// typed from where every sequence starts.
    #[test]
    fn the_taught_keys_start_in_the_default_set() {
        let (trees, _) = default_of().unwrap();
        let taught = |id: &str| taught_keys_with(&trees, id, None, "SUPER");
        // THE FOOTER'S OWN SEQUENCE, when it has one.
        assert_eq!(taught("transfers-open").as_deref(), Some("t"));
        assert_eq!(taught("notif-open").as_deref(), Some("n"));
        assert_eq!(taught("window-screen-full").as_deref(), Some("w f"));
        assert_eq!(taught("apps-open").as_deref(), Some(APPS_KEYS));
        assert_eq!(taught("brain-open").as_deref(), Some(BRAIN_KEYS));
        // ELSE THE WINDOW SET'S, after the leader pressed again.
        assert_eq!(taught("files-window").as_deref(), Some("SUPER ."));
        // ELSE THE FULL TREE'S, after the leader pressed twice more.
        assert_eq!(
            taught("bluetooth-accept"),
            Some(format!("SUPER SUPER {PAIRING_YES_KEYS}"))
        );
        assert_eq!(
            taught("software-trials"),
            Some(format!("SUPER SUPER {SOFTWARE_TRIALS_KEYS}"))
        );
        // AND THE ARGUMENT IS PART OF THE COMMAND.
        assert_eq!(
            taught_keys_with(&trees, "workspace-go", Some("3"), "SUPER").as_deref(),
            Some("3")
        );
        assert_eq!(taught("mx-does-not-exist"), None);
        // AND THE PROCESS'S OWN LEADER IS WHAT `taught_keys` REPEATS.
        assert_eq!(
            taught_keys(&trees, "files-window", None),
            Some(format!("{} .", crate::leader()))
        );
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
            assert!(!t.released(sym), "{sym:?}: a chord with the hand");
        }
        // THE POINTER BEFORE THE PRESS takes nothing from the tap after it.
        let mut t = Tap::default();
        t.pointer_acted();
        t.pressed(Keysym::Control_L);
        assert!(t.released(Keysym::Control_L));
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
    /// press counts, `Alt` or `AltGr` cycles the bars, a `Ctrl` under a held
    /// `Alt` counts, or `Control_R` held for the paging arrows cycles the bars
    /// when it is let go.
    #[test]
    fn only_a_clean_tap_of_ctrl_cycles_the_bars() {
        use smithay_client_toolkit::seat::keyboard::Keysym;
        let tapped = |keys: &[(bool, Keysym)]| {
            let mut t = Tap::default();
            let mut last = false;
            for (down, sym) in keys {
                if *down {
                    t.pressed(*sym);
                } else {
                    last = t.released(*sym);
                }
            }
            last
        };
        // A CLEAN TAP, either hand.
        assert!(tapped(&[
            (true, Keysym::Control_L),
            (false, Keysym::Control_L)
        ]));
        // THE RIGHT CTRL TAPPED ALONE is a tap too...
        assert!(tapped(&[
            (true, Keysym::Control_R),
            (false, Keysym::Control_R)
        ]));
        // ...and HELD WITH AN ARROW it is the paging chord, and no tap.
        assert!(!tapped(&[
            (true, Keysym::Control_R),
            (true, Keysym::Right),
            (false, Keysym::Right),
            (false, Keysym::Control_R),
        ]));
        // `ALT` CHOOSES NOTHING ANY MORE, either hand (the owner, 2026-09-30).
        for alt in [Keysym::Alt_L, Keysym::Alt_R] {
            assert!(!tapped(&[(true, alt), (false, alt)]), "{alt:?}");
        }
        // A KEY IN BETWEEN is a chord.
        assert!(!tapped(&[
            (true, Keysym::Control_L),
            (true, Keysym::n),
            (false, Keysym::Control_L),
        ]));
        // A RELEASE WITH NO PRESS: the leader's own `Ctrl` and `Alt` in the
        // nested loop, pressed before the footer had the keyboard.
        assert!(!tapped(&[
            (false, Keysym::Control_L),
            (false, Keysym::Alt_L)
        ]));
        // TWO MODIFIERS AT ONCE is a chord, whichever came first.
        assert!(!tapped(&[
            (true, Keysym::Control_L),
            (true, Keysym::Alt_L),
            (false, Keysym::Alt_L),
            (false, Keysym::Control_L),
        ]));
        assert!(!tapped(&[
            (true, Keysym::Alt_L),
            (true, Keysym::Control_L),
            (false, Keysym::Control_L),
        ]));
        // `AltGr` IS NOTHING HERE.
        assert!(!tapped(&[
            (true, Keysym::ISO_Level3_Shift),
            (false, Keysym::ISO_Level3_Shift),
        ]));
        // A REPEAT OF THE SAME MODIFIER is still one tap.
        assert!(tapped(&[
            (true, Keysym::Control_L),
            (true, Keysym::Control_L),
            (false, Keysym::Control_L),
        ]));
        // AND A TAP AFTER A TAP: each release answers its own.
        assert!(tapped(&[
            (true, Keysym::Alt_L),
            (false, Keysym::Alt_L),
            (true, Keysym::Control_L),
            (false, Keysym::Control_L),
        ]));
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

    #[test]
    fn every_binding_points_at_a_command_that_exists() {
        // The bug section 13 exists to prevent: a shortcut pointing at a
        // non-existent command. `default_of()` refuses to build in that case, so
        // this test is the proof that the refusal is wired up.
        let (_, registry) = default_of().unwrap();
        for id in [
            "buffer-next",
            "buffer-close",
            "buffer-terminal",
            "icons-open",
            "session-quit",
            "mx-open",
        ] {
            assert!(registry.search(id).is_some(), "the command {id} is missing");
        }
    }

    #[test]
    fn every_keymap_command_has_an_action_in_the_shell() {
        // THE DEFECT THIS PREVENTS: five commands appeared in which-key,
        // were "executed", and fell into a log `warn`. On screen: nothing — and
        // indistinguishable from an unregistered key.
        //
        // The list of ids with an action is in `surface::run_command`. It is
        // repeated here on purpose: a `match` is not enumerable at
        // runtime, and the alternative (a table consulted by both) would be
        // one more indirection for the same effect. What matters is that
        // adding a binding with no action BREAKS something.
        const WITH_ACTION: &[&str] = &[
            "workspace-all",
            "session-quit",
            "notif-open",
            "notif-window",
            "notif-read-all",
            "notif-dismiss",
            // The nine that reach the manager's focused row, through the same
            // `View::key` the keyboard uses — see `Footer::manager_press`.
            "notif-read",
            "notif-forget",
            "notif-act",
            "notif-adjust",
            "notif-up",
            "notif-down",
            "notif-pane-next",
            "notif-pane-previous",
            "notif-knob-next",
            "notif-see",
            "notif-filters",
            "notif-new",
            "notif-drop",
            "notif-share",
            "notif-share-answer",
            "notif-draft-type",
            "notif-draft-add",
            "notif-draft-drop",
            "notif-draft-save",
            "notif-knob-previous",
            "notif-restore",
            "display-open",
            "display-window",
            "display-up",
            "display-down",
            "display-less",
            "display-more",
            "display-enter",
            "display-automatic",
            "display-forget",
            "display-arrange",
            // SOUND: the two doors, the view's seven gestures, and the
            // keyboard's own top row. The last seven have no leader at all —
            // the compositor binds them bare — but they are commands like
            // everything else, so this list is where they are accounted for.
            "audio-open",
            "audio-window",
            "audio-up",
            "audio-down",
            "audio-enter",
            "audio-less",
            "audio-more",
            "audio-mute",
            "audio-default",
            "pointers-open",
            "pointers-window",
            "camera-open",
            "onscreen-keyboard",
            "gestures",
            "gestures-calibrate",
            "dev-resizer",
            "camera-settings-open",
            "camera-settings-window",
            "camera-settings-up",
            "camera-settings-down",
            "camera-settings-less",
            "camera-settings-more",
            "camera-settings-enter",
            "camera-settings-remove",
            "camera-photo",
            "camera-record",
            "camera-effect-next",
            "camera-effect-previous",
            "camera-background-next",
            "camera-param-less",
            "camera-param-more",
            "camera-camera-next",
            "camera-mirror",
            "camera-scene-next",
            "camera-scene-previous",
            "camera-script",
            "camera-next-cue",
            "camera-virtual",
            "camera-stream",
            "camera-scene-add",
            "camera-scene-rename",
            "camera-script-add",
            "camera-stream-url",
            "camera-stream-key",
            "player-open",
            "player-window",
            "player-column",
            "player-play-file",
            "player-enqueue",
            "player-together",
            "player-machines",
            "player-pair",
            "player-toggle",
            "player-next",
            "player-previous",
            "player-stop",
            "player-shuffle",
            "player-repeat",
            "player-up",
            "player-down",
            "player-less",
            "player-more",
            "player-enter",
            "player-remove",
            "player-browse",
            "player-add",
            "player-only",
            "player-maximize",
            "player-play-now",
            "player-clear",
            "player-quieter",
            "player-louder",
            "player-pip",
            "player-mute",
            "player-swap",
            "player-tracks",
            "player-fold",
            "player-earlier",
            "player-later",
            "player-also",
            "player-rename",
            "player-save-set",
            "player-save-queue",
            "player-quit",
            "player-subtitles",
            "player-lane-shown",
            "player-ambience",
            "player-ambience-silence",
            "player-timeline",
            "player-layout",
            "player-metadata",
            "player-suggestions",
            "player-find-subtitles",
            "player-find-lyrics",
            "player-internet",
            "player-every-screen",
            "player-radio",
            "player-tv",
            "player-channels",
            "player-search",
            "player-favourite",
            "player-nearer",
            "player-further",
            "player-world",
            "pointers-up",
            "pointers-down",
            "pointers-enter",
            "pointers-less",
            "pointers-more",
            "pointers-forget",
            "volume-up",
            "volume-down",
            "volume-mute",
            "mic-mute",
            "media-play",
            "media-next",
            "media-previous",
            "media-stop",
            "brightness-up",
            "brightness-down",
            "power-open",
            "power-window",
            "power-profile-performance",
            "power-profile-balanced",
            "power-profile-saver",
            "power-keep-awake",
            "power-keep-lid",
            "power-thresholds",
            "power-suspend",
            "power-hibernate",
            "power-reboot",
            "power-off",
            "power-enter",
            "power-less",
            "power-more",
            "power-up",
            "power-down",
            "world-glance",
            "locale-open",
            "locale-window",
            "geo-open",
            "geo-window",
            "geo-up",
            "geo-down",
            "geo-enter",
            "geo-find",
            "geo-remove",
            "locale-less",
            "locale-more",
            "locale-zone",
            "locale-place",
            "locale-where",
            "locale-enter",
            "locale-up",
            "locale-down",
            "bluetooth-more",
            "bluetooth-settings",
            "bluetooth-less",
            "bluetooth-open",
            "bluetooth-window",
            "bluetooth-scan",
            "bluetooth-power",
            "bluetooth-discoverable",
            "bluetooth-reconnect",
            "bluetooth-disconnect-all",
            "bluetooth-accept",
            "bluetooth-refuse",
            "bluetooth-device",
            "bluetooth-connect",
            "bluetooth-pair",
            "bluetooth-forget",
            "bluetooth-trust",
            "bluetooth-block",
            "bluetooth-rename",
            "bluetooth-unnamed",
            "bluetooth-up",
            "bluetooth-down",
            "keyboard-restore",
            "help",
            "mx-open",
            // THE LEADER'S THREE SETS: no sequence reaches them (the leader
            // is not a key of the tree), and the hint's click and `LEADER :`
            // run them through `run_with`.
            "leader-set-window",
            "leader-set-full",
            "apps-open",
            "buffer-switch",
            "files-open",
            "files-window",
            "buffer-terminal",
            "buffer-terminal-external",
            // THE CAPTURE IS OURS. The compositor takes none, so the shell
            // runs `grim` and builds the name — see `capture_the_screen`.
            "screen-capture",
            "monitor-btop",
            "rescue-open",
            "monitor-bottom",
            "keyboard-open",
            "memory-open",
            // THE BRAIN. `brain-open` is the screen; the other two answer the
            // countdown on the footer's own line, with no window open — which
            // is why they are commands of the registry and not keys of a view.
            "brain-open",
            "brain-abort",
            "brain-proceed",
            "buffer-close",
            "buffer-next",
            // THE TRANSFER QUEUE. Only `transfers-open` is a command of the
            // registry; everything that acts on a row is a key of the view,
            // like the file manager's.
            "transfers-open",
            "transfers-window",
            "format-open",
            "transfers-add",
            "transfers-video",
            "transfers-audio",
            "transfers-subtitles",
            "transfers-add-as-me",
            "transfers-video-as-me",
            "transfers-audio-as-me",
            "transfers-subtitles-as-me",
            // THE CLIPBOARD. `clipboard-open` has a key in the tree; the other
            // six have keys INSIDE the view, and that is why they go through
            // `reachable` — all seven need an action, otherwise they would appear in the
            // list and do nothing.
            "apps-pin",
            "apps-details",
            "apps-remove",
            "software-open",
            "software-window",
            "software-trials",
            "software-maintenance",
            "software-android",
            "software-web-new",
            "software-web-address",
            "software-web-name",
            "software-icon-path",
            "software-file-path",
            "software-launch-latest",
            "software-down",
            "software-up",
            "software-enter",
            "software-next-source",
            "software-previous-source",
            "software-less",
            "software-more",
            "software-install",
            "software-try-it",
            "software-launch",
            "software-update",
            "software-edit",
            "software-remove",
            "software-stop",
            "software-paste",
            "clipboard-open",
            "clipboard-copy-of-new",
            "clipboard-remove",
            "clipboard-pin",
            "clipboard-qrcode",
            "clipboard-clear",
            "clipboard-sync",
            "clipboard-down",
            "clipboard-up",
            "clipboard-more",
            "clipboard-less",
            // NETWORK and SYNCTHING — the seventh view and the service the distro
            // installs. `network-connect` and `network-forget` act on the focused
            // row and that is why they only have keys INSIDE the view; the others
            // have keys in the tree.
            "network-open",
            "network-airplane",
            "theme-open",
            "world-glance",
            "locale-open",
            "locale-window",
            "locale-less",
            "locale-more",
            "locale-zone",
            "locale-place",
            "locale-where",
            "theme-apply",
            "network-scan",
            "network-connect",
            "network-disconnect",
            "network-forget",
            "network-window",
            "network-up",
            "network-down",
            "network-lan",
            "network-import",
            "network-section-next",
            "atuin-open",
            "atuin-window",
            "atuin-sync",
            "atuin-down",
            "atuin-up",
            "atuin-enter",
            "atuin-more",
            "atuin-less",
            "atuin-back",
            "network-tor-open",
            "network-tor-more",
            "network-tor-less",
            "network-tor-back",
            "network-section-previous",
            "network-wifi-toggle",
            "network-link",
            "syncthing-open",
            "syncthing-window",
            "syncthing-web",
            "syncthing-state",
            "syncthing-pause-all",
            "syncthing-resume-all",
            "syncthing-up",
            "syncthing-down",
            "syncthing-less",
            "syncthing-more",
            "syncthing-enter",
            "syncthing-pause",
            "syncthing-rescan",
            "syncthing-add",
            "syncthing-forget",
            "syncthing-add-device",
            "syncthing-add-folder",
            "window-menu",
            "drawer-open",
            "trail-open",
            "selection-open",
            // THE CHROME KIOSK — every one an arm of `run_with`, which hands
            // it to `kiosk::Kiosks::command` or to the kiosk's view.
            "kiosk-open",
            "kiosk-back",
            "kiosk-forward",
            "kiosk-reload",
            "kiosk-tab",
            "kiosk-tab-next",
            "kiosk-tab-previous",
            "kiosk-tab-close",
            "kiosk-tab-left",
            "kiosk-tab-right",
            "kiosk-tab-move",
            "kiosk-new-tab",
            "kiosk-address",
            "kiosk-zoom",
            "kiosk-zoom-in",
            "kiosk-zoom-out",
            "kiosk-zoom-reset",
            "kiosk-chrome-ui",
            "kiosk-address-line",
            "kiosk-more",
            "kiosk-tabs",
            "kiosk-extensions",
            "kiosk-extension",
            "kiosk-extension-options",
            "kiosk-extension-details",
            "kiosk-release",
            "modeline-adapt-kiosk",
            "kiosk-list-down",
            "kiosk-list-up",
            "kiosk-list-enter",
            "kiosk-tab-earlier",
            "kiosk-tab-later",
            "footer-hide-push",
            "footer-hide-cover",
            // THE STRIP'S THREE ABOUT A NOTICE, and the actions a module
            // offers: each an arm of `run_with`, which asks the brain.
            "brain-dismiss",
            "brain-snooze",
            "brain-act",
            // ── end of brain-ui ──
            "leader-set-default",
            "footer-bars-cycle",
            "leader-close",
            "minibuffer-fold-open",
            "minibuffer-fold-close",
            "alert-dismiss",
            "footer-settings-open",
            "footer-settings-window",
            "footer-settings-up",
            "footer-settings-down",
            "footer-settings-enter",
            "footer-hours",
            "icons-open",
            "icons-window",
            "icons-nerd-fonts",
            "icons-lucide",
            "icons-unicode",
            "icons-colour",
            "calc-open",
            "calc-window",
            "player-rewind",
            "player-forward",
            "player-picture-fold",
            "player-save-queue-again",
            "player-countries-order",
            "player-lyrics-translate",
            "player-website",
            "player-listing-earlier",
            "player-listing-later",
            "player-listing-first",
            "player-listing-last",
            "player-listing-full",
            "player-listing-layout",
            "player-listing-source",
            "player-listing-letter",
            "player-listing-order",
            "video-metrics-open",
            "video-metrics-window",
            "video-metrics-scan",
            "video-metrics-up",
            "video-metrics-down",
            "video-metrics-less",
            "video-metrics-more",
            "video-metrics-enter",
            "video-metrics-page-back",
            "video-metrics-page-on",
            "video-metrics-layout",
            "video-metrics-search",
            "video-metrics-player",
            "video-metrics-kiosk",
            "video-metrics-download",
            "video-metrics-hide",
            "video-metrics-delete",
            "video-metrics-add-channel",
            "video-metrics-channel-score",
            "video-metrics-points",
            "video-metrics-days",
            "video-metrics-band",
            "video-metrics-words",
            "video-metrics-second-tags",
            "video-metrics-weight",
            "security-open",
            "security-window",
            "security-relax",
            "security-shields",
            "security-firewall",
            "security-check-password",
            "security-check-email",
            "security-check-file",
            "security-down",
            "security-up",
            "security-enter",
            "security-left",
            "security-right",
            "security-rule-add",
            "security-rule-delete",
            "security-enrol",
            "security-password",
            "security-rule",
            "security-vt-key",
            "security-vt-forget",
            "security-home",
            "containers-open",
            "containers-window",
            "containers-down",
            "containers-up",
            "containers-enter",
            "containers-left",
            "containers-right",
            "containers-next-kind",
            "containers-previous-kind",
            "containers-next-tab",
            "containers-previous-tab",
            "containers-start-stop",
            "containers-restart",
            "containers-remove",
            "containers-logs",
            "containers-shell",
            "containers-inspect",
            "containers-pull",
            "containers-build",
            "containers-login",
            "containers-login-password",
            "containers-prune",
            "containers-reveal",
            // THE BELONGINGS (`LEADER s v`, `crate::belongings`).
            "belongings-open",
            "belongings-window",
            "belongings-sync",
            "belongings-up",
            "belongings-down",
            "belongings-less",
            "belongings-more",
            "belongings-enter",
            "belongings-keep-local",
            "belongings-take-remote",
            "belongings-keep-both",
            "belongings-apply",
            "belongings-abandon",
            "belongings-all-local",
            "belongings-all-remote",
            "belongings-repository",
            "belongings-add",
            "belongings-auth",
            "belongings-keygen",
            "belongings-copy-key",
            "belongings-token",
            "belongings-user",
            "belongings-passphrase",
            "belongings-period",
            "fast-access-open",
            "fast-access-window",
            "fast-access-column",
            "fast-access-float",
            "fast-access-launch",
            "fast-access-open-path",
            "fast-access-reveal",
            "fast-access-man",
            // THE RANKING and its keys, arms of `run_with`.
            "player-ranking",
            "player-ranking-more",
            "player-ranking-earlier",
            "player-ranking-later",
            "player-ranking-first",
            "player-ranking-last",
            "player-ranking-layout",
            "display-rise",
            "display-sink",
            "clipboard-window",
        ];
        let (tree, registry) = default_of().unwrap();
        for id in super::reachable(&tree.full) {
            // The ones that are COMPOSITOR deeds go through the table,
            // consulted by id — and the generator proves, against the
            // compositor's own source, that every one of them exists.
            if crate::actions::deed_of(id).is_some() {
                continue;
            }
            // The compositor's DEEDS are dispatched by prefix: `run_with`
            // looks the id up in the generated table and sends the text.
            // Some of them DO have keys, because a chord names one.
            if id.starts_with("wm-") {
                continue;
            }
            // The two workspace ones that take a number have no null form:
            // the chord carries the number in the bind itself.
            if matches!(id, "workspace-go" | "workspace-move") {
                continue;
            }
            // THE UPDATE'S THREE are dispatched by the one question the
            // footer asks its window (`update_window::order_of`), the same
            // function `run_with` calls — so being answered there IS being
            // handled.
            assert!(
                WITH_ACTION.contains(&id) || crate::update_window::order_of(id).is_some(),
                "`{id}` has a key and `surface::run_command` does not handle it: \
                 it would show up in the list and do nothing"
            );
        }
        // And the inverse: an id in the list above that left the registry would become a
        // dead branch in the `match`, waiting for a key that does not exist.
        for id in WITH_ACTION
            .iter()
            .chain(crate::update_window::COMMANDS.iter().map(|c| &c.id))
        {
            assert!(
                registry.search(id).is_some(),
                "`{id}` is in the handled list and no longer exists"
            );
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
}
