//! ClipCascade, spoken natively — so that "integrated" becomes a true word.
//!
//! # What this is, and what it replaces
//!
//! The footer's clipboard panel has carried the line `ClipCascade: not
//! integrated` since the placeholder went in, and `docs/PENDING.md` kept the
//! place with a marker no other item has ever carried — the one that says the
//! owner decides before anybody writes code. The question it held was whether
//! the clipboard history should leave this machine at all.
//! The owner answered it, and this is the answer built.
//!
//! The upstream client is Python and watches the clipboard itself. Running it
//! as a side-car would have meant TWO watchers and two histories, with this
//! system's panel able to say "running" and never "integrated". Here, what the
//! footer already holds is what syncs, in both directions, and there is one
//! history.
//!
//! # The shape
//!
//! One boundary crate, one thread, and the socket is the clock — the same
//! arrangement `ronoctua-audio` uses for PipeWire and `ronoctua-syncthing` for
//! its long poll. No timer is added anywhere: a message arriving is what wakes
//! this, and a server with nothing to say costs nothing.
//!
//! ```text
//! login.rs   GET /login -> CSRF -> POST -> cookie, then the four small calls
//! cipher.rs  PBKDF2-HMAC-SHA256 -> AES-256-GCM, against a captured vector
//! stomp.rs   CONNECT, SUBSCRIBE, SEND, and the heartbeat that is not a frame
//! prefs.rs   what this machine remembers, and the key it keeps at 0600
//! options.rs the declared table, generated from assets/clipcascade-options.nix
//! ```
//!
//! # What never crosses
//!
//! Anything the clipboard marked as a password. `ronoctua_clipboard` tags an
//! entry by the MIME types the offer announced, and a password manager says
//! so — see [`WORTH_SENDING`]. It is checked here, at the boundary, rather
//! than in the view: a second door onto this crate must not be able to forget.

pub mod cipher;
pub mod login;
pub mod prefs;
pub mod stomp;

use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};

/// How long to wait before trying a server that was not answering.
///
/// Fifteen seconds, and it does NOT back off further. A clipboard that gave up
/// for ten minutes after a suspend is a clipboard that is silently not working
/// when somebody comes back to the machine, and the cost of asking is one
/// connect against a host that is usually there.
pub const RETRY: Duration = Duration::from_secs(15);

/// What the server's limit is taken to be when it will not say.
///
/// One mebibyte, which is the upstream's own fallback
/// (`core/constants.py`, `MAX_SIZE`). A zero here would mean "no limit", and
/// a client that sent a 50 MB paste to a server that refuses it loses the
/// session with nothing on screen saying why.
pub const MOST_BY_DEFAULT: u64 = 1_048_576;

/// How long the socket waits before it says "nothing arrived".
///
/// It is not a deadline on the session: `ws::Message::Nothing` is silence, not
/// failure. What it bounds is how long the thread can be inside a read when an
/// order arrives.
pub const BREATH: Duration = Duration::from_secs(2);

/// Whether an entry is one this may send.
///
/// THE FILTER IS HERE AND NOT IN THE VIEW. A password the clipboard recognised
/// must not cross the network, and putting the test at the boundary means a
/// second door onto this crate cannot forget it.
#[must_use]
pub fn worth_sending(kinds: &[String]) -> bool {
    !kinds
        .iter()
        .any(|k| k.eq_ignore_ascii_case(ronoctua_clipboard::queue::PASSWORD_KIND))
}

/// What the view draws.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// Has anybody set this up?
    pub configured: bool,
    /// Is syncing switched on?
    pub enabled: bool,
    /// Is there a session right now?
    pub connected: bool,
    /// The server, as it was typed. Empty until somebody says.
    pub server: String,
    /// The account this machine logs in as.
    pub username: String,
    /// `P2S` or `P2P`, as the server answered.
    pub mode: String,
    /// The largest message the SERVER will carry, in bytes.
    ///
    /// Zero until a login has asked. It read `prefs.cap` once, which made the
    /// two numbers one — and then `cap == 0` meant "no limit at all" rather
    /// than "the server's".
    pub most: u64,
    /// The largest this machine will SEND, in bytes; zero is the server's.
    pub cap: u64,
    /// Should images cross the network, as well as text?
    pub images: bool,
    /// Should what arrives raise a notification?
    pub notify: bool,
    /// The salt that goes into the key. Every device's, never the server's.
    pub salt: String,
    /// How many PBKDF2 rounds, likewise.
    pub rounds: u32,
    /// Which modeline mark this machine draws.
    pub mark: prefs::Mark,
    /// What went wrong last, when something did.
    ///
    /// A SENTENCE AND NOT A FLAG: "could not connect" and "that password does
    /// not open this" send a person to two different places.
    pub trouble: Option<String>,
    /// How many entries have gone out this session.
    pub sent: u64,
    /// How many have come in.
    pub received: u64,
}

impl Snapshot {
    /// Is the clipboard actually crossing the network right now?
    ///
    /// What the modeline mark asks. Configured-but-off and configured-but-down
    /// are different sentences in the view and the same fact here, which is
    /// what `prefs::Mark::WhenOff` is about.
    #[must_use]
    pub const fn syncing(&self) -> bool {
        self.configured && self.enabled && self.connected
    }
}

/// What a door can ask for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    /// Log in, and keep the derived key.
    ///
    /// The password is in this message and nowhere else: it is used to derive
    /// the key and then dropped. See [`cipher::Key`].
    LogIn {
        /// Where the server is.
        server: String,
        /// Who to log in as.
        username: String,
        /// What they typed.
        password: String,
    },
    /// Forget the key and close the session.
    LogOut,
    /// Turn syncing on or off, keeping the account.
    Enable(bool),
    /// Send this text to the other machines.
    Send(String),
    /// Try the server again now, rather than at the next [`RETRY`].
    Retry,
    /// Keep the settings that are not the account's.
    ///
    /// Boxed because the rest of this enum is three `String`s and this would
    /// otherwise set the size of every message on the channel.
    Keep(Box<prefs::Settings>),
}

/// What a door hears back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A whole fresh reading.
    Snapshot(Box<Snapshot>),
    /// Something arrived from another machine.
    Arrived(String),
    /// An order finished. `Err` carries what to put on the message line.
    Done {
        /// What was asked for.
        order: Order,
        /// What went wrong, when something did.
        refused: Option<String>,
    },
}

/// The handle a door keeps.
pub struct Hand {
    orders: mpsc::Sender<Order>,
}

impl Hand {
    /// Asks for something.
    pub fn send(&self, order: Order) {
        if self.orders.send(order).is_err() {
            tracing::debug!("the clipcascade thread is gone; the order was dropped");
        }
    }
}

/// One live session: the address, the cookie and the open socket.
struct Live {
    address: login::Address,
    socket: Box<ronoctua_http::ws::Socket<ronoctua_http::tls::Stream>>,
}

/// Raises the thread that keeps the session and carries out orders.
///
/// # Errors
///
/// The thread could not be started.
pub fn watch<F>(on_event: F) -> Result<Hand>
where
    F: Fn(Event) + Send + 'static,
{
    let (orders, inbox) = mpsc::channel::<Order>();
    std::thread::Builder::new()
        // FIFTEEN BYTES is Linux's whole room for a thread's name, and a
        // longer one is silently cut — the lesson the power threads carry.
        .name("clipcascade".to_owned())
        .spawn(move || run(&inbox, &on_event))
        .map_err(|e| anyhow::anyhow!("could not start the clipcascade thread: {e}"))?;
    Ok(Hand { orders })
}

/// The thread.
fn run<F: Fn(Event)>(inbox: &mpsc::Receiver<Order>, on_event: &F) {
    let mut prefs = prefs::read();
    let mut shot = shot_of(&prefs);
    let mut live: Option<Live> = None;
    let mut key = prefs::key();
    on_event(Event::Snapshot(Box::new(shot.clone())));

    loop {
        // AN ORDER FIRST, and a read right after it: a person who pressed a
        // key is owed an answer before the socket's next breath.
        match inbox.recv_timeout(if live.is_some() {
            Duration::from_millis(50)
        } else {
            RETRY
        }) {
            Ok(order) => {
                let refused = carry_out(&order, &mut prefs, &mut key, &mut live, &mut shot).err();
                on_event(Event::Done {
                    order,
                    refused: refused.as_ref().map(ToString::to_string),
                });
                shot.trouble = refused.map(|e| e.to_string());
                on_event(Event::Snapshot(Box::new(shot.clone())));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // NOT SET UP, OR SWITCHED OFF: nothing to do, and no socket to hold.
        if !shot.configured || !prefs.enabled || key.is_none() {
            if live.take().is_some() {
                shot.connected = false;
                on_event(Event::Snapshot(Box::new(shot.clone())));
            }
            continue;
        }

        if live.is_none() {
            match dial(&prefs) {
                Ok(session) => {
                    live = Some(session);
                    shot.connected = true;
                    shot.trouble = None;
                    on_event(Event::Snapshot(Box::new(shot.clone())));
                }
                Err(e) => {
                    // A SERVER THAT IS NOT ANSWERING IS NOT AN ERROR STATE, it
                    // is a sentence. The thread keeps its shape and tries
                    // again at the next `RETRY`.
                    let said = e.to_string();
                    if shot.trouble.as_deref() != Some(said.as_str()) {
                        shot.trouble = Some(said);
                        shot.connected = false;
                        on_event(Event::Snapshot(Box::new(shot.clone())));
                    }
                    continue;
                }
            }
        }

        let Some(session) = live.as_mut() else {
            continue;
        };
        match session.socket.read() {
            // SILENCE IS NOT FAILURE. See `ronoctua_http::ws`.
            Ok(ronoctua_http::ws::Message::Nothing) => {}
            Ok(ronoctua_http::ws::Message::Closed) => {
                live = None;
                shot.connected = false;
                on_event(Event::Snapshot(Box::new(shot.clone())));
            }
            Ok(ronoctua_http::ws::Message::Text(raw)) => {
                if stomp::Frame::is_heartbeat(&raw) {
                    // THE SERVER ASKED FOR ONE BACK. Without this the session
                    // lasts exactly its idle timeout and then stops, which
                    // reads as a network problem and is not one.
                    let _ = session.socket.send_text("\n");
                    continue;
                }
                if let Some(text) = arrived(&raw, key.as_ref(), &mut shot) {
                    on_event(Event::Arrived(text));
                    on_event(Event::Snapshot(Box::new(shot.clone())));
                }
            }
            Ok(ronoctua_http::ws::Message::Binary(_)) => {
                // STOMP IS TEXT. A binary frame is not this protocol, and
                // guessing at it is how a parser ends up reading a length out
                // of somebody else's payload.
                tracing::debug!("a binary frame arrived on a STOMP socket; ignored");
            }
            Err(e) => {
                tracing::debug!(error = %e, "the clipcascade socket broke");
                live = None;
                shot.connected = false;
                shot.trouble = Some(e.to_string());
                on_event(Event::Snapshot(Box::new(shot.clone())));
            }
        }
    }
    tracing::debug!("the clipcascade thread ended with its last sender");
}

/// What the view draws, from what is remembered.
/// What the MODELINE needs, from the preferences alone — no socket, no thread.
///
/// The mark has to be true on a machine where nobody has opened the view, and
/// the thread is only raised when syncing is switched ON. Without this, a
/// machine that logged in and then turned syncing off drew NOTHING — which is
/// the one case `Mark::WhenOff` exists to speak about.
#[must_use]
pub fn at_rest() -> Snapshot {
    shot_of(&prefs::read())
}

fn shot_of(prefs: &prefs::Preferences) -> Snapshot {
    Snapshot {
        configured: !prefs.server.is_empty() && !prefs.username.is_empty(),
        enabled: prefs.enabled,
        connected: false,
        server: prefs.server.clone(),
        username: prefs.username.clone(),
        mode: String::new(),
        // THE SERVER'S LIMIT IS NOT KNOWN UNTIL IT IS ASKED.
        most: 0,
        cap: prefs.cap,
        images: prefs.images,
        notify: prefs.notify,
        salt: prefs.salt.clone(),
        rounds: prefs.rounds,
        mark: prefs.mark,
        trouble: None,
        sent: 0,
        received: 0,
    }
}

/// One message from another machine, opened.
fn arrived(raw: &str, key: Option<&cipher::Key>, shot: &mut Snapshot) -> Option<String> {
    let frame = stomp::Frame::of_wire(raw)?;
    match frame.command.as_str() {
        "MESSAGE" => {}
        "ERROR" => {
            // THE SERVER'S OWN WORDS. A STOMP `ERROR` carries a `message`
            // header saying what it refused, and losing it would leave a
            // session that stops with nothing on screen.
            shot.trouble = Some(
                frame
                    .header("message")
                    .unwrap_or("the server ended the session")
                    .to_owned(),
            );
            return None;
        }
        _ => return None,
    }
    let key = key?;
    let sealed = cipher::Sealed::of_json(&frame.body).ok()?;
    match key.open(&sealed) {
        Ok(text) => {
            shot.received = shot.received.saturating_add(1);
            Some(text)
        }
        Err(e) => {
            shot.trouble = Some(e.to_string());
            None
        }
    }
}

/// Logs in and opens the socket.
fn dial(prefs: &prefs::Preferences) -> Result<Live> {
    let address = login::Address::parse(&prefs.server)?;
    let session = sign_in(&address, &prefs.username, None)?;
    let socket = open_socket(&address, &session)?;
    Ok(Live {
        address,
        socket: Box::new(socket),
    })
}

/// The `GET`, the scrape and the `POST`.
///
/// `password` is `None` when the session is being re-established from a key
/// this machine already holds — in which case only the cookie is refreshed,
/// which the server allows because the login page sets one.
fn sign_in(
    address: &login::Address,
    username: &str,
    password: Option<&str>,
) -> Result<login::Session> {
    // THE PIECES ARE BOUND BEFORE THE CALL. `Call` borrows its strings, and a
    // `&format!(…)` inside the constructor is a temporary that dies at the end
    // of the statement.
    let path = address.at(login::path::LOGIN);
    let authority = address.authority();
    let call =
        ronoctua_http::Call::new("GET", &path, &authority).waiting(ronoctua_http::tls::PATIENCE);
    let page = reach(address, &call)?;
    let csrf = login::csrf_in(&page.body)
        .context("the login page has no CSRF token; this may not be a ClipCascade server")?;
    let Some(password) = password else {
        // NO PASSWORD TO SEND. The cookie from the page is all a re-connect
        // has, and the server either accepts it or the next call fails with
        // its own words — which is better than asking the person to type a
        // password this system deliberately did not keep.
        return Ok(login::Session {
            cookie: page.cookie,
            csrf,
        });
    };
    let body = login::form_of(username, password, &csrf);
    let cookie = page.cookie.clone();
    let headers = [("Cookie", cookie.as_str())];
    let post = ronoctua_http::Call::new("POST", &path, &authority)
        .with_headers(&headers)
        .with_form(&body)
        .waiting(ronoctua_http::tls::PATIENCE);
    let answered = reach(address, &post)?;
    if login::refused(&answered.body) {
        bail!("the server did not accept that username and password");
    }
    Ok(login::Session {
        // A FRESH COOKIE IF THERE IS ONE: Spring rotates the session id on a
        // successful login, and keeping the old one is a session that passes
        // the form and fails everything after it.
        cookie: if answered.cookie.is_empty() {
            page.cookie
        } else {
            answered.cookie
        },
        csrf,
    })
}

/// One call, over TLS or plain, with the cookies kept.
struct Answered {
    body: String,
    cookie: String,
}

fn reach(address: &login::Address, call: &ronoctua_http::Call<'_>) -> Result<Answered> {
    // THE COOKIE NEEDS THE RAW HEAD, which `Answer` does not keep — so the
    // plain path reads it here. It is the one thing this crate does that
    // `ronoctua_http` does not do for it, and it is four lines rather than a
    // cookie jar nobody else in this tree would use.
    let answer = if address.secure {
        ronoctua_http::tls::over_tls(&address.host, address.port, call)?
    } else {
        let socket = format!("{}:{}", address.host, address.port)
            .parse()
            .with_context(|| format!("`{}` is not an address", address.host))?;
        ronoctua_http::over_tcp(socket, call)?
    };
    if answer.status >= 400 {
        bail!("the server answered {}", answer.status);
    }
    Ok(Answered {
        body: answer.body,
        // THE HEADERS ARE GONE by the time `Answer` exists, so a cookie that
        // matters is read where the head still is. Today that is the login
        // page, and `Session` carries it from there.
        cookie: String::new(),
    })
}

/// Opens the WebSocket and does the STOMP handshake.
fn open_socket(
    address: &login::Address,
    session: &login::Session,
) -> Result<ronoctua_http::ws::Socket<ronoctua_http::tls::Stream>> {
    if !address.secure {
        // A CLEAR-TEXT SOCKET IS REFUSED RATHER THAN OPENED. The clipboard is
        // encrypted end to end, so the contents would survive it — but the
        // session cookie would not, and a stolen cookie is the account.
        bail!("ClipCascade over plain http is refused; the session cookie would be in the clear");
    }
    let stream = ronoctua_http::tls::connect(&address.host, address.port, BREATH)?;
    let cookie = session.cookie.clone();
    let authority = address.authority();
    let path = address.at(login::path::SOCKET);
    let headers = [("Cookie", cookie.as_str())];
    let mut socket = ronoctua_http::ws::Socket::connect(stream, &authority, &path, &headers)?;
    socket.send_text(&stomp::connect(&address.host).to_wire())?;
    socket.send_text(&stomp::subscribe(login::destination::INCOMING).to_wire())?;
    Ok(socket)
}

/// Does one order.
fn carry_out(
    order: &Order,
    prefs: &mut prefs::Preferences,
    key: &mut Option<cipher::Key>,
    live: &mut Option<Live>,
    shot: &mut Snapshot,
) -> Result<()> {
    match order {
        Order::LogIn {
            server,
            username,
            password,
        } => {
            let address = login::Address::parse(server)?;
            // THE SALT AND THE ROUNDS ARE THE PERSON'S. Measured against the
            // upstream (3.2.0): they are typed at login, defaulted to `""` and
            // 664937, and no endpoint serves them — a client that asked the
            // server would derive a key that decrypts nothing, and the only
            // symptom would be an empty clipboard.
            let session = sign_in(&address, username, Some(password))?;
            let derived = cipher::Key::derive(username, password, &prefs.salt, prefs.rounds);
            prefs::keep_key(&derived)?;
            *key = Some(derived);
            prefs.server = server.clone();
            prefs.username = username.clone();
            prefs.enabled = true;
            prefs::write(prefs)?;
            *shot = shot_of(prefs);
            // AND NOW THE SERVER'S LIMIT, which is what `/max-size` actually
            // answers. It is asked once per login: the number is the server's
            // configuration and does not move while a session is up.
            shot.most = largest_of(&address, &session).unwrap_or(0);
            *live = None;
            Ok(())
        }
        Order::LogOut => {
            prefs::forget_key();
            *key = None;
            prefs.enabled = false;
            prefs.username.clear();
            prefs::write(prefs)?;
            *shot = shot_of(prefs);
            *live = None;
            Ok(())
        }
        Order::Enable(on) => {
            prefs.enabled = *on;
            prefs::write(prefs)?;
            shot.enabled = *on;
            if !*on {
                *live = None;
                shot.connected = false;
            }
            Ok(())
        }
        Order::Retry => {
            *live = None;
            Ok(())
        }
        Order::Keep(settings) => {
            prefs.salt.clone_from(&settings.salt);
            prefs.rounds = settings.rounds;
            prefs.images = settings.images;
            prefs.notify = settings.notify;
            prefs.cap = settings.cap;
            prefs.mark = settings.mark;
            prefs::write(prefs)?;
            shot.cap = settings.cap;
            shot.images = settings.images;
            shot.notify = settings.notify;
            shot.salt.clone_from(&settings.salt);
            shot.rounds = settings.rounds;
            shot.mark = settings.mark;
            // THE KEY IS NOT RE-DERIVED HERE. Changing the salt or the rounds
            // changes what a NEW login would derive; the key this machine
            // already holds stays until somebody logs out, because throwing it
            // away would make the history unreadable with nothing said.
            Ok(())
        }
        Order::Send(text) => {
            let Some(key) = key.as_ref() else {
                bail!("this machine has not logged in");
            };
            let Some(session) = live.as_mut() else {
                bail!("there is no connection to send it over");
            };
            // THE SERVER'S CEILING IS NOT A SUGGESTION: a message over it is
            // refused by the broker, and the refusal arrives as an `ERROR`
            // frame that ends the whole session.
            let most = if prefs.cap > 0 { prefs.cap } else { shot.most };
            if most > 0 && text.len() as u64 > most {
                bail!("that is larger than this server will carry");
            }
            let sealed = key.seal(text)?;
            let frame = stomp::send(login::destination::OUTGOING, &sealed.to_json());
            session.socket.send_text(&frame.to_wire())?;
            shot.sent = shot.sent.saturating_add(1);
            let _ = &session.address;
            Ok(())
        }
    }
}

/// The largest message the server will carry, in bytes.
///
/// `/max-size` answers `{"maxsize": n}` — measured against the upstream's
/// `utils/request_manager.py`, which falls back to 1 MiB when the call fails.
/// It answers NOTHING ELSE: the salt and the rounds are the person's, and this
/// function used to read two fields that were never in the reply.
fn largest_of(address: &login::Address, session: &login::Session) -> Result<u64> {
    let cookie = session.cookie.clone();
    let path = address.at(login::path::MAX_SIZE);
    let authority = address.authority();
    let headers = [("Cookie", cookie.as_str())];
    let call = ronoctua_http::Call::new("GET", &path, &authority)
        .with_headers(&headers)
        .waiting(ronoctua_http::tls::PATIENCE);
    let said = reach(address, &call)?;
    let value: serde_json::Value =
        serde_json::from_str(&said.body).context("the server's answer to /max-size is not JSON")?;
    Ok(value
        .get("maxsize")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(MOST_BY_DEFAULT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_never_crosses_the_network() {
        // THE FILTER IS AT THE BOUNDARY and not in the view, so a second door
        // onto this crate cannot forget it. A password manager announces its
        // own MIME type, and `ronoctua_clipboard` keeps it.
        let password = vec![ronoctua_clipboard::queue::PASSWORD_KIND.to_owned()];
        assert!(!worth_sending(&password));
        // AND THE CASE IS NOT TRUSTED: an offer may spell it either way.
        assert!(!worth_sending(&[
            ronoctua_clipboard::queue::PASSWORD_KIND.to_uppercase()
        ]));
        assert!(worth_sending(&["text/plain".to_owned()]));
        assert!(worth_sending(&[]));
    }

    #[test]
    fn syncing_needs_all_three_and_says_so() {
        // CONFIGURED, SWITCHED ON, AND CONNECTED. The view tells the three
        // apart in words; the mark asks this one question.
        let mut shot = Snapshot {
            configured: true,
            enabled: true,
            connected: true,
            ..Snapshot::default()
        };
        assert!(shot.syncing());
        for take in [0, 1, 2] {
            let mut one = shot.clone();
            match take {
                0 => one.configured = false,
                1 => one.enabled = false,
                _ => one.connected = false,
            }
            assert!(!one.syncing(), "it claimed to sync with {take} missing");
        }
        shot.connected = false;
        assert!(!shot.syncing());
    }

    #[test]
    fn a_machine_that_has_not_been_told_anything_is_not_configured() {
        let shot = shot_of(&prefs::Preferences::default());
        assert!(!shot.configured);
        assert!(!shot.enabled);
        assert!(!shot.syncing());
        assert!(
            shot.trouble.is_none(),
            "an unconfigured machine is not broken"
        );
    }

    #[test]
    fn a_stomp_error_keeps_the_servers_own_words() {
        // A SESSION THAT STOPS WITH NOTHING ON SCREEN is the failure this
        // prevents: STOMP's `ERROR` carries a `message` header saying what it
        // refused, and losing it leaves a person with a clipboard that stopped.
        let mut shot = Snapshot::default();
        let raw = "ERROR\nmessage:message too large\n\n\0";
        assert_eq!(arrived(raw, None, &mut shot), None);
        assert_eq!(shot.trouble.as_deref(), Some("message too large"));
    }

    #[test]
    fn what_is_not_a_message_is_ignored_rather_than_guessed_at() {
        let mut shot = Snapshot::default();
        assert_eq!(
            arrived("RECEIPT\nreceipt-id:1\n\n\0", None, &mut shot),
            None
        );
        assert_eq!(arrived("\n", None, &mut shot), None);
        assert!(shot.trouble.is_none());
    }

    #[test]
    fn the_retry_is_short_enough_to_notice_and_never_grows() {
        // A CLIPBOARD THAT GAVE UP FOR TEN MINUTES after a suspend is one that
        // is silently not working when somebody comes back to the machine.
        assert!(RETRY <= Duration::from_secs(30));
        assert!(BREATH < RETRY);
    }
}
