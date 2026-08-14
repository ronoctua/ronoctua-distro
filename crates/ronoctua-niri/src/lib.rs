//! The only boundary with niri's IPC — invariant 3, section 7.3.
//!
//! Node other crate talks to the compositor. If some day a new piece of data from
//! niri is needed, it comes in HERE and goes out as a type of ours; a
//! `serde_json::Value` is not spread through the shell.
//!
//! # Why our own types, and not the `niri-ipc` crate
//!
//! Milestone 4's acceptance criterion is "the contract passes against the pinned tag
//! **and** against `niri-next`". Depending on the published `niri-ipc` crate would make
//! that impossible to verify: Cargo would pin ONE version of the types, and the
//! test against `niri-next` would be checking the old version of the types against
//! the new binary — that is, nothing. With our own types, the same code is
//! exercised against both actual binaries, which is the only way for the drift warning
//! to arrive BEFORE the release becomes mandatory (risk R2).
//!
//! # Tolerance of unknown variants
//!
//! Events are read as `serde_json::Value` and only then mapped. That is
//! deliberate: niri versions by calendar and ADDS `Event` variants
//! frequently. A Rust enum with `deny_unknown` would break on every
//! release over an event that does not interest us. Here, an unknown event
//! becomes [`Event::Ignored`] and life goes on — while the contract test
//! goes on proving that the ones we USE exist.

#![forbid(unsafe_code)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

/// The variable niri exports to its children with the socket's path.
pub const SOCKET_ENV: &str = "NIRI_SOCKET";

/// A niri window, reduced to what the modeline uses (section 7.1 of the plan).
///
/// Fields niri has and we do not: position, size, column, workspace,
/// urgency, focus timestamp. They come in when a segment needs them, and
/// not before — every field here is contract surface to maintain.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Window {
    /// A stable id for as long as the window exists.
    pub id: u64,
    /// The title, when the window sets one.
    pub title: Option<String>,
    /// Application ID — it becomes the modeline's `major-mode` (section 4.3).
    pub app_id: Option<String>,
    /// The PID of whoever opened the Wayland connection.
    ///
    /// It is the key to section 7.5's PID -> cgroup correlation. It may be `None`;
    /// in that case the modeline degrades, it does not invent.
    pub pid: Option<i32>,
    /// Whether it is the focused window. There are zero or one.
    pub is_focused: bool,
}

/// A compositor event, reduced to what the daemon consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The complete window state. It is always the first to arrive.
    WindowsChanged(Vec<Window>),
    /// A window opened or had some property changed.
    WindowOpenedOrChanged(Window),
    /// A window closed.
    WindowClosed(u64),
    /// Focus changed. `None` when nothing is focused — for example, when a
    /// layer-shell surface (ours, with the leader active) takes it.
    FocusChanged(Option<u64>),
    /// Any other event. See the comment at the top of the module.
    Ignored,
}

/// A connection to niri's IPC.
pub struct Niri {
    stream: BufReader<UnixStream>,
}

impl Niri {
    /// Connects to the socket `NIRI_SOCKET` points at.
    pub fn connect() -> Result<Self> {
        let path = std::env::var_os(SOCKET_ENV)
            .map(PathBuf::from)
            .with_context(|| {
                format!(
                    "{SOCKET_ENV} nao esta definida.\n\
                     \n\
                     O niri define essa variavel para os processos que ELE inicia.\n\
                     Fora de uma sessao do niri, aponte-a para o socket a mao."
                )
            })?;
        Self::connect_at(&path)
    }

    /// Connects to a specific socket. Used by the contract test, which
    /// has to talk to two different niris on the same machine.
    pub fn connect_at(path: &std::path::Path) -> Result<Self> {
        let stream = UnixStream::connect(path)
            .with_context(|| format!("nao consegui conectar em {}", path.display()))?;
        Ok(Self {
            stream: BufReader::new(stream),
        })
    }

    /// Sends a request and returns the raw reply.
    ///
    /// The protocol is one JSON line per message: request, `\n`, and the reply
    /// comes on ONE line. niri's `Reply` is a serialised `Result`, so
    /// on the wire it appears as `{"Ok": …}` or `{"Err": "…"}`.
    fn request(&mut self, pedido: &serde_json::Value) -> Result<serde_json::Value> {
        let mut text = serde_json::to_string(pedido)?;
        text.push('\n');
        self.stream
            .get_mut()
            .write_all(text.as_bytes())
            .context("nao consegui escrever no socket do niri")?;
        self.stream.get_mut().flush()?;

        let mut line = String::new();
        let n = self
            .stream
            .read_line(&mut line)
            .context("nao consegui ler a resposta do niri")?;
        if n == 0 {
            bail!("o niri fechou a conexao sem responder a {pedido}");
        }

        let answer: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("resposta do niri nao e JSON: {line:?}"))?;

        if let Some(error) = answer.get("Err") {
            bail!("o niri recusou {pedido}: {error}");
        }
        answer
            .get("Ok")
            .cloned()
            .with_context(|| format!("resposta do niri sem `Ok` nem `Err`: {line:?}"))
    }

    /// The running compositor's version.
    pub fn version(&mut self) -> Result<String> {
        let ok = self.request(&serde_json::json!("Version"))?;
        ok.get("Version")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .with_context(|| format!("resposta de Version com forma inesperada: {ok}"))
    }

    /// Every open window.
    pub fn windows(&mut self) -> Result<Vec<Window>> {
        let ok = self.request(&serde_json::json!("Windows"))?;
        let raw = ok
            .get("Windows")
            .with_context(|| format!("resposta de Windows com forma inesperada: {ok}"))?;
        serde_json::from_value(raw.clone()).context("uma janela do niri nao casa com `Janela`")
    }

    /// The focused window, if there is one.
    pub fn focused_window(&mut self) -> Result<Option<Window>> {
        let ok = self.request(&serde_json::json!("FocusedWindow"))?;
        let raw = ok
            .get("FocusedWindow")
            .with_context(|| format!("resposta de FocusedWindow com forma inesperada: {ok}"))?;
        serde_json::from_value(raw.clone()).context("a janela focada nao casa com `Janela`")
    }

    /// Sends an ACTION to the compositor.
    ///
    /// # Why the shell sends an action, and does not simulate a key
    ///
    /// Closing a window and switching focus are the compositor's business
    /// (invariant 4). The shell does not reimplement them: it asks, over the same IPC
    /// it already listens on. Simulating a key would go through niri's keymap, which is
    /// another configuration and another truth.
    ///
    /// The action goes as it is in niri's protocol. Building an enum of
    /// actions here would mirror theirs and would diverge on the first new version; what
    /// covers the shape is the contract test.
    ///
    /// # Errors
    ///
    /// niri refused the action, or the socket went down.
    pub fn action(&mut self, action: serde_json::Value) -> Result<()> {
        self.request(&serde_json::json!({ "Action": action }))?;
        Ok(())
    }

    /// Closes the focused window.
    ///
    /// # Errors
    ///
    /// The same as [`Niri::action`].
    pub fn close_window(&mut self) -> Result<()> {
        // `id: null` means "the focused one" in niri's protocol. Sending the id
        // the daemon keeps would be a race: between the event and the action the
        // focus may have changed, and then we would close the wrong window — which is
        // destructive and silent.
        self.action(serde_json::json!({ "CloseWindow": { "id": null } }))
    }

    /// Passes focus to the next window.
    ///
    /// # Errors
    ///
    /// The same as [`Niri::action`].
    pub fn next_window(&mut self) -> Result<()> {
        self.action(serde_json::json!({ "FocusWindowDown": {} }))
    }

    /// An action WITHOUT an argument, by the name niri uses in the protocol.
    ///
    /// # Why the raw name, and not an enum of ours
    ///
    /// niri has 142 actions. Mirroring them in an enum would give a table that
    /// diverges on the first new version of the compositor — and diverging here
    /// means a command that disappears from the modeline with nobody noticing. The name
    /// crosses as it is; what guarantees it exists is the contract
    /// test, which exercises each one against a actual niri.
    ///
    /// # Errors
    ///
    /// niri does not know the action, or the socket went down.
    pub fn plain_action(&mut self, name: &str) -> Result<()> {
        self.action(serde_json::json!({ name: {} }))
    }

    /// The layer-shell surfaces the compositor knows about.
    ///
    /// Each comes out as `namespace@layer/interactivity`.
    ///
    /// It does not feed the modeline. It exists for two testing reasons, both about
    /// questions no log of ours answers:
    ///
    /// - "does the compositor SEE our footer?" A footer that shows up here but
    ///   not on screen is a buffer problem; one that does not even show up is a
    ///   protocol problem. They are different fixes.
    /// - "did the compositor accept the keyboard request?" The interactivity IT
    ///   reports is milestone 5's criterion, and it is different from the one we asked for —
    ///   asking is not getting.
    pub fn layers(&mut self) -> Result<Vec<String>> {
        let ok = self.request(&serde_json::json!("Layers"))?;
        let raw = ok
            .get("Layers")
            .and_then(serde_json::Value::as_array)
            .with_context(|| format!("resposta de Layers com forma inesperada: {ok}"))?;
        Ok(raw
            .iter()
            .map(|c| {
                let ns = c
                    .get("namespace")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?");
                let layer = c
                    .get("layer")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?");
                let keyboard = c
                    .get("keyboard_interactivity")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?");
                format!("{ns}@{layer}/{keyboard}")
            })
            .collect())
    }

    /// Turns the connection into an event stream.
    ///
    /// It consumes the connection on purpose: after `EventStream` the socket is only
    /// good for that. A request sent afterwards would be read by niri as
    /// garbage, and the error would show up far from the cause.
    pub fn event_stream(mut self) -> Result<Events> {
        let ok = self.request(&serde_json::json!("EventStream"))?;
        // `Response::Handled` is a UNIT variant, so on the wire it is the
        // string `"Handled"`, and not `{"Handled": …}` like the ones that carry
        // data. Looking for the key in an object would always fail, and the message
        // would say "niri did not confirm" when it did.
        anyhow::ensure!(
            ok.as_str() == Some("Handled"),
            "o niri nao confirmou o EventStream: {ok}"
        );
        Ok(Events {
            stream: self.stream,
        })
    }
}

/// An event stream already accepted by the compositor.
///
/// niri sends the complete state on the way out, so the reader does not have to ask for
/// `Windows` first: the first window event is already the whole list.
pub struct Events {
    stream: BufReader<UnixStream>,
}

impl Events {
    /// Reads the next event. It blocks. `Ok(None)` when niri shuts down.
    pub fn next_one(&mut self) -> Result<Option<Event>> {
        let mut line = String::new();
        if self.stream.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let raw: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("evento do niri nao e JSON: {line:?}"))?;
        Ok(Some(interpret(&raw)?))
    }
}

/// Maps an event's raw JSON to our narrow type.
///
/// Separated from the reading so it can be tested without a socket: the tests below pass
/// literal JSON, which is how a actual event arrives on the wire.
fn interpret(raw: &serde_json::Value) -> Result<Event> {
    let Some((name, body)) = raw.as_object().and_then(|o| o.iter().next()) else {
        bail!("evento sem variante: {raw}");
    };

    let event = match name.as_str() {
        "WindowsChanged" => {
            let windows = body
                .get("windows")
                .context("WindowsChanged sem `windows`")?;
            Event::WindowsChanged(
                serde_json::from_value(windows.clone())
                    .context("uma janela de WindowsChanged nao casa com `Janela`")?,
            )
        }
        "WindowOpenedOrChanged" => {
            let window = body
                .get("window")
                .context("WindowOpenedOrChanged sem `window`")?;
            Event::WindowOpenedOrChanged(
                serde_json::from_value(window.clone())
                    .context("a janela de WindowOpenedOrChanged nao casa com `Janela`")?,
            )
        }
        "WindowClosed" => Event::WindowClosed(
            body.get("id")
                .and_then(serde_json::Value::as_u64)
                .context("WindowClosed sem `id`")?,
        ),
        "WindowFocusChanged" => {
            Event::FocusChanged(body.get("id").and_then(serde_json::Value::as_u64))
        }
        _ => Event::Ignored,
    };
    Ok(event)
}

/// The state the modeline reads, maintained by the event stream.
///
/// It lives in the DAEMON, not in the surface — section 9.2 of the plan decided it that way
/// so a prompt survives the focused output disappearing.
#[derive(Debug, Default)]
pub struct State {
    windows: Vec<Window>,
    focus: Option<u64>,
}

impl State {
    /// Applies an event. It is the only way to change the state.
    pub fn apply_to(&mut self, event: &Event) {
        match event {
            Event::WindowsChanged(js) => {
                // niri sends the focus INSIDE the window, and the focus event
                // may not come with it. Reading from there keeps the two coherent.
                self.focus = js.iter().find(|j| j.is_focused).map(|j| j.id);
                self.windows = js.clone();
            }
            Event::WindowOpenedOrChanged(j) => {
                if let Some(existente) = self.windows.iter_mut().find(|x| x.id == j.id) {
                    *existente = j.clone();
                } else {
                    self.windows.push(j.clone());
                }
                // The event's documentation is explicit: "If the window is
                // focused, all other windows are no longer focused." Without
                // this, a window that opens ALREADY focused only shows up in the modeline
                // if niri also sends `WindowFocusChanged` — and depending on
                // that is depending on an order nobody promised.
                if j.is_focused {
                    self.focus = Some(j.id);
                    for otherwise_one in &mut self.windows {
                        otherwise_one.is_focused = otherwise_one.id == j.id;
                    }
                }
            }
            Event::WindowClosed(id) => {
                self.windows.retain(|j| j.id != *id);
                if self.focus == Some(*id) {
                    self.focus = None;
                }
            }
            Event::FocusChanged(id) => self.focus = *id,
            Event::Ignored => {}
        }
    }

    /// The focused window, if there is one.
    #[must_use]
    pub fn focused(&self) -> Option<&Window> {
        let id = self.focus?;
        self.windows.iter().find(|j| j.id == id)
    }

    /// Every window the compositor knows about, in its own order.
    ///
    /// The buffer selector (`LEADER b b`) lists these: a "buffer" of this system
    /// IS a window of the compositor (invariant 4), so there is no second list
    /// to keep in step.
    #[must_use]
    pub fn all_windows(&self) -> &[Window] {
        &self.windows
    }

    /// How many windows the compositor has.
    #[must_use]
    pub fn amount(&self) -> usize {
        self.windows.len()
    }
}

/// The compositor's ACTION TABLE, extracted from the PINNED niri.
///
/// Generated by `nix/scripts/gen-niri-actions.py` and copied by `build.rs` — see
/// `nix/niri-actions.nix` for why it is generated.
mod table {
    include!(concat!(env!("OUT_DIR"), "/niri-actions.rs"));
}

/// The compositor's DEFAULT BINDS, from the pinned commit's `default-config.kdl`.
///
/// It is the authoritative list of the shortcuts niri ships — and not one of ours, which
/// would age silently with every release.
mod default_of {
    include!(concat!(env!("OUT_DIR"), "/binds-default.rs"));
}

pub use default_of::{DEFAULT_BINDS, DefaultBind};
pub use table::{NIRI_ACTIONS, NiriAction};

/// The niri binary, by path and not by name.
///
/// The footer runs as a user unit, and a unit's `PATH` is minimal:
/// a bare name would fail with "Node such file or directory" far from the cause. Outside
/// Nix it falls back to the name, and the shell's `PATH` resolves it.
const NIRI: &str = match option_env!("RONOCTUA_NIRI") {
    Some(c) => c,
    None => "niri",
};

/// Runs a compositor action by its kebab-case NAME.
///
/// # Why through niri's client, and not through our IPC
///
/// Our `Niri::action` sends `{"Name": {arguments}}` assembled here — and the
/// arguments are the problem. `Screenshot` requires `show_pointer` and
/// `MoveWindowToWorkspaceDown` requires `focus`, with defaults that exist in
/// **niri's clap** and nowhere we can read. Sending `{}` gives
/// `"error parsing request"`, which is the SAME error as a non-existent name.
///
/// For the 141 mirrored actions, the compositor's own client fills in
/// those defaults: `niri msg action <name> [argument]`. There is no conversion
/// of ours to get wrong, and the error that comes back is theirs, with their text.
///
/// The ~25 keymap commands still go through the direct IPC, which is the hot
/// path and is already proved, action by action, by `niri-contract`.
///
/// # Errors
///
/// The client cannot be run, or niri refused the action — and then their text
/// comes back whole, because it is the only clue.
pub fn run_through_client(name: &str, argument: Option<&str>) -> Result<()> {
    let mut cmd = std::process::Command::new(NIRI);
    cmd.args(["msg", "action", name]);
    if let Some(a) = argument {
        cmd.arg(a);
    }
    let output = cmd
        .output()
        .with_context(|| format!("nao consegui rodar `{NIRI} msg action {name}`"))?;
    if output.status.success() {
        return Ok(());
    }
    let reason = String::from_utf8_lossy(&output.stderr);
    let reason = reason.trim();
    bail!(
        "{}",
        if reason.is_empty() {
            format!("`{name}` falhou com {}", output.status)
        } else {
            reason.to_owned()
        }
    )
}

/// Does the action exist in this niri?
#[must_use]
pub fn action_exists(name: &str) -> bool {
    NIRI_ACTIONS.iter().any(|a| a.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_json(id: u64, focused: bool) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "title": "arq.rs",
            "app_id": "ghostty",
            "pid": 42,
            "workspace_id": 1,
            "is_focused": focused,
            "is_floating": false,
            "is_urgent": false,
            "layout": { "pos_in_scrolling_layout": null },
            "focus_timestamp": null
        })
    }

    #[test]
    fn an_unknown_event_is_not_an_error() {
        // niri ADDS variants with every release. If this became an error, the
        // distro would stop coming up on an upgrade over an event we do not use.
        let fresh = serde_json::json!({ "AlgoQueAindaNaoExiste": { "x": 1 } });
        assert_eq!(interpret(&fresh).unwrap(), Event::Ignored);
    }

    #[test]
    fn a_new_field_in_a_window_is_not_an_error() {
        // Same reason, one level down: `Window` ignores fields it does not
        // declare. Without that, a new field in `Window` would break everything.
        let mut j = window_json(1, true);
        j["campo_do_futuro"] = serde_json::json!("valor");
        let event = interpret(&serde_json::json!({ "WindowOpenedOrChanged": { "window": j } }));
        assert!(matches!(event.unwrap(), Event::WindowOpenedOrChanged(_)));
    }

    #[test]
    fn the_focus_comes_from_windows_changed() {
        // The initial `WindowsChanged` carries the focus embedded and may not come
        // accompanied by `WindowFocusChanged`. Reading only the focus event
        // would leave the modeline empty until the first Alt-Tab.
        let mut e = State::default();
        let raw = serde_json::json!({
            "WindowsChanged": { "windows": [window_json(1, false), window_json(7, true)] }
        });
        e.apply_to(&interpret(&raw).unwrap());
        assert_eq!(e.focused().map(|j| j.id), Some(7));
        assert_eq!(e.amount(), 2);
    }

    #[test]
    fn closing_the_focused_window_zeroes_the_focus() {
        let mut e = State::default();
        e.apply_to(
            &interpret(&serde_json::json!({
                "WindowsChanged": { "windows": [window_json(7, true)] }
            }))
            .unwrap(),
        );
        e.apply_to(&Event::WindowClosed(7));
        assert!(e.focused().is_none());
        assert_eq!(e.amount(), 0);
    }

    #[test]
    fn a_null_focus_is_valid() {
        // It really does happen: when the active leader takes the keyboard, focus
        // goes to our layer-shell and niri sends `id: null`. Treating that
        // as an error would take the shell down on its own main shortcut.
        let e = interpret(&serde_json::json!({ "WindowFocusChanged": { "id": null } }));
        assert_eq!(e.unwrap(), Event::FocusChanged(None));
    }

    #[test]
    fn a_window_that_opens_already_focused_gains_the_focus() {
        // niri promises "if the window is focused, all other windows are no
        // longer focused" IN THIS event. Waiting for a separate
        // `WindowFocusChanged` is depending on an order nobody promised — and that is
        // how the modeline ended up on `*scratch*` with a window open.
        let mut e = State::default();
        e.apply_to(
            &interpret(&serde_json::json!({
                "WindowsChanged": { "windows": [window_json(1, true)] }
            }))
            .unwrap(),
        );
        e.apply_to(
            &interpret(&serde_json::json!({
                "WindowOpenedOrChanged": { "window": window_json(2, true) }
            }))
            .unwrap(),
        );
        assert_eq!(e.focused().map(|j| j.id), Some(2));
        // And the previous one really did lose focus, not just in `self.focus`.
        assert!(!e.windows.iter().find(|j| j.id == 1).unwrap().is_focused);
    }

    #[test]
    fn opening_the_same_window_twice_does_not_duplicate() {
        // `WindowOpenedOrChanged` is also emitted for a CHANGE — the name says so.
        // Treating it as an insertion would fill the list with duplicates silently.
        let mut e = State::default();
        let open = interpret(&serde_json::json!({
            "WindowOpenedOrChanged": { "window": window_json(3, true) }
        }))
        .unwrap();
        e.apply_to(&open);
        e.apply_to(&open);
        assert_eq!(e.amount(), 1);
    }
}
