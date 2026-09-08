//! The leader, the default keymap and the control channel — section 7.4.
//!
//! # How the leader gets here
//!
//! At rest the footer **does not capture the keyboard** (`keyboard-interactivity:
//! none`). That is not a detail: if it did, no buffer would receive a key.
//! Therefore the footer itself **cannot see** the leader's key.
//!
//! What sees it is the compositor. `nix/modules/niri.nix` binds `F13` — the key
//! keyd emits when Super is TAPPED — to `spawn ronoctua-shell leader`, and
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
    },
}

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
/// learns it from `ronoctua_niri::Window::pid` on the focused window — the same
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
pub type States = (String, Vec<(String, char)>);

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
    let now = lines.next()?.trim().to_owned();
    if now.is_empty() {
        return None;
    }
    let states = lines
        .filter_map(|l| {
            let (id, letter) = l.trim().split_once(' ')?;
            Some((id.to_owned(), letter.chars().next()?))
        })
        .collect();
    Some((now, states))
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
                if BufReader::new(stream).read_line(&mut line).is_err() {
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
                    // `state <pid> <id>`. A pid that does not parse is dropped
                    // with the rest: there is nothing sensible to do with a
                    // report about a window nobody can name.
                    (Some("state"), Some(pid)) => match (pid.parse(), fields.next()) {
                        (Ok(pid), Some(id)) => Order::State {
                            pid,
                            id: id.to_owned(),
                        },
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

/// The leader's state: nothing, or a sequence in progress.
#[derive(Debug, Default)]
pub struct Leader {
    /// The keys already typed after the leader. Empty = a freshly activated leader.
    sequence: Vec<ronoctua_keymap::Key>,
    active_one: bool,
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
    #[must_use]
    pub fn prefix(&self, trigger: &str) -> String {
        if self.sequence.is_empty() {
            format!("{trigger}-")
        } else {
            format!("{trigger} {}-", ronoctua_keymap::notation(&self.sequence))
        }
    }

    /// The typed sequence, WITHOUT the continuation ellipsis.
    ///
    /// The `prefix` ends in `…` because it says "keys are missing". In an
    /// error message that would be a lie: there the sequence ENDED, and with no
    /// destination. Two readings, two functions.
    #[must_use]
    pub fn raw_prefix(&self, trigger: &str) -> String {
        if self.sequence.is_empty() {
            trigger.to_owned()
        } else {
            format!("{trigger} {}", ronoctua_keymap::notation(&self.sequence))
        }
    }

    /// Activates the leader from scratch.
    pub fn activate(&mut self) {
        self.active_one = true;
        self.sequence.clear();
    }

    /// Switches it off.
    pub fn finish(&mut self) {
        self.active_one = false;
        self.sequence.clear();
    }

    /// Processes a key while the leader is active.
    ///
    /// `ESC` **always** ends it, even if there is a binding for it. Section
    /// 7.4 puts that as a rule: there has to be an exit that works
    /// without the user knowing what state they are in.
    pub fn key(&mut self, tree: &Tree, t: ronoctua_keymap::Key) -> Effect {
        if t.sym == ronoctua_keymap::Sym::Esc {
            self.finish();
            return Effect::Close { other_case: None };
        }

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

/// How `brain-abort` is typed, after the leader.
///
/// A constant and not a literal in two places: the BAND DRAWS IT, because a
/// control whose key is only in the manual is a control nobody presses — and a
/// sequence that lived in the table and again in the drawing would be two
/// answers to "which key stops it", with the wrong one on screen.
///
/// `b` is the BRAIN's own prefix — the machine's agenda, at the first level by
/// the owner's decision — and `a` is the letter the brain's own buffer already
/// uses for the same verb under its local leader (`m a`). One letter, two
/// prefixes, one meaning.
pub const BRAIN_ABORT_KEYS: &str = "b a";

/// The same, for `brain-proceed`. `p`, as in the buffer's `m p`.
pub const BRAIN_PROCEED_KEYS: &str = "b p";

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
/// A constant for the reason the two above are: the window's door SAYS it
/// ("the action runs from the footer: …") and a sentence that carried the
/// keys as text went stale the day the key moved from `n h` to `n n`.
pub const NOTIF_OPEN_KEYS: &str = "n n";

/// The two sequences that take and drop the sleep locks, in the footer.
///
/// The power window's message says where the lock lives, and it used to say it
/// as TEXT in both `.ftl` files: "LEADER s p k, or LEADER s p l". Two things
/// were wrong with that at once — the sequences would go stale the day either
/// key moved, exactly as `n h` did; and the word "LEADER" is nobody's key.
/// `nix/modules/niri.nix` sets `RONOCTUA_LEADER` to `SUPER` and `nix/dev.nix`
/// to `CTRL ALT SPC`, and every other surface in this system renders
/// `crate::leader()` — the one place that knows.
pub const KEEP_AWAKE_KEYS: &str = "s p k";

/// The two sequences that answer a pairing question from anywhere.
///
/// The card that carries the question lives twenty seconds and the question
/// lives fifty-five, so the body names these — and names them as ARGUMENTS,
/// resolved through `crate::leader()`, for the reason [`NOTIF_OPEN_KEYS`]
/// gives.
pub const PAIRING_YES_KEYS: &str = "s b y";

/// The refusal's half of [`PAIRING_YES_KEYS`].
pub const PAIRING_NO_KEYS: &str = "s b n";

/// The lid's half of [`KEEP_AWAKE_KEYS`].
pub const KEEP_LID_KEYS: &str = "s p l";

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
pub fn default_of() -> Result<(Tree, Registry)> {
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
        // A MANAGER AND NOT A PICKER, and `f f` IS this manager: the key
        // used to open the editor's file picker — hand a path over and stop
        // there — and it was given to the manager, which lists everything a
        // directory holds (permissions, owner, group, size, date, and a glyph
        // for what each thing is) and acts on it. This comment used to say
        // `f f` was still the picker, eight hundred lines above the binding
        // that says otherwise.
        Command {
            id: "files-open",
            label: key::CMD_FILES_OPEN,
        },
        // The SAME manager, in a window of its own. The footer's region has a
        // twelve-line ceiling (`space.expansion`, invariant 14), which is right
        // for a footer and not for the job.
        Command {
            id: "files-window",
            label: key::CMD_FILES_WINDOW,
        },
        Command {
            id: "buffer-close",
            label: key::CMD_BUFFER_CLOSE,
        },
        Command {
            id: "glyphs-nerd-fonts",
            label: key::CMD_GLYPHS_NERD_FONTS,
        },
        Command {
            id: "unicode-symbols",
            label: key::CMD_UNICODE_SYMBOLS,
        },
        Command {
            id: "lucide-icons",
            label: key::CMD_LUCIDE_ICONS,
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
        // `LEADER b b` — what the machine does BY ITSELF. The panel's own
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
        // does not. The buffer has its own `m a` and `m p` for the same two
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
        // `f f` are — a prefix cannot also be a command, and the tree refuses
        // it. (`r r` was the network's, and the same change that made this
        // group killed the first-level `r`.)
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
        // `LEADER s u` — THE WHOLE UPDATE, BY HAND, WITH NO BRAIN IN IT.
        //
        // The brain decides WHEN the machine updates itself; this is the
        // person deciding it is now. The two are not two sequences: this
        // starts the same unit the brain starts, and neither of them holds a
        // list of steps — `assets/updates.nix` does, and everything else is
        // generated from it. That is why there is no way to add a step to one
        // and not the other: there is only one place a step can be added.
        //
        // It works with the daemon down, on purpose. A scheduler that is not
        // running must not be able to stop a person from updating their own
        // machine — see `update_window::begin`.
        Command {
            id: "system-update",
            label: key::CMD_SYSTEM_UPDATE,
        },
        // AND THE WINDOW THAT WATCHES ONE HAPPEN — the door the modeline's
        // animated mark opens.
        //
        // It is a command of the registry and not a key of a view for the same
        // reason `brain-abort` is: the mark appears on the footer's own line,
        // with no window open, and the mouse may not reach a door the keyboard
        // does not. It also opens with NOTHING running, which is when somebody
        // wants to read what the last run did.
        Command {
            id: "update-watch",
            label: key::CMD_UPDATE_WATCH,
        },
        Command {
            id: "monitor-bottom",
            label: key::CMD_MONITOR_BOTTOM,
        },
        // ---------------------------------------------------------------
        // WINDOW AND WORKSPACE — what used to be niri binds.
        //
        // Section 7.4 wants ONE keymap. While the compositor had its
        // shortcuts and the shell had its own, there were two tables to
        // maintain, two notations and two places where a key could collide
        // — and which-key knew only half. Now niri has ONE bind (the
        // leader) and all the rest lives here, reaching the compositor through
        // an IPC action (invariant 3).
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
        // ---------------------------------------------------------------
        // THE ONES THAT EXISTED ONLY AS A KEY OR ONLY AS A SOCKET ORDER.
        //
        // `window-float` was a chord with hand-written KDL; the `notif-*` were
        // socket orders with no id at all. All three were features with no
        // name — the opposite of what section 13 asks for, and invisible in `M-x`.
        // ---------------------------------------------------------------
        Command {
            id: "window-float",
            label: key::CMD_WINDOW_FLOAT,
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
        // twelve-line ceiling (invariant 14), which is right for a footer and
        // not for reading a fortnight of history.
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
        // `OF_CLIPBOARD_VIEW` — otherwise the orphan-command check flags them, and
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
        Command {
            id: "clipboard-clipcascade-configure",
            label: key::CMD_CLIPBOARD_CLIPCASCADE_CONFIGURE,
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
        // THE MANUAL. It exists because the chords left the tap list:
        // what one holds has to appear somewhere, and the tap
        // list was not that place — see `manual.rs`.
        Command {
            id: "help-shortcuts",
            label: key::CMD_HELP_SHORTCUTS,
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
        // the list, and the one the list's `RET` runs. Moving the cursor
        // PREVIEWS the theme in the footer; `theme-apply` is what writes it,
        // and the write is what reaches the other doors (see `theme.rs`).
        // No `theme-next`: `check-keymap` refuses a command with no key, and a
        // key outside the list would cycle a palette nobody can see the name of.
        // ---------------------------------------------------------------
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
        // SYNCTHING. It ships with the distro (nix/modules/
        // syncthing.nix), and a service the distro installs needs a
        // path through the leader — otherwise the only way to look at it is typing
        // `systemctl` in a terminal, which is the opposite of section 13.
        Command {
            id: "syncthing-open",
            label: key::CMD_SYNCTHING_OPEN,
        },
        Command {
            id: "syncthing-state",
            label: key::CMD_SYNCTHING_STATE,
        },
        Command {
            id: "syncthing-pause",
            label: key::CMD_SYNCTHING_PAUSE,
        },
        Command {
            id: "syncthing-resume",
            label: key::CMD_SYNCTHING_RESUME,
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
        // So that there are not two truths again, niri's chord binds
        // are GENERATED from this same command table — see `actions::chords`
        // and `nix/niri-binds.nix`.
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
        // AND THE BUFFER'S OWN, under the same letter, by the owner's
        // decision: a buffer IS a window of the compositor (invariant 4), and
        // "buffer" and "window" were two groups for one thing. `w w` switches
        // (the doubled letter, the convention of every prefix's own view),
        // `n` next, `d` closes — Doom's `SPC w d` —, `s` chooses the state
        // (grafted at runtime, see `Footer::state_key`), `o` opens a terminal
        // and `O` one outside the compositor's tiling. The `b` they used to
        // hang off is the brain's now.
        ("w w", "buffer-switch", None),
        ("w n", "buffer-next", None),
        ("w d", "buffer-close", None),
        ("w o", "buffer-terminal", None),
        ("w O", "buffer-terminal-external", None),
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
        // A command here is only legal because nothing hangs off `c` any more:
        // a prefix that is also a command is the `PrefixWithCommand` collision.
        ("c", "clipboard-open", None),
        // `a` for APPLICATIONS, at the first level. It was free, and an
        // application launcher is a first-level thing: if it costs two
        // keys, the person goes back to opening everything through the terminal.
        ("a", "apps-open", None),
        // `LEADER t` IS THE TRANSFER QUEUE. `t t` opens the panel in the
        // footer's region, doubling the key the way `s n n` and `f f` already do
        // — because `t` is a PREFIX, and hanging a command off a prefix is the
        // collision `PrefixWithCommand` exists to flag.
        ("t t", "transfers-open", None),
        // `t w` — the same queue, in a window. `w` for window, and it is the
        // second door of one thing, exactly like the file manager's `f w`.
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
        // `f f` IS THE MANAGER, in the footer's region — the same thing `.`
        // opens, and deliberately the same thing.
        //
        // It used to be the editor's file PICKER, and that was reported from
        // the screen as "a different and inferior file manager". The reading
        // was right: whoever types `f f` in a group called "file" is looking
        // for files, and what came up was a flat list of names that could only
        // do one thing with the one you chose.
        ("f f", "files-open", None),
        // `w` for WINDOW — the same manager as `f f`, in one of its own,
        // because the footer's region has a twelve-line ceiling
        // (`space.expansion`, invariant 14) and that is right for a footer and
        // not for the job people open a manager to do.
        //
        // It was `m` for "manager" and the letter was never written down; being
        // asked "why m?" is what earned it these lines — and the answer was that
        // `w` says which of the two DOORS it is, which is the actual difference.
        ("f w", "files-window", None),
        // `LEADER .` — Doom's own key for reaching a file, at the first level
        // because that is how often it is used. A manager that costs two keys
        // is a manager people go back to the terminal for.
        (".", "files-open", None),
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
        ("m s", "screen-capture", None),
        // `i` for ICONS, and not `g` for glyphs: the Nerd Fonts browser is the
        // first collection, and others are going to come in — the group is born with the
        // name of what it is going to hold, and not with the first item's.
        ("m i n", "glyphs-nerd-fonts", None),
        ("m i u", "unicode-symbols", None),
        ("m i l", "lucide-icons", None),
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
        // `b b` and not `b` alone: `b` is a PREFIX, and hanging a command off
        // a prefix is the collision `PrefixWithCommand` exists to flag.
        // Doubling the letter is the convention `s n n`, `t t`, `f f` and `n n`
        // already follow.
        ("b b", "brain-open", None),
        // AND THE TWO THE BAND DRAWS, beside the screen under the same prefix:
        // they act on what the machine ANNOUNCED, and one does not have to
        // open a screen to answer it. The band draws these two sequences.
        (BRAIN_ABORT_KEYS, "brain-abort", None),
        (BRAIN_PROCEED_KEYS, "brain-proceed", None),
        // `LEADER s u` — `u` for update, under the SYSTEM group, beside the
        // memory panel and the disk formatter. It was free, and it is where
        // somebody looks for "do something to this machine".
        ("s u", "system-update", None),
        // `LEADER s d` — THE SCREENS. `s d d` opens the view (the doubled
        // letter, the convention `s p p`, `s n n`, `t t` and `f f` already
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
        ("s b y", "bluetooth-accept", None),
        ("s b n", "bluetooth-refuse", None),
        // `s w` — `w` for WINDOW, the letter every view with a second door
        // already uses: `t w`, `f w`, `n w`, `s n w`, `s p w`, `s b w`. Seven
        // doors, one letter, and this is the seventh.
        ("s w", "update-watch", None),
        // `t` for theme, under `s` because a theme is the SYSTEM's skin and not
        // any one buffer's: the same drawer as memory, format and the brain.
        ("s t", "theme-open", None),
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
        // `n n` — the doubled letter, which is what `s n n`, `t t` and `f f`
        // already do for the view that belongs to a prefix. It was `n h` for
        // "history", and the owner asked for the convention instead.
        (NOTIF_OPEN_KEYS, "notif-open", None),
        ("n d", "notif-dismiss", None),
        // `w` for WINDOW, the same letter every view with a second door uses
        // — see the note over `s w`, which lists all seven.
        ("n w", "notif-window", None),
        ("n r", "notif-read-all", None),
        // `n f` — THE FILTERS, which had no door at all. They are the third
        // column of the manager and the only thing that named them was that
        // column's own title, so the feature existed for whoever pressed `TAB`
        // twice inside a screen they had already opened.
        ("n f", "notif-filters", None),
        // `h` for help, as in Doom. A group with a single item is the honest
        // beginning: the other help content comes in here, and the group already
        // exists to receive it.
        ("h s", "help-shortcuts", None),
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
        ("s n v", "network-import", None),
        // SYNCTHING is `s s` — its own prefix under SYSTEM, beside the
        // network it rides on, and no longer inside it.
        ("s s a", "syncthing-open", None),
        ("s s s", "syncthing-state", None),
        ("s s p", "syncthing-pause", None),
        ("s s r", "syncthing-resume", None),
    ];

    /// The digits as `&'static str`, so the binding can carry them.
    ///
    /// `format!` would give a `String`, and the binding needs `'static`. Nine
    /// literals cost less than leaking memory to fit the type.
    const DIGITS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

    const GROUPS: &[(&str, &str)] = &[
        // `b` FOR BRAIN — the buffer commands it used to hold live under `w`.
        ("b", key::GROUP_BRAIN),
        // `f` HAD NO LABEL until now. `f f` existed and the group it hangs off
        // showed up in which-key with `keymap.no-label` — a prefix that says
        // nothing about where it leads. It has two items now, and a name.
        ("f", key::GROUP_FILE),
        ("w", key::GROUP_WINDOW),
        ("TAB", key::GROUP_WORKSPACE),
        ("TAB m", key::GROUP_WORKSPACE_MOVE),
        // `m` FOR MISCELLANEOUS, and `m i` for the browsers inside it. See the
        // bindings for why the trio keeps a prefix of its own.
        ("m", key::GROUP_MISCELLANEOUS),
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
        ("h", key::GROUP_HELP),
        // `s` for SYSTEM — what the machine is doing to itself, as opposed to
        // what one is doing with it: memory, disks, the update, the theme —
        // and the network and Syncthing, which moved in from the first level.
        ("s", key::GROUP_SYSTEM),
        ("s n", key::GROUP_NETWORK),
        ("s s", key::GROUP_SYNCTHING),
        ("s d", key::GROUP_DISPLAY),
        ("s p", key::GROUP_POWER),
        ("s b", key::GROUP_BLUETOOTH),
        ("s q", key::GROUP_QUIT),
    ];

    let mut registry = Registry::fresh();
    for c in COMMANDS {
        registry.register(*c)?;
    }

    // THE COMPOSITOR'S 141 ACTIONS, each as a named command.
    //
    // "Every OS action has a command" (section 13), and what the compositor
    // knows how to do is part of the OS. The list is GENERATED from the pinned niri
    // (`nix/niri-actions.nix`): written by hand, it would age silently —
    // a removed action becomes a command that answers "error parsing request",
    // which is a `warn` in the log and nothing on screen.
    //
    // They do NOT get keys, and that is the design: 141 shortcuts would be 141
    // keys fewer for what actually gets used. They are reachable by NAME, which is
    // exactly what section 13 asks for — and the ~25 used every day have,
    // besides the name, a command of ours with its own shortcut and label.
    for a in ronoctua_niri::NIRI_ACTIONS {
        registry.register(Command {
            id: a.id,
            label: a.key,
        })?;
    }

    // THE 1-TO-9 RANGES, generated.
    //
    // Eighteen hand-written lines would be eighteen places to get a digit
    // wrong, and the mistake would be mute: the key would lead to the wrong workspace. Here the
    // key and the argument come from the SAME number.
    //
    // The `0` goes LAST: it is the keyboard's convention, where zero closes the
    // row after nine. Since the compositor has nine fixed workspaces (see
    // `nix/modules/niri.nix`), the last one is the ninth.
    const LAST: &str = "9";
    let ranges: Vec<(String, &str, &'static str)> = (1..=9)
        .map(|n| (format!("TAB {n}"), "workspace-go", DIGITS[n - 1]))
        .chain((1..=9).map(|n| (format!("TAB m {n}"), "workspace-move", DIGITS[n - 1])))
        .chain([
            ("TAB 0".to_owned(), "workspace-go", LAST),
            ("TAB m 0".to_owned(), "workspace-move", LAST),
        ])
        .collect();

    let mut bindings = Vec::with_capacity(BINDINGS.len() + ranges.len());
    for (seq, command, argument) in BINDINGS
        .iter()
        .map(|(s, c, a)| ((*s).to_owned(), *c, *a))
        .chain(ranges.into_iter().map(|(s, c, a)| (s, c, Some(a))))
    {
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
        .map(|(seq, label)| parse_sequence(seq).map(|sequence| GroupNode { sequence, label }))
        .collect();

    let tree = Tree::build_it(&bindings, &groups?).map_err(|collisions| {
        let list = collisions
            .iter()
            .map(|c| format!("  - {c}"))
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::anyhow!("the default keymap has a collision:\n{list}")
    })?;

    Ok((tree, registry))
}

/// The launcher's own key: put the focused application on the file manager's
/// sidebar.
///
/// `C-` and not a bare letter, for the reason the manager's own table gives at
/// length: in a list of NAMES the letter is query, so anything that is a
/// command has to be something a name never is. In the manager's MENU the same
/// idea is a bare `s`, because a menu row is not a list of names.
pub const OF_LAUNCHER_VIEW: &[(&str, &str)] = &[("C-s", "apps-pin")];

/// The commands the clipboard VIEW reaches, with each one's key.
///
/// They are not in the leader's tree because the view captures the whole
/// keyboard while it is open — `d` there removes, and it is not `LEADER d`. But
/// they **are commands with keys**, and that is why they go into [`reachable`]: without
/// this the orphan check would flag them, and it would be right to flag them if the list
/// did not exist.
///
/// Declared as DATA, and not hidden in a `match`, for the same reason as the
/// chords: the list is what the manual and the check read.
pub const OF_CLIPBOARD_VIEW: &[(&str, &str)] = &[
    ("RET", "clipboard-copy-of-new"),
    ("d", "clipboard-remove"),
    ("f", "clipboard-pin"),
    ("q", "clipboard-qrcode"),
    ("D", "clipboard-clear"),
    ("c", "clipboard-clipcascade-configure"),
];

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
/// top of two other items' documentation — so `OF_CLIPBOARD_VIEW` and this
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
    out.extend(OF_CLIPBOARD_VIEW.iter().map(|(_, id)| *id));
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
    out.extend(crate::display::OF_THE_VIEW.iter().map(|(_, id, _)| *id));
    out.extend(crate::bluetooth::OF_THE_VIEW.iter().map(|(_, id, _)| *id));
    out.extend(crate::network::OF_THE_VIEW.iter().map(|(_, id)| *id));
    out.extend(OF_THEME_VIEW.iter().map(|(_, id)| *id));
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
                    "states" => "filter\nnormal n\nfilter f\nsheet s".to_owned(),
                    order => {
                        seen.lock().expect("lock").push(order.to_owned());
                        "ok".to_owned()
                    }
                };
                writeln!(stream, "{reply}").expect("reply");
            }
        });

        let (now, listed) = super::buffer_states_at(&path).expect("read the states");
        // THE CURRENT ONE IS THE FIRST LINE and is NOT in the list — that is the
        // whole reason the shape differs from the keymap's.
        assert_eq!(now, "filter");
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
        for c in ['ç', '='] {
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
    /// a host shortcut (`Super+Space`) the nested niri may miss the key's
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
            tree.resolve(&[with_alt]),
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

        assert!(tree.amount() > 0, "an empty keymap");

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
        let reachable = super::reachable(&tree);
        for c in registry.all_ones() {
            assert!(!c.id.is_empty(), "a command with no id");
            assert!(!c.label.is_empty(), "a command with no label key");
            // THE COMPOSITOR'S MIRRORED ONES ARE THE EXCEPTION, and the only one.
            //
            // They exist to be reachable by NAME (section 13); giving keys
            // to all 141 would spend the whole keymap on what is almost never
            // used. The exception is recognisable by its prefix, so it cannot
            // grow without someone writing `niri-` on purpose.
            if c.id.starts_with("niri-") {
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
        assert!(matches!(
            l.key(&tree, Key::simple(Sym::Char('b'))),
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
        // `m i n` — THREE keys, which is also what this test wants: a sequence
        // that runs has to be walked to the end, and a two-key one cannot tell
        // "the last key ran it" from "any key after a group runs it". The
        // browsers keep the `i` prefix inside `m`; see the bindings.
        l.key(&tree, Key::simple(Sym::Char('m')));
        l.key(&tree, Key::simple(Sym::Char('i')));
        assert_eq!(
            l.key(&tree, Key::simple(Sym::Char('n'))),
            Effect::Run("glyphs-nerd-fonts", None)
        );
        // Leaving the leader after executing is what gives the keyboard back to the
        // buffers. Staying active would leave the system with no input.
        assert!(!l.active_one());
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
        l.key(&tree, Key::simple(Sym::Char('b')));
        assert_eq!(l.prefix("SPC"), "SPC b-");
        // Without the ellipsis: in an error message it would lie, because
        // there the sequence has ended and there is no continuation.
        assert_eq!(l.raw_prefix("SPC"), "SPC b");
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
            "glyphs-nerd-fonts",
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
            "help-shortcuts",
            "mx-open",
            "apps-open",
            "buffer-switch",
            "files-open",
            "files-window",
            "buffer-terminal",
            "buffer-terminal-external",
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
            // THE UPDATE, outside the brain. `system-update` starts the same
            // unit the brain starts and `update-watch` opens the window the
            // modeline's mark opens — which is why both are commands of the
            // registry rather than keys of a view.
            "system-update",
            "update-watch",
            "glyphs-nerd-fonts",
            "unicode-symbols",
            "lucide-icons",
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
            "clipboard-open",
            "clipboard-copy-of-new",
            "clipboard-remove",
            "clipboard-pin",
            "clipboard-qrcode",
            "clipboard-clear",
            "clipboard-clipcascade-configure",
            // NETWORK and SYNCTHING — the seventh view and the service the distro
            // installs. `network-connect` and `network-forget` act on the focused
            // row and that is why they only have keys INSIDE the view; the others
            // have keys in the tree.
            "network-open",
            "network-airplane",
            "theme-open",
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
            "network-section-previous",
            "network-wifi-toggle",
            "network-link",
            "syncthing-open",
            "syncthing-state",
            "syncthing-pause",
            "syncthing-resume",
        ];
        let (tree, registry) = default_of().unwrap();
        for id in super::reachable(&tree) {
            // The ones that are COMPOSITOR actions go through the action table,
            // consulted by id — and `niri-contract` proves, against a actual niri,
            // that every name in it exists.
            if crate::actions::kdl_of(id).is_some() {
                continue;
            }
            // The compositor's MIRRORED ones are dispatched by prefix, and
            // `run_with` sends them to niri's client. Now some
            // DO have keys — the default binds became chords — so they
            // go through here.
            if id.starts_with("niri-") {
                continue;
            }
            // The TWO workspace ones with an argument (`workspace-go`, `workspace-move`)
            // have no null action: the chord carries the number in the KDL itself, and
            // `only_a_chord_with_an_argument_escapes_the_action_table` guards that
            // exception from the other side.
            if matches!(id, "workspace-go" | "workspace-move") {
                continue;
            }
            assert!(
                WITH_ACTION.contains(&id),
                "`{id}` has a key and `surface::run_command` does not handle it: \
                 it would show up in the list and do nothing"
            );
        }
        // And the inverse: an id in the list above that left the registry would become a
        // dead branch in the `match`, waiting for a key that does not exist.
        for id in WITH_ACTION {
            assert!(
                registry.search(id).is_some(),
                "`{id}` is in the handled list and no longer exists"
            );
        }
    }
}
