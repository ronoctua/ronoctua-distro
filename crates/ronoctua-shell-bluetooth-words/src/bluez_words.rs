//! THE RADIO AS THE SHELL SAYS IT — what the footer, which keeps the radio
//! for the session, and the radio's view (`ronoctua-shell-bluetooth`) both
//! say and both read.
//!
//! # Why a leaf under both
//!
//! The pairing agent is the footer's (bluetoothd asks the default agent, and
//! only the process that outlives every window may register it), and so are
//! the modeline's device marks, the radio's alarms and the three keys that act
//! on the whole adapter with no view open. The VIEW is a feature of its own
//! crate, which the footer never names (`footer-names-no-feature`). What the
//! two share — the marks, the pairing question and the words its answers
//! cross a socket in, the refusals' sentences, the adapter's three orders —
//! is written once, here, on no other shell crate: a line of the view
//! recompiles the view and the binary, and a line of this recompiles both
//! doors.

use ronoctua_bluetooth::{Ask, Kind, Refusal, Scene};
use ronoctua_tokens as tokens;

/// What the message line says when the daemon refused an order.
#[must_use]
pub fn refused(ask: &Ask, why: Refusal, words: &str, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    let what = texts.t(match ask {
        Ask::Connect(_) | Ask::Disconnect(_) | Ask::Profile { .. } => key::BLUETOOTH_WHAT_CONNECT,
        Ask::Pair(_) => key::BLUETOOTH_WHAT_PAIR,
        Ask::Forget(_) => key::BLUETOOTH_WHAT_FORGET,
        Ask::Power(_)
        | Ask::Discoverable(_)
        | Ask::Pairable(_)
        | Ask::Scan(_)
        | Ask::AdapterAlias(_)
        | Ask::DiscoverableTimeout(_)
        | Ask::PairableTimeout(_) => key::BLUETOOTH_WHAT_RADIO,
        Ask::Trust(..) | Ask::Block(..) | Ask::Rename(..) | Ask::Wake(..) => {
            key::BLUETOOTH_WHAT_DEVICE
        }
    });
    let reason = texts.t(match why {
        Refusal::NotAuthorized => key::BLUETOOTH_REFUSED_AUTHORITY,
        Refusal::NotAvailable => key::BLUETOOTH_REFUSED_ABSENT,
        Refusal::Away => key::BLUETOOTH_REFUSED_AWAY,
        Refusal::Busy => key::BLUETOOTH_REFUSED_BUSY,
        Refusal::Rejected => key::BLUETOOTH_REFUSED_REJECTED,
        Refusal::TimedOut => key::BLUETOOTH_REFUSED_TIMED_OUT,
        Refusal::NoBus => key::BLUETOOTH_REFUSED_NO_BUS,
        Refusal::Other => key::BLUETOOTH_REFUSED_OTHER,
    });
    // BLUEZ'S OWN WORDS GO TO THE LOG: they are English, and the line is in
    // the person's language. The name already chose the phrase.
    tracing::info!(?why, words, "the Bluetooth daemon refused");
    texts.t_with(
        key::ECHO_BLUETOOTH_REFUSED,
        &[("what", &what), ("why", &reason)],
    )
}

/// The four device marks the modeline can carry at once.
///
/// FOUR literals and not a `format!`: `layout::Segment::id` is a
/// `&'static str`, for the reason the nine workspace digits are. Four is what
/// fits beside the battery, the profile and the bell on a narrow screen; the
/// fifth connected device is in the view, which is where the rest of it is.
pub const POOL: [&str; 4] = ["bt-1", "bt-2", "bt-3", "bt-4"];

/// The glyph of a kind — one table, and it is the kind's own id.
#[must_use]
pub fn glyph(kind: Kind) -> char {
    match kind {
        Kind::Headphones => tokens::icon::BLUETOOTH_DEVICE_HEADPHONES,
        Kind::Headset => tokens::icon::BLUETOOTH_DEVICE_HEADSET,
        Kind::Speaker => tokens::icon::BLUETOOTH_DEVICE_SPEAKER,
        Kind::Keyboard => tokens::icon::BLUETOOTH_DEVICE_KEYBOARD,
        Kind::Mouse => tokens::icon::BLUETOOTH_DEVICE_MOUSE,
        Kind::Gamepad => tokens::icon::BLUETOOTH_DEVICE_GAMEPAD,
        Kind::Tablet => tokens::icon::BLUETOOTH_DEVICE_TABLET,
        Kind::Phone => tokens::icon::BLUETOOTH_DEVICE_PHONE,
        Kind::Computer => tokens::icon::BLUETOOTH_DEVICE_COMPUTER,
        Kind::Watch => tokens::icon::BLUETOOTH_DEVICE_WATCH,
        Kind::Camera => tokens::icon::BLUETOOTH_DEVICE_CAMERA,
        Kind::Printer => tokens::icon::BLUETOOTH_DEVICE_PRINTER,
        Kind::Network => tokens::icon::BLUETOOTH_DEVICE_NETWORK,
        Kind::Display => tokens::icon::BLUETOOTH_DEVICE_DISPLAY,
        Kind::Unknown => tokens::icon::BLUETOOTH_DEVICE_UNKNOWN,
    }
}

/// One mark on the modeline: what to draw, and whether it is alarmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The device's path — what a click on it opens.
    pub at: String,
    /// The glyph, and the percentage when it is low enough to matter.
    pub text: String,
    /// Is the battery at or under the danger mark?
    pub alarmed: bool,
}

/// What the modeline draws: one mark per CONNECTED device, four at most.
///
/// PURE, and tested beside the view: the rule is the owner's — the kind's
/// glyph, the percentage only at [`ronoctua_bluetooth::SHOWN_FROM`] and
/// under, the danger colour at [`ronoctua_bluetooth::DANGER_FROM`] and under
/// — and nothing about it needs a bus.
#[must_use]
pub fn modeline(scene: &Scene) -> Vec<Shown> {
    scene
        .connected()
        .into_iter()
        .take(POOL.len())
        .map(|d| Shown {
            at: d.at.clone(),
            text: match d.battery.filter(|_| d.shows_percent()) {
                Some(p) => format!("{} {p}%", glyph(d.kind)),
                None => glyph(d.kind).to_string(),
            },
            alarmed: d.alarmed(),
        })
        .collect()
}

/// The line a door opens for a new name — the command whose argument it is.
pub const RENAME: &str = "bluetooth-rename";

/// The line a door opens for a PIN or a passkey, masked — and the command
/// that says yes to the pairing, with the digits as its argument.
pub const ACCEPT: &str = "bluetooth-accept";

/// The command that says no to the pairing.
pub const REFUSE: &str = "bluetooth-refuse";

/// What [`ACCEPT`] and [`REFUSE`] carry as their argument.
///
/// # An answer from a window NAMES ITS QUESTION
///
/// The window hears the question through a file and answers it down a
/// socket, and the footer's agent answers whatever is waiting when the order
/// arrives. Between the two, the question can end and another take its
/// place — the other side cancels, another device asks — and a PIN typed for
/// one device, or a yes pressed for it, confirmed the next one. So the
/// window's answer says which question it was given for: the device's path
/// and what was asked ([`standing::asked_word`]), before the digits. The
/// footer answers only when that is still the question standing
/// ([`Answer::fits`]), and the agent only when it is still the one waiting
/// (`Questions::answer`).
///
/// THE FOOTER'S OWN ANSWERS NAME NOTHING: its line, its keys and its card
/// answer the question the footer holds, in the process that holds it, and
/// its line is let go the moment that question ends. They are the plain
/// form — no argument, or the digits alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer<'a> {
    /// The question the footer holds now: the digits, or nothing typed.
    Plain(&'a str),
    /// The question named — its device and what it asked — and the digits.
    For {
        /// The device's object path.
        device: &'a str,
        /// What was asked, by [`standing::asked_word`].
        asked: &'a str,
        /// What was typed for it; empty for a yes or a no.
        typed: &'a str,
    },
}

impl<'a> Answer<'a> {
    /// The argument as the footer reads it.
    ///
    /// A DEVICE'S PATH STARTS WITH `/`, and that is the mark: digits typed on
    /// the footer's own line are the plain form. A PIN that itself began with
    /// `/` would be read as a named answer and refused for naming nothing —
    /// the safe way to be wrong about it.
    #[must_use]
    pub fn read(argument: Option<&'a str>) -> Self {
        let argument = argument.unwrap_or_default();
        if !argument.starts_with('/') {
            return Self::Plain(argument);
        }
        let mut words = argument.splitn(3, ' ');
        let device = words.next().unwrap_or_default();
        let asked = words.next().unwrap_or_default();
        let typed = words.next().unwrap_or_default();
        Self::For {
            device,
            asked,
            typed,
        }
    }

    /// The argument a window sends for `question`, with what was typed for
    /// it — the digits never in an argv or a log: down the control socket.
    #[must_use]
    pub fn named(question: &ronoctua_bluetooth::agent::Question, typed: Option<&str>) -> String {
        let asked = standing::asked_word(&question.asked);
        match typed {
            Some(t) => format!("{} {asked} {t}", question.device),
            None => format!("{} {asked}", question.device),
        }
    }

    /// Whether this answer is for `standing` — the question the footer holds.
    /// A plain answer is, by construction; a named one only when it names it.
    #[must_use]
    pub fn fits(&self, standing: &ronoctua_bluetooth::agent::Question) -> bool {
        match self {
            Self::Plain(_) => true,
            Self::For { device, asked, .. } => {
                *device == standing.device && *asked == standing::asked_word(&standing.asked)
            }
        }
    }

    /// What was typed.
    #[must_use]
    pub const fn typed(&self) -> &'a str {
        match self {
            Self::Plain(typed) | Self::For { typed, .. } => typed,
        }
    }
}

/// THE STANDING QUESTION, where the window can read it.
///
/// # Why a file
///
/// The agent is the footer's: bluetoothd asks the DEFAULT agent, only one
/// door may register it (`ronoctua_bluetooth::watch`), and the footer is the
/// process that outlives every window. So a question arrives in the footer's
/// process — and the window has to show it and answer it all the same (the
/// owner's rule: the two doors do the same things). What carries it across
/// is the rule every other pair of doors here keeps: the file is the
/// broadcast. The footer writes it when a question arrives and takes it away
/// when the question ends; the window watches it by inotify.
///
/// The ANSWER travels the other way, down the control socket
/// (`bluetooth-accept`, `bluetooth-refuse`) — never in an argv — and names
/// the question it was given for ([`Answer`]).
///
/// # What is in it
///
/// The device's path, what is asked, and the code to compare or to type on
/// the other thing — the digits the footer's own screen and its card already
/// show. A PIN typed HERE is never in it. It is written `0600`, in the
/// runtime directory, named per display beside the control socket.
///
/// # A file whose writer is gone is no question
///
/// A footer that died with a question up leaves the file behind, and its
/// agent went with it: the question is over. So the file carries the footer's
/// pid, and a reader whose writer is not alive reads nothing. The next footer
/// also clears it as it starts.
pub mod standing {
    use ronoctua_bluetooth::agent::{Asked, Question};

    /// The file's name, after the instance and the display.
    const LEAF: &str = "bluetooth-question.json";

    /// What the file holds — [`Question`] and its writer, in words a reader
    /// refuses when it does not know them.
    #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct Written {
        /// The footer that wrote it.
        pid: u32,
        /// The device's object path.
        device: String,
        /// What is asked, by name.
        asked: String,
        /// The code or the service, when the question carries one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    }

    /// `$XDG_RUNTIME_DIR/<name>-<display>-bluetooth-question.json`: per
    /// display, as the control socket is, because a question belongs to the
    /// footer of one display.
    #[must_use]
    pub fn path() -> Option<std::path::PathBuf> {
        let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".into());
        ronoctua_xdg::runtime_file(&format!("{display}-{LEAF}"))
    }

    /// What is asked, as one word — the file's, and the one a window's
    /// answer names its question by (`bluetooth::Answer`).
    #[must_use]
    pub const fn asked_word(asked: &Asked) -> &'static str {
        match asked {
            Asked::Confirm(_) => "confirm",
            Asked::Passkey(_) => "passkey",
            Asked::Pin => "pin",
            Asked::Typed => "typed",
            Asked::Authorize => "authorize",
            Asked::Service(_) => "service",
        }
    }

    /// The question as the file says it, written by `pid`.
    #[must_use]
    pub fn written(question: &Question, pid: u32) -> String {
        let detail = match &question.asked {
            Asked::Confirm(code) | Asked::Passkey(code) => Some(code.clone()),
            Asked::Service(uuid) => Some(uuid.clone()),
            Asked::Pin | Asked::Typed | Asked::Authorize => None,
        };
        serde_json::to_string(&Written {
            pid,
            device: question.device.clone(),
            asked: asked_word(&question.asked).to_owned(),
            detail,
        })
        .unwrap_or_default()
    }

    /// The question in `text`, and who wrote it.
    ///
    /// A word this does not know is NO QUESTION, never a plausible one: a
    /// yes/no drawn for a question that wanted digits would send the other
    /// side an empty PIN.
    #[must_use]
    pub fn read_from(text: &str) -> Option<(u32, Question)> {
        let w: Written = serde_json::from_str(text).ok()?;
        let asked = match (w.asked.as_str(), w.detail) {
            ("confirm", Some(code)) => Asked::Confirm(code),
            ("passkey", Some(code)) => Asked::Passkey(code),
            ("pin", None) => Asked::Pin,
            ("typed", None) => Asked::Typed,
            ("authorize", None) => Asked::Authorize,
            ("service", Some(uuid)) => Asked::Service(uuid),
            _ => return None,
        };
        (!w.device.is_empty()).then_some((
            w.pid,
            Question {
                device: w.device,
                asked,
            },
        ))
    }

    /// Writes the question that stands — or takes the file away when none
    /// does. Written beside and renamed over, so a reader never sees half of
    /// it; `0600`, because the code is the other thing's.
    pub fn publish(question: Option<&Question>) {
        let Some(path) = path() else {
            return;
        };
        let Some(question) = question else {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    tracing::warn!(error = %e, "the pairing question could not be taken back")
                }
            }
            return;
        };
        if let Err(e) = write(&path, &written(question, std::process::id())) {
            tracing::warn!(error = %e, "the pairing question could not be written for the window");
        }
    }

    /// `text` into `path`, whole, owner-only.
    fn write(path: &std::path::Path, text: &str) -> std::io::Result<()> {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let draft = path.with_extension(format!("json.{}", std::process::id()));
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&draft)?;
        f.write_all(text.as_bytes())?;
        drop(f);
        std::fs::rename(&draft, path)
    }

    /// The question standing now, when its footer is alive to be answered.
    #[must_use]
    pub fn read() -> Option<Question> {
        let text = std::fs::read_to_string(path()?).ok()?;
        let (pid, question) = read_from(&text)?;
        let pid = rustix::process::Pid::from_raw(i32::try_from(pid).ok()?)?;
        rustix::process::test_kill_process(pid).ok()?;
        Some(question)
    }

    /// Watches the file, calling `stir` when it is written or taken away.
    ///
    /// At once, and not coalesced: a question is something somebody is
    /// waiting on, and a quarter of a second of hold-off is a quarter of a
    /// second of a device waiting for a yes.
    #[must_use]
    pub fn watch(
        stir: impl Fn() -> bool + Send + 'static,
    ) -> Option<ronoctua_files::notice::Notice> {
        let path = path()?;
        let (dir, name) = (path.parent()?, path.file_name()?.to_str()?);
        match ronoctua_files::notice::Notice::on_file(
            dir,
            name,
            ronoctua_files::notice::Pace::AtOnce,
            stir,
        ) {
            Ok(n) => Some(n),
            Err(e) => {
                tracing::warn!(error = %e, "the pairing question will not reach this window");
                None
            }
        }
    }
}

/// The bluetooth window's `app_id` — the window's own (its
/// `bluetooth_window::APP_ID` is this one), here so the footer asks whether
/// the person is at that window without naming the window.
pub const APP_ID: &str = "br.ronoctua.bluetooth";

/// Whether the person is at THIS window, by what the compositor says has the
/// focus — asked by the footer when a pairing question arrives.
///
/// The window then answers it, and the footer neither opens its region nor
/// takes the keyboard: a question the window started with `p` is typed into
/// the window, and a footer that grabbed the keyboard for it would be the
/// one door sending the person to the other.
#[must_use]
pub fn has_the_person(focused_app_id: Option<&str>) -> bool {
    focused_app_id == Some(APP_ID)
}

/// One of the three keys that act on the whole ADAPTER, whatever row a
/// cursor is on: `s` looks for devices, `o` turns the radio on or off, `v`
/// lets others see this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKey {
    /// Look for devices, or stop.
    Scan,
    /// Turn the radio on or off.
    Power,
    /// Let others see this machine, or stop.
    Discoverable,
}

/// What one of the adapter's three keys orders, over a reading of the radio
/// — the ONE answer the view's keys and the footer's (with the view closed,
/// `LEADER s b s`) both read, so the two doors cannot answer differently.
///
/// They obey the rule their own rows draw: `radio()` marks scan and
/// discoverable `disabled` while the adapter is off, and `RET` on a dim row
/// does nothing — so these two order nothing then, where they used to reach
/// `org.bluez.Error.NotReady` and say "there is no such thing on this
/// machine". POWER IS THE EXCEPTION, and has to be: it is the key that turns
/// the radio back on, so a rule that refused it while the radio was off would
/// be a switch with no way back.
#[must_use]
pub fn adapter_order(key: AdapterKey, scene: &Scene) -> Option<Ask> {
    let adapter = scene.adapter();
    let powered = adapter.is_some_and(|a| a.powered);
    match key {
        AdapterKey::Scan if !powered => None,
        AdapterKey::Scan => Some(Ask::Scan(!adapter.is_some_and(|a| a.discovering))),
        AdapterKey::Power => Some(Ask::Power(!powered)),
        AdapterKey::Discoverable if !powered => None,
        AdapterKey::Discoverable => {
            Some(Ask::Discoverable(!adapter.is_some_and(|a| a.discoverable)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_has_a_sentence() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let asks = [
            Ask::Connect("/d/1".to_owned()),
            Ask::Pair("/d/1".to_owned()),
            Ask::Forget("/d/1".to_owned()),
            Ask::Power(true),
            Ask::Trust("/d/1".to_owned(), true),
        ];
        let whys = [
            Refusal::NotAuthorized,
            Refusal::NotAvailable,
            Refusal::Away,
            Refusal::Busy,
            Refusal::Rejected,
            Refusal::TimedOut,
            Refusal::NoBus,
            Refusal::Other,
        ];
        let mut seen = std::collections::BTreeSet::new();
        for ask in &asks {
            for why in whys {
                let said = refused(ask, why, "org.bluez.Error.Failed", &t);
                assert!(!said.contains('['), "a key reached the screen raw: {said}");
                // THE DAEMON'S WORDS NEVER REACH THE LINE: they are English.
                assert!(
                    !said.contains("org.bluez"),
                    "the daemon's words were drawn: {said}"
                );
                seen.insert(said);
            }
        }
        // AND EVERY REFUSAL IS ITS OWN SENTENCE.
        assert_eq!(seen.len(), asks.len() * whys.len());
    }

    /// THE FOOTER LEAVES A QUESTION TO THE WINDOW only when the person is
    /// at it.
    #[test]
    fn the_window_answers_only_when_it_has_the_person() {
        assert!(has_the_person(Some(APP_ID)));
        assert!(!has_the_person(Some("br.ronoctua.power")));
        assert!(!has_the_person(None));
    }

    /// THE STANDING QUESTION CROSSES INTACT, every kind of it, with the pid
    /// of the footer that wrote it — and a word the reader does not know is
    /// no question, never a yes/no drawn for one that wanted digits.
    #[test]
    fn the_standing_question_is_read_back_as_it_was_written() {
        use ronoctua_bluetooth::agent::{Asked, Question};
        for asked in [
            Asked::Confirm("012345".to_owned()),
            Asked::Passkey("000042".to_owned()),
            Asked::Pin,
            Asked::Typed,
            Asked::Authorize,
            Asked::Service("0000110b-0000-1000-8000-00805f9b34fb".to_owned()),
        ] {
            let q = Question {
                device: "/org/bluez/hci0/dev_X".to_owned(),
                asked,
            };
            let text = standing::written(&q, 4242);
            assert_eq!(
                standing::read_from(&text),
                Some((4242, q.clone())),
                "{text}"
            );
        }
        for wrong in [
            r#"{"pid":1,"device":"/d","asked":"pin","detail":"0000"}"#,
            r#"{"pid":1,"device":"/d","asked":"confirm"}"#,
            r#"{"pid":1,"device":"/d","asked":"yes-please"}"#,
            r#"{"pid":1,"device":"","asked":"pin"}"#,
            "",
            "{",
        ] {
            assert_eq!(standing::read_from(wrong), None, "{wrong}");
        }
    }
}
