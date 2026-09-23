//! The ADAPTER — the only crate in this tree that speaks a compositor's
//! protocol, and the only one that knows which compositor it is.
//!
//! Everything above it holds a `dyn ronoctua_compositor::Compositor` and has
//! no idea what is underneath. What keeps that true is
//! `only-the-composition-root-names-the-compositor`, which refuses a mention
//! of this crate outside the three `main`s that are allowed to choose one.
//!
//! # What the wire is
//!
//! i3's IPC: a six-byte magic, a length, a type, and JSON. Commands are FREE
//! TEXT rather than enumerated actions, which is why [`wire::spoken`]
//! validates an argument instead of interpolating it.
//!
//! # The promise this crate keeps that the wire does not
//!
//! The contract says the complete window state arrives first
//! ([`Event::Windows`]), because everything above it is written against that.
//! The wire makes no such promise: it sends increments, and a reading is a
//! separate request. [`Watching`] does both on one socket and hands the
//! reading out first — see there for why the ordering is safe.

#![forbid(unsafe_code)]

pub mod deeds;
pub mod read;
pub mod wire;

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use ronoctua_compositor::{
    Ask, Compositor, Deed, Device, Event, Events, FocusFollows, Output, Seat, SeatAsk, Setting,
    Window,
};
use serde_json::Value;

/// The variable the compositor exports with its socket's path.
///
/// It exports two more with the same value — the names sway and i3 use — and
/// reading those as a fallback is what lets a session started by hand, or an
/// older wrapper, still be found. The order is the compositor's own client's.
pub const SOCKET_ENV: &str = "SCROLLSOCK";
/// The name the wider ecosystem looks for, carrying the same path.
pub const SOCKET_ENV_COMPATIBLE: &str = "SWAYSOCK";

/// A connection to the compositor.
#[derive(Debug)]
pub struct Scroll {
    stream: UnixStream,
}

impl Scroll {
    /// Opens the socket the environment names.
    ///
    /// # Errors
    ///
    /// When neither variable is set, or the socket refuses the connection —
    /// both of which mean "there is no compositor here", which is an ordinary
    /// state and not a bug. See [`ronoctua_compositor::Absent`].
    pub fn connect() -> Result<Self> {
        let path = std::env::var_os(SOCKET_ENV)
            .or_else(|| std::env::var_os(SOCKET_ENV_COMPATIBLE))
            .map(PathBuf::from)
            .with_context(|| {
                format!(
                    "neither {SOCKET_ENV} nor {SOCKET_ENV_COMPATIBLE} is set.\n\
                     The compositor sets both for the processes IT starts.\n\
                     Outside a session, point one at the socket by hand."
                )
            })?;
        Self::connect_at(&path)
    }

    /// Opens a named socket. The contract test needs this: it is given one.
    ///
    /// # Errors
    ///
    /// When the socket cannot be reached.
    pub fn connect_at(path: &std::path::Path) -> Result<Self> {
        let stream = UnixStream::connect(path)
            .with_context(|| format!("connecting to {}", path.display()))?;
        Ok(Self { stream })
    }

    /// The composition root's [`ronoctua_compositor::Opener`].
    ///
    /// # Errors
    ///
    /// When there is no compositor to open.
    pub fn open() -> Result<Box<dyn Compositor>> {
        Ok(Box::new(Self::connect()?))
    }

    /// One request, one reply, parsed.
    fn request(&mut self, kind: wire::Kind, payload: &str) -> Result<Value> {
        wire::send(&mut self.stream, kind, payload)?;
        let (_kind, body) = wire::receive(&mut self.stream)?;
        serde_json::from_str(&body).context("a reply that is not JSON")
    }

    /// Runs commands, and turns a refusal into an error carrying the
    /// compositor's OWN words.
    ///
    /// The reply is an array with one entry per command, because a caller may
    /// send several separated by `;`. Any one of them failing fails the lot,
    /// which is right: a caller that sent two wanted both.
    /// Runs free text, and this is NOT on the contract.
    ///
    /// # Why it is public here and nowhere else
    ///
    /// The contract deliberately has no "run this string" method: the moment
    /// one exists, every caller can send `focus left` and the boundary is
    /// decoration. What needs it is the CONTRACT TEST, which is part of this
    /// crate and whose whole job is to put arbitrary commands in front of a
    /// live compositor — including two (`create_output`, `output … unplug`)
    /// that exist so the test can build itself an output to experiment on.
    ///
    /// `only-the-composition-root-names-the-compositor` is what keeps that
    /// true: nothing outside this crate may name it, so nothing outside can
    /// reach this.
    ///
    /// # Errors
    ///
    /// When the compositor refuses, carrying its own words.
    pub fn run_text(&mut self, command: &str) -> Result<()> {
        self.run(command)
    }

    fn run(&mut self, command: &str) -> Result<()> {
        let said = self.request(wire::Kind::Command, command)?;
        let Some(results) = said.as_array() else {
            bail!("the compositor answered a command with something else");
        };
        for one in results {
            if one.get("success").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            // ITS OWN WORDS, and not a sentence of ours about them: the
            // compositor knows whether the command was unknown or merely
            // matched nothing, and a person reading the minibuffer wants
            // whichever it was.
            let why = one
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("the compositor refused it and did not say why");
            bail!("{why}");
        }
        Ok(())
    }
}

impl Compositor for Scroll {
    fn version(&mut self) -> Result<String> {
        let said = self.request(wire::Kind::Version, "")?;
        Ok(said
            .get("human_readable")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned())
    }

    fn windows(&mut self) -> Result<Vec<Window>> {
        Ok(read::windows_in(&self.request(wire::Kind::Tree, "")?))
    }

    fn focused_window(&mut self) -> Result<Option<Window>> {
        Ok(self.windows()?.into_iter().find(|w| w.is_focused))
    }

    fn outputs(&mut self) -> Result<BTreeMap<String, Output>> {
        Ok(read::outputs_in(&self.request(wire::Kind::Outputs, "")?))
    }

    fn focused_output(&mut self) -> Result<Option<Output>> {
        let said = self.request(wire::Kind::Outputs, "")?;
        let Some(at) = read::focused_in(&said) else {
            return Ok(None);
        };
        Ok(read::outputs_in(&said).remove(&at))
    }

    fn ask(&mut self, at: &str, ask: Ask) -> Result<()> {
        let Some(command) = wire::command_of(at, ask) else {
            bail!("there is no command for {ask:?}; it goes through the file");
        };
        self.run(&command)
    }

    fn deed(&mut self, deed: &Deed, argument: Option<&str>) -> Result<()> {
        let Some(spoken) = deeds::spoken_of(deed.id) else {
            bail!("`{}` is not a deed this compositor was given", deed.id);
        };
        let command = wire::spoken(deed, spoken.text, argument)?;
        self.run(&command)
    }

    fn reread(&mut self) -> Result<()> {
        // IT DROPS EVERY TRANSIENT `ask` MADE SINCE THE LAST READ, which is
        // the whole of undo: `load_main_config` frees `config->output_configs`
        // and builds it again from the files.
        //
        // AND IT ANSWERS `success` WHATEVER PARSED, which is worth writing
        // down here because the opposite was believed and written into three
        // files. Measured against a live compositor: a broken fragment and a
        // broken MAIN file both came back `{"success": true}`. The reason is
        // `load_config`'s own return —
        // `config->active || !config->validating || config_load_success` —
        // where `config->active` is true for every reload of a running
        // session, so the result never depends on whether anything parsed.
        //
        // So an `Ok` here means the compositor took the request, and nothing
        // more. What proves the grammar is the build-time gate
        // (`display-fragment-validates`); what proves a change happened is the
        // reading that follows it.
        self.run("reload")
    }

    fn devices(&mut self) -> Result<Vec<Device>> {
        Ok(read::devices_in(&self.request(wire::Kind::Inputs, "")?))
    }

    fn seats(&mut self) -> Result<Vec<Seat>> {
        Ok(read::seats_in(&self.request(wire::Kind::Seats, "")?))
    }

    fn ask_input(&mut self, device: &str, setting: Setting) -> Result<()> {
        let command = wire::input_command(device, setting)?;
        self.run(&command)
    }

    fn ask_seat(&mut self, seat: &str, ask: SeatAsk) -> Result<()> {
        let command = wire::seat_command(seat, &ask)?;
        self.run(&command)
    }

    fn ask_focus(&mut self, follows: FocusFollows) -> Result<()> {
        self.run(wire::focus_command(follows))
    }

    fn close_window(&mut self) -> Result<()> {
        self.run("kill")
    }

    fn focus_window(&mut self, id: u64) -> Result<()> {
        self.run(&format!("[con_id={id}] focus"))
    }

    fn leave(&mut self) -> Result<()> {
        self.run("exit")
    }

    fn watch(self: Box<Self>) -> Result<Box<dyn Events>> {
        Ok(Box::new(Watching::start(self.stream)?))
    }
}

/// The event stream.
///
/// # Why it asks for the tree before it reads anything
///
/// The contract promises that the complete window state is the first event.
/// The wire does not: it sends increments. So this subscribes FIRST, asks for
/// the tree SECOND, and hands the tree out before anything else — buffering
/// any events that arrive in between.
///
/// Replaying those buffered events on top of the tree is safe, and that is
/// worth saying rather than hoping: the tree was computed by the server after
/// those events happened, so each one is either already in it (and the upsert
/// is a no-op) or names a window that has gone (and the removal is a no-op).
/// Subscribing second instead would leave a gap between the reading and the
/// subscription, with no way to know what fell in it.
#[derive(Debug)]
pub struct Watching {
    stream: UnixStream,
    /// What arrived before the reading did, plus the reading itself.
    waiting: std::collections::VecDeque<Event>,
    /// Whether a read deadline is currently set on the descriptor.
    deadline: bool,
}

impl Watching {
    fn start(mut stream: UnixStream) -> Result<Self> {
        wire::send(
            &mut stream,
            wire::Kind::Subscribe,
            // WINDOW, OUTPUT, SHUTDOWN, INPUT AND MODE, AND NOTHING ELSE.
            // Every other subscription is a wake and a repaint for something
            // no part of this shell reads. `mode` is how the footer knows a
            // key mode is on, which nothing else on screen says.
            r#"["window","output","shutdown","input","mode"]"#,
        )?;
        let mut waiting = std::collections::VecDeque::new();
        // The subscribe's own reply, and anything that raced it.
        let mut settled = false;
        while !settled {
            let (kind, body) = wire::receive(&mut stream)?;
            if kind & wire::EVENT_BIT == 0 {
                settled = true;
            } else if let Some(e) = event_of(kind, &body) {
                waiting.push_back(e);
            }
        }
        wire::send(&mut stream, wire::Kind::Tree, "")?;
        let mut before = Vec::new();
        let tree = loop {
            let (kind, body) = wire::receive(&mut stream)?;
            if kind & wire::EVENT_BIT == 0 {
                break serde_json::from_str::<Value>(&body).context("a tree that is not JSON")?;
            }
            if let Some(e) = event_of(kind, &body) {
                before.push(e);
            }
        };
        // The reading first, then whatever raced it.
        let mut all = std::collections::VecDeque::new();
        all.push_back(Event::Windows(read::windows_in(&tree)));
        all.extend(waiting);
        all.extend(before);
        Ok(Self {
            stream,
            waiting: all,
            deadline: false,
        })
    }

    /// Reads until something interesting arrives.
    fn read_one(&mut self) -> Result<Option<Event>> {
        loop {
            if let Some(e) = self.waiting.pop_front() {
                return Ok(Some(e));
            }
            let (kind, body) = match wire::receive(&mut self.stream) {
                Ok(got) => got,
                Err(e) => {
                    // A stream that went quiet is not a stream that broke.
                    if let Some(io) = e.downcast_ref::<std::io::Error>()
                        && matches!(io.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
                    {
                        return Ok(None);
                    }
                    if let Some(io) = e.downcast_ref::<std::io::Error>()
                        && io.kind() == ErrorKind::UnexpectedEof
                    {
                        return Ok(None);
                    }
                    return Err(e);
                }
            };
            if let Some(e) = event_of(kind, &body) {
                return Ok(Some(e));
            }
        }
    }
}

impl Events for Watching {
    fn next_one(&mut self) -> Result<Option<Event>> {
        // THE DEADLINE LIVES ON THE DESCRIPTOR, so it outlives the call that
        // set it. Clearing it here is what stops a patient read from making
        // every later read impatient.
        if self.deadline {
            self.stream.set_read_timeout(None)?;
            self.deadline = false;
        }
        self.read_one()
    }

    fn next_within(&mut self, patience: Duration) -> Result<Option<Event>> {
        self.stream.set_read_timeout(Some(patience))?;
        self.deadline = true;
        self.read_one()
    }
}

/// One event, if it is one we act on.
fn event_of(kind: u32, body: &str) -> Option<Event> {
    let said: Value = serde_json::from_str(body).ok()?;
    match kind {
        wire::EVENT_OUTPUT => Some(Event::ScreensChanged),
        wire::EVENT_SHUTDOWN => Some(Event::Leaving),
        // `default` is the absence of a mode, and is said as one.
        wire::EVENT_MODE => {
            let name = said.get("change").and_then(Value::as_str)?;
            Some(Event::ModeChanged(
                (name != "default").then(|| name.to_owned()),
            ))
        }
        // ADDED, REMOVED OR RECONFIGURED, and all three mean the same thing
        // to a reader: read the devices again. The keymap changes that ride
        // the same event are a keyboard's business, and a keyboard is not
        // something this reading configures.
        wire::EVENT_INPUT => match said.get("change").and_then(Value::as_str) {
            Some("added" | "removed" | "libinput_config") => Some(Event::DevicesChanged),
            _ => Some(Event::Ignored),
        },
        wire::EVENT_WINDOW => {
            let change = said.get("change").and_then(Value::as_str)?;
            let container = said.get("container")?;
            match change {
                // A CLOSE CARRIES THE CONTAINER IT CLOSED, so the id comes
                // from the same place as every other one.
                "close" => container
                    .get("id")
                    .and_then(Value::as_u64)
                    .map(Event::WindowClosed),
                "focus" => Some(Event::FocusChanged(
                    container.get("id").and_then(Value::as_u64),
                )),
                // `new`, `title`, `move`, `floating`, `fullscreen_mode`,
                // `urgent`, `mark` and whatever is added next all say the
                // same thing: this window is now like this.
                _ => read::windows_in(container)
                    .into_iter()
                    .next()
                    .map(Event::WindowChanged),
            }
        }
        // An event we did not subscribe to, or one added after this was
        // written. Neither is an error — see the module's header.
        _ => Some(Event::Ignored),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn a_message_goes_out_the_way_the_protocol_says() {
        let (mut ours, mut theirs) = UnixStream::pair().unwrap();
        wire::send(&mut ours, wire::Kind::Command, "focus left").unwrap();
        let (kind, body) = wire::receive(&mut theirs).unwrap();
        assert_eq!(kind, wire::Kind::Command as u32);
        assert_eq!(body, "focus left");
    }

    #[test]
    fn a_reply_that_is_not_ours_is_refused_rather_than_read() {
        // A header without the magic means the socket is not what we think
        // it is, and reading on would turn somebody else's bytes into
        // windows.
        let (mut ours, mut theirs) = UnixStream::pair().unwrap();
        theirs.write_all(b"not-ipcxxxxxxxx").unwrap();
        assert!(wire::receive(&mut ours).is_err());
    }

    #[test]
    fn a_refusal_carries_the_compositors_own_words() {
        let said: Value = serde_json::from_str(
            r#"[{"success":false,"error":"Unknown/invalid command 'nope'","parse_error":true}]"#,
        )
        .unwrap();
        // The shape `run` reads, checked without a socket.
        let one = &said.as_array().unwrap()[0];
        assert_eq!(one.get("success").and_then(Value::as_bool), Some(false));
        assert_eq!(
            one.get("error").and_then(Value::as_str),
            Some("Unknown/invalid command 'nope'")
        );
    }

    #[test]
    fn a_window_event_becomes_the_change_it_describes() {
        let closed = event_of(
            wire::EVENT_WINDOW,
            r#"{"change":"close","container":{"id":7,"pid":1,"name":"x"}}"#,
        );
        assert_eq!(closed, Some(Event::WindowClosed(7)));

        let focused = event_of(
            wire::EVENT_WINDOW,
            r#"{"change":"focus","container":{"id":9,"pid":1,"name":"x"}}"#,
        );
        assert_eq!(focused, Some(Event::FocusChanged(Some(9))));

        let titled = event_of(
            wire::EVENT_WINDOW,
            r#"{"change":"title","container":{"id":9,"pid":22,"name":"new title","focused":true}}"#,
        );
        match titled {
            Some(Event::WindowChanged(w)) => {
                assert_eq!(w.id, 9);
                assert_eq!(w.title.as_deref(), Some("new title"));
            }
            other => panic!("expected a change, got {other:?}"),
        }
    }

    #[test]
    fn an_event_nobody_wrote_a_branch_for_is_ignored() {
        // A compositor adds events. Breaking the desktop over one nobody
        // reads is the failure this arm exists to prevent.
        assert_eq!(event_of(wire::EVENT_BIT | 99, "{}"), Some(Event::Ignored));
        assert_eq!(
            event_of(
                wire::EVENT_MODE,
                r#"{"change":"resize","pango_markup":false}"#
            ),
            Some(Event::ModeChanged(Some("resize".to_owned())))
        );
        assert_eq!(
            event_of(
                wire::EVENT_MODE,
                r#"{"change":"default","pango_markup":false}"#
            ),
            Some(Event::ModeChanged(None))
        );
        // And a change we have no branch for still says "this window is now
        // like this", which is true of every one of them.
        let odd = event_of(
            wire::EVENT_WINDOW,
            r#"{"change":"something_new","container":{"id":1,"pid":2,"name":"n"}}"#,
        );
        assert!(matches!(odd, Some(Event::WindowChanged(_))));
    }

    #[test]
    fn the_outputs_moving_is_one_event_with_nothing_in_it() {
        // It says only THAT something moved, so the answer is to read again.
        assert_eq!(
            event_of(wire::EVENT_OUTPUT, r#"{"change":"unspecified"}"#),
            Some(Event::ScreensChanged)
        );
    }

    #[test]
    fn a_device_coming_or_going_is_one_event_with_nothing_in_it() {
        for change in ["added", "removed", "libinput_config"] {
            assert_eq!(
                event_of(
                    wire::EVENT_INPUT,
                    &format!(r#"{{"change":"{change}","input":{{}}}}"#)
                ),
                Some(Event::DevicesChanged),
                "{change}"
            );
        }
        assert_eq!(
            event_of(wire::EVENT_INPUT, r#"{"change":"xkb_layout","input":{}}"#),
            Some(Event::Ignored),
            "a keyboard's layout is not a pointer's business"
        );
    }

    #[test]
    fn the_session_ending_is_said_before_the_socket_dies() {
        assert_eq!(
            event_of(wire::EVENT_SHUTDOWN, r#"{"change":"exit"}"#),
            Some(Event::Leaving)
        );
    }
}
