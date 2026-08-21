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
}

/// The word a new footer says to the one it is going to replace.
const YIELD: &str = "sair";

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
    tracing::info!("ja ha um rodape neste display; pedindo o lugar a ele");
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
            tracing::info!("o rodape anterior saiu");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    tracing::warn!(
        "o rodape anterior nao saiu em {} ms; tomando o socket.\n\
         Ele pode ficar com uma superficie na tela ate ser encerrado a mao.",
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
        .context("XDG_RUNTIME_DIR nao esta definida; sem ela nao ha onde por o socket")?;
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
        .context("XDG_RUNTIME_DIR nao esta definida; sem ela nao ha onde por o socket")?;
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
    // Splitting on the first gave `("m", "f s salvar-arquivo")` — a plausible
    // pair that would have listed one key and run nothing.
    Ok(answer
        .lines()
        .filter_map(|l| l.trim().rsplit_once(' '))
        .map(|(keys, command)| (keys.to_owned(), command.to_owned()))
        .collect())
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
        .with_context(|| format!("nenhum buffer escutando em {}", path.display()))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
    writeln!(stream, "command {id}").context("nao consegui escrever no socket do buffer")?;
    // THE ANSWER IS READ, and not assumed. The buffer refuses an id it did not
    // declare, and a caller that never looked would report success for a key
    // that did nothing — the exact failure this whole path exists to avoid.
    let mut answer = String::new();
    std::io::Read::read_to_string(&mut stream, &mut answer).context("o buffer nao respondeu")?;
    anyhow::ensure!(
        answer.trim() == "ok",
        "o buffer recusou `{id}`: {}",
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
            "nao ha rodape escutando em {}.\n\
             \n\
             `ronoctua-shell leader` avisa um rodape JA rodando; ele nao sobe um.",
            path.display()
        )
    })?;
    writeln!(s, "{order}").context("nao consegui escrever no socket de controle")?;
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
            tracing::warn!(error = %e, "sem socket de controle; a leader nao vai funcionar");
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

    let ouvinte = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "nao consegui abrir o socket");
            return false;
        }
    };
    tracing::info!(path = %path.display(), "socket de controle aberto");

    thread::Builder::new()
        .name("controle".into())
        .spawn(move || {
            for connection in ouvinte.incoming() {
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
                    _ => {
                        tracing::warn!(order = line.trim(), "ordem desconhecida no controle");
                        continue;
                    }
                };
                if channel.send(order).is_err() {
                    return;
                }
            }
        })
        .expect("a thread de controle nao pode falhar em nascer");

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
    pub fn prefix(&self, gatilho: &str) -> String {
        if self.sequence.is_empty() {
            format!("{gatilho}-")
        } else {
            format!("{gatilho} {}-", ronoctua_keymap::notation(&self.sequence))
        }
    }

    /// The typed sequence, WITHOUT the continuation ellipsis.
    ///
    /// The `prefix` ends in `…` because it says "keys are missing". In an
    /// error message that would be a lie: there the sequence ENDED, and with no
    /// destination. Two readings, two functions.
    #[must_use]
    pub fn raw_prefix(&self, gatilho: &str) -> String {
        if self.sequence.is_empty() {
            gatilho.to_owned()
        } else {
            format!("{gatilho} {}", ronoctua_keymap::notation(&self.sequence))
        }
    }

    /// Activates the leader from scratch.
    pub fn ativar(&mut self) {
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

        let sem_mods = Key {
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
            let trocada = if c.is_uppercase() {
                c.to_lowercase().next()?
            } else if c.is_lowercase() {
                c.to_uppercase().next()?
            } else {
                return None;
            };
            Some(Key {
                sym: Sym::Char(trocada),
                ..t
            })
        };

        // IN ORDER OF MOST LIKELY CAUSE, and stopping at the first that matches.
        // Removing the modifier comes first because it is the case the person cannot
        // undo on their own.
        let candidatas = [
            Some(sem_mods).filter(|t| *t != last_one),
            other_case(last_one),
            other_case(sem_mods),
        ];

        let mut attempt = self.sequence.clone();
        for target in candidatas.into_iter().flatten() {
            *attempt.last_mut()? = target;
            if !matches!(tree.resolve(&attempt), ronoctua_keymap::Resolution::Nothing) {
                return Some(target);
            }
        }
        None
    }
}

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
        // `LEADER b b` — the buffer SELECTOR, with completion.
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
        // A manager and not a picker: `f f` above hands a path to the editor
        // and stops there. This one lists everything a directory holds —
        // permissions, owner, group, size, date, and a glyph for what each
        // thing is — and acts on it.
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
        Command {
            id: "memory-open",
            label: key::CMD_MEMORY_OPEN,
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
        Command {
            id: "notif-history",
            label: key::CMD_NOTIF_HISTORY,
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
        // AND THE TWO THAT PUT SOMETHING IN. What they read is the clipboard,
        // because an address is copied and never typed — see
        // `Footer::queue_the_clipboard`.
        Command {
            id: "transfers-add",
            label: key::CMD_TRANSFERS_ADD,
        },
        Command {
            id: "transfers-add-media",
            label: key::CMD_TRANSFERS_ADD_MEDIA,
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
        // is from INSIDE the view — they come in through `OF_NETWORK_VIEW`.
        // ---------------------------------------------------------------
        Command {
            id: "network-open",
            label: key::CMD_NETWORK_OPEN,
        },
        Command {
            id: "network-scan",
            label: key::CMD_NETWORK_SCAN,
        },
        Command {
            id: "network-connect",
            label: key::CMD_NETWORK_CONNECT,
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
        // are GENERATED from this same command table — see `actions::acordes`
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
        // `c` STOPPED BEING A COMMAND AND BECAME A GROUP.
        //
        // The screen capture used to be `t c`, and `t` is now the transfers
        // group — so it had to move, and the owner chose where: `c` for
        // CAPTURE, which is what both things under it do. The clipboard
        // captures what you copy; the screen captures what you see.
        //
        // It costs the clipboard one keystroke (`c` became `c c`), and that is
        // the convention this keymap already uses everywhere a prefix gained a
        // second item — `r r`, `f f`, `TAB TAB`.
        ("c c", "clipboard-open", None),
        ("c s", "screen-capture", None),
        // `a` for APPLICATIONS, at the first level. It was free, and an
        // application launcher is a first-level thing: if it costs two
        // keys, the person goes back to opening everything through the terminal.
        ("a", "apps-open", None),
        // `LEADER t` IS THE TRANSFER QUEUE. `t t` opens the panel in the
        // footer's region, doubling the key the way `r r` and `f f` already do
        // — because `t` is a PREFIX, and hanging a command off a prefix is the
        // collision `PrefixWithCommand` exists to flag.
        ("t t", "transfers-open", None),
        // `t w` — the same queue, in a window. `w` for window, and it is the
        // second door of one thing, exactly like the file manager's `f w`.
        ("t w", "transfers-window", None),
        // `t a` — add whatever was copied: a link, a magnet, a `.torrent`.
        ("t a", "transfers-add", None),
        // `t m` — the same address, but as MEDIA: yt-dlp resolves the page.
        ("t m", "transfers-add-media", None),
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
        ("b b", "buffer-switch", None),
        ("b n", "buffer-next", None),
        ("b k", "buffer-close", None),
        // `i` for ICONS, and not `g` for glyphs: the Nerd Fonts browser is the
        // first collection, and others are going to come in — the group is born with the
        // name of what it is going to hold, and not with the first item's.
        ("i n", "glyphs-nerd-fonts", None),
        ("i u", "unicode-symbols", None),
        ("i l", "lucide-icons", None),
        // `LEADER s m` — the memory panel. The machine already had a policy
        // against freezing and no screen ever said so; this is where it
        // becomes something one can look at, and loosen.
        ("s m", "memory-open", None),
        ("q q", "session-quit", None),
        // `LEADER :` — `M-x`'s key in Doom, which is where the concept
        // comes from. It used to be `x`, which in Doom is the scratch buffer and not the locator.
        // One key only: with both, the list showed the same thing on two
        // lines, and the list is the scarce resource here.
        (":", "mx-open", None),
        // `x` TOO, at the owner's request. The two coexist because they are two
        // different reflexes: `:` is Doom's, `x` is Emacs's `M-x`.
        ("x", "mx-open", None),
        ("b t", "buffer-terminal", None),
        ("b T", "buffer-terminal-external", None),
        // NOTIFICATIONS. Both were socket orders with no key at all: the
        // history only opened through `ronoctua-shell history`, typed in a
        // terminal. A feature with no path is a feature nobody
        // finds.
        ("n h", "notif-history", None),
        ("n d", "notif-dismiss", None),
        // `h` for help, as in Doom. A group with a single item is the honest
        // beginning: the other help content comes in here, and the group already
        // exists to receive it.
        ("h s", "help-shortcuts", None),
        // ---------------------------------------------------------------
        // NETWORK. `r r` opens the list, and the others act directly.
        //
        // `r r` and not just `r` because `r` is a PREFIX: hanging a command off it
        // is having the collision `PrefixWithCommand` exists to flag.
        // Doubling the key is the convention `TAB TAB` already uses here.
        // ---------------------------------------------------------------
        ("r r", "network-open", None),
        ("r v", "network-scan", None),
        ("r w", "network-wifi-toggle", None),
        ("r d", "network-disconnect", None),
        // `i` for INFORMATION. It shows band, rate and signal in the minibuffer —
        // it is the answer to "why is this slow?" without leaving the screen.
        ("r i", "network-link", None),
        ("r s a", "syncthing-open", None),
        ("r s s", "syncthing-state", None),
        ("r s p", "syncthing-pause", None),
        ("r s r", "syncthing-resume", None),
    ];

    /// The digits as `&'static str`, so the binding can carry them.
    ///
    /// `format!` would give a `String`, and the binding needs `'static`. Nine
    /// literals cost less than leaking memory to fit the type.
    const DIGITS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

    const GROUPS: &[(&str, &str)] = &[
        ("b", key::GROUP_BUFFER),
        // `f` HAD NO LABEL until now. `f f` existed and the group it hangs off
        // showed up in which-key with `keymap.sem-rotulo` — a prefix that says
        // nothing about where it leads. It has two items now, and a name.
        ("f", key::GROUP_FILE),
        ("w", key::GROUP_WINDOW),
        ("TAB", key::GROUP_WORKSPACE),
        ("TAB m", key::GROUP_WORKSPACE_MOVE),
        ("i", key::GROUP_ICONS),
        // `t` FOR TRANSFERS. It held the screen capture alone and gave the
        // letter up: a queue that everything long goes through earns a
        // first-level prefix, and a single screenshot does not.
        ("t", key::GROUP_TRANSFER),
        // `c` FOR CAPTURE — the clipboard and the screen, which is where the
        // screenshot went.
        ("c", key::GROUP_CAPTURE),
        ("n", key::GROUP_NOTIFICATION),
        ("h", key::GROUP_HELP),
        ("q", key::GROUP_QUIT),
        ("r", key::GROUP_NETWORK),
        ("r s", key::GROUP_SYNCTHING),
        // `s` for SYSTEM — what the machine is doing to itself, as opposed to
        // what one is doing with it. It starts with one item, which is the
        // honest beginning: the group is born with the name of what it will
        // hold, not with the first thing in it.
        ("s", key::GROUP_SYSTEM),
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

    let mut ligacoes = Vec::with_capacity(BINDINGS.len() + ranges.len());
    for (seq, command, argument) in BINDINGS
        .iter()
        .map(|(s, c, a)| ((*s).to_owned(), *c, *a))
        .chain(ranges.into_iter().map(|(s, c, a)| (s, c, Some(a))))
    {
        let cmd = registry.search(command).ok_or_else(|| {
            anyhow::anyhow!("a ligacao `{seq}` aponta para o comando `{command}`, que nao existe")
        })?;
        // A COMMAND WITH AN ARGUMENT ONLY ACCEPTS A BINDING WITH AN ARGUMENT, and the
        // converse too: `workspace-go` with no number does not know where to go, and
        // `buffer-close` with a number receives something it silently ignores.
        anyhow::ensure!(
            crate::actions::with_argument(command).is_some() == argument.is_some(),
            "a ligacao `{seq}` e o comando `{command}` discordam sobre argumento"
        );
        ligacoes.push(Binding {
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

    let tree = Tree::build_it(&ligacoes, &groups?).map_err(|collisions| {
        let list = collisions
            .iter()
            .map(|c| format!("  - {c}"))
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::anyhow!("o keymap padrao tem colisao:\n{list}")
    })?;

    Ok((tree, registry))
}

/// Every command SOME key in the system reaches.
///
/// # Why the three places
///
/// The keymap resolves sequences, but it is not the system's only source of
/// keys: the **chords** live in the compositor's config (the footer never
/// sees them — §7.4) and the **emergency** binds live outside the leader on purpose.
/// Asking "does this have a shortcut?" of the keymap alone answers wrongly for two thirds
/// of the shortcuts.
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

/// The commands the NETWORK VIEW reaches, with each one's key.
///
/// The same reason as the clipboard list: while the view is open it
/// captures the whole keyboard, so `f` there is "forget this network" and not
/// `LEADER f`. They **are** commands with keys, and without this list the orphan
/// check would flag them — rightly, because a command with no path at all is a
/// command nobody uses.
///
/// `RET` and `f` act on the FOCUSED ROW, and that is why they cannot have a
/// key in the leader's tree: outside the list there is no focused row to
/// act on.
pub const OF_NETWORK_VIEW: &[(&str, &str)] = &[
    ("RET", "network-connect"),
    ("f", "network-forget"),
    ("r", "network-scan"),
    ("w", "network-wifi-toggle"),
    ("d", "network-disconnect"),
];

#[must_use]
pub fn reachable(tree: &Tree) -> std::collections::BTreeSet<&'static str> {
    let mut out: std::collections::BTreeSet<&'static str> =
        tree.reachable_commands().into_iter().collect();
    out.extend(
        crate::actions::acordes_completos()
            .into_iter()
            .map(|a| a.command),
    );
    out.extend(
        crate::actions::emergency()
            .into_iter()
            .map(|(_, _, id)| id)
            // The alternative leader is not a command: it fires the sequence.
            .filter(|id| *id != crate::actions::LEADER_ALTERNATE),
    );
    out.extend(OF_CLIPBOARD_VIEW.iter().map(|(_, id)| *id));
    out.extend(OF_NETWORK_VIEW.iter().map(|(_, id)| *id));
    out
}

#[cfg(test)]
mod tests {
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
                    .expect("ler");
                let reply = match line.trim() {
                    "keymap" => "m f s salvar-arquivo\nm f r recarregar-arquivo".to_owned(),
                    order => {
                        seen.lock().expect("lock").push(order.to_owned());
                        "ok".to_owned()
                    }
                };
                writeln!(stream, "{reply}").expect("responder");
            }
        });

        let keymap = super::buffer_keymap_at(&path).expect("ler o keymap");
        assert_eq!(
            keymap,
            vec![
                ("m f s".to_owned(), "salvar-arquivo".to_owned()),
                ("m f r".to_owned(), "recarregar-arquivo".to_owned()),
            ],
            "a sequencia leva espacos, entao so o PRIMEIRO separa dela o id"
        );

        super::buffer_command_at(&path, "salvar-arquivo").expect("rodar o comando");
        served.join().expect("a thread do falso buffer");
        assert_eq!(
            *ran.lock().expect("lock"),
            vec!["command salvar-arquivo".to_owned()],
            "o comando tem de chegar do lado do buffer, e nao so ser enviado"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// No buffer listening is a NORMAL answer, and not an error: most focused
    /// windows are not ours, and `SPC m` has to be absent there rather than
    /// showing a failure.
    #[test]
    fn no_buffer_listening_gives_an_empty_keymap() {
        let path = std::env::temp_dir().join("ronoctua-nao-existe-mesmo.sock");
        let _ = std::fs::remove_file(&path);
        assert!(
            super::buffer_keymap_at(&path)
                .expect("nao pode ser erro")
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
        std::fs::create_dir_all(&dir).expect("diretorio temporario");
        let path = dir.join("controle.sock");

        let (envio1, recebe1) = channel::channel::<Order>();
        assert!(
            listen_on(&path, envio1),
            "o primeiro rodape tem de conseguir abrir o socket"
        );

        // THE SECOND. It is the one that has to ask for the place.
        let (envio2, _recebe2) = channel::channel::<Order>();
        assert!(
            listen_on(&path, envio2),
            "o segundo rodape tem de conseguir assumir o socket"
        );

        // calloop's channel does not block, and the first one's control thread
        // passes the order along as soon as it reads it — but "as soon as" is not "already". A
        // short wait with an early exit measures the mechanism, not the machine.
        let up_to = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            match recebe1.try_recv() {
                Ok(Order::Quit) => break,
                Ok(otherwise_one) => panic!("o primeiro recebeu {otherwise_one:?} em vez de sair"),
                Err(_) if std::time::Instant::now() < up_to => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => panic!(
                    "o primeiro rodape NAO foi avisado de que perdeu o display.\n\
                     \n\
                     Ele continua vivo, com a superficie na tela e o teclado no\n\
                     seat. Se ele estiver com a leader aberta, e ele que recebe\n\
                     as teclas que o rodape novo esta esperando."
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
        let (tree, _) = default_of().expect("o keymap padrao tem de fechar");
        let mut l = Leader::default();
        l.ativar();
        match l.key(&tree, Key::simple(Sym::Char('I'))) {
            Effect::Close {
                other_case: Some(t),
            } => assert_eq!(t.sym, Sym::Char('i'), "a sugestao tem de ser a minuscula"),
            otherwise => panic!("`I` devia encerrar apontando `i`; veio {otherwise:?}"),
        }
    }

    #[test]
    fn with_no_other_case_there_is_no_suggestion() {
        // The suggestion must not become noise: a key that exists in no case at
        // all ends without pointing at anything, and one that HAS no case (a digit,
        // punctuation) too.
        let (tree, _) = default_of().expect("o keymap padrao tem de fechar");
        for c in ['ç', '='] {
            let mut l = Leader::default();
            l.ativar();
            assert!(
                matches!(
                    l.key(&tree, Key::simple(Sym::Char(c))),
                    Effect::Close { other_case: None }
                ),
                "`{c}` nao existe em caixa nenhuma; nao ha o que sugerir"
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
        let (tree, _) = default_of().expect("o keymap padrao tem de fechar");
        let com_alt = Key {
            mods: Mods {
                ctrl: false,
                alt: true,
                shift: false,
                logo: false,
            },
            sym: Sym::Char('i'),
        };
        // The premise: `M-i` is not bound to anything, and `i` is.
        assert!(matches!(
            tree.resolve(&[com_alt]),
            ronoctua_keymap::Resolution::Nothing
        ));

        let mut l = Leader::default();
        l.ativar();
        match l.key(&tree, com_alt) {
            Effect::Close {
                other_case: Some(t),
            } => {
                assert_eq!(t.sym, Sym::Char('i'));
                assert!(!t.mods.alt, "a sugestao tem de ser a tecla SEM o Alt");
            }
            otherwise => panic!("`M-i` devia encerrar apontando `i`; veio {otherwise:?}"),
        }
    }

    #[test]
    fn the_other_case_runs_nothing() {
        // THE SUGGESTION IS NOT A FALLBACK. `w H` moves the window and `w h` focuses: falling
        // from one to the other would make the shortcut do SOMETHING ELSE, which is worse than
        // doing nothing. Here `w` followed by a non-existent key ends the sequence —
        // never `Run`.
        let (tree, _) = default_of().expect("o keymap padrao tem de fechar");
        let mut l = Leader::default();
        l.ativar();
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
            "tem de encerrar apontando `t`, e NAO executar `window-float`"
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
        let (tree, registry) = default_of().expect("o keymap padrao tem de fechar");

        assert!(tree.amount() > 0, "keymap vazio");

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
            assert!(!c.id.is_empty(), "comando sem id");
            assert!(!c.label.is_empty(), "comando sem chave de rotulo");
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
                "`{}` e comando NOSSO e nenhuma tecla o alcanca: \
                 nem sequencia, nem acorde, nem bind de emergencia",
                c.id
            );
        }

        // And the converse: nothing points at a command that does not exist. `default_of()`
        // already refuses to build in that case; this proves the refusal is wired up.
        for id in reachable {
            assert!(
                registry.search(id).is_some(),
                "ha tecla apontando para `{id}`, que nao esta no registro"
            );
        }
    }

    #[test]
    fn esc_always_ends_it() {
        // Even with the sequence half-typed. Without an unconditional exit, a
        // lost user has no way back.
        let (tree, _) = default_of().unwrap();
        let mut l = Leader::default();
        l.ativar();
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
        l.ativar();
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
        l.ativar();
        // `i n`, and not `g g`: the group became `i` for ICONS, because the
        // Nerd Fonts browser is the first collection and others are going to come in.
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
            l.ativar();
            l.key(&tree, Key::simple(Sym::Tab));
            let d = char::from_digit(u32::from(n), 10).unwrap();
            match l.key(&tree, Key::simple(Sym::Char(d))) {
                Effect::Run(id, Some(arg)) => {
                    assert_eq!(id, "workspace-go");
                    assert_eq!(
                        arg.parse::<u8>().unwrap(),
                        n,
                        "a tecla e o argumento divergiram"
                    );
                }
                otherwise => panic!("`TAB {d}` nao levou ao espaco {n}: {otherwise:?}"),
            }
        }
    }

    #[test]
    fn the_zero_goes_to_the_last_workspace() {
        // The keyboard's convention: zero closes the row after nine. With
        // nine fixed workspaces in the compositor, the last is the ninth.
        let (tree, _) = default_of().unwrap();
        let mut l = Leader::default();
        l.ativar();
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
            l.ativar();
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
                "`TAB {c}` nao leva a comando nenhum"
            );
            assert!(
                matches!(ir(&[tab, m, key]), Effect::Run(..)),
                "`TAB m {c}` nao leva a comando nenhum"
            );
        }
        // And the arrows, in both.
        for sym in [Sym::Up, Sym::Down] {
            let key = Key::simple(sym);
            assert!(matches!(ir(&[tab, key]), Effect::Run(..)), "{sym:?} em TAB");
            assert!(
                matches!(ir(&[tab, m, key]), Effect::Run(..)),
                "{sym:?} em TAB m"
            );
        }
    }

    #[test]
    fn the_prefix_is_dooms_notation() {
        let (tree, _) = default_of().unwrap();
        let mut l = Leader::default();
        l.ativar();
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
            assert!(registry.search(id).is_some(), "falta o comando {id}");
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
            "notif-history",
            "notif-dismiss",
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
            "memory-open",
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
            "transfers-add",
            "transfers-add-media",
            // THE CLIPBOARD. `clipboard-open` has a key in the tree; the other
            // six have keys INSIDE the view, and that is why they go through
            // `reachable` — all seven need an action, otherwise they would appear in the
            // list and do nothing.
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
            "network-scan",
            "network-connect",
            "network-disconnect",
            "network-forget",
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
            // The TWO workspace ones with an argument (`espaco-ir`, `espaco-levar`)
            // have no null action: the chord carries the number in the KDL itself, and
            // `only_a_chord_with_an_argument_escapes_the_action_table` guards that
            // exception from the other side.
            if matches!(id, "workspace-go" | "workspace-move") {
                continue;
            }
            assert!(
                WITH_ACTION.contains(&id),
                "`{id}` tem tecla e o `surface::run_command` nao o trata: \
                 ele apareceria na lista e nao faria nada"
            );
        }
        // And the inverse: an id in the list above that left the registry would become a
        // dead branch in the `match`, waiting for a key that does not exist.
        for id in WITH_ACTION {
            assert!(
                registry.search(id).is_some(),
                "`{id}` esta na lista de tratados e nao existe mais"
            );
        }
    }
}
