//! The pairing agent — the object bluetoothd asks its questions of.
//!
//! # Why there is one at all
//!
//! Pairing is a question with two sides: a machine shows six digits and a
//! person says whether the other thing shows the same. Without an agent the
//! daemon answers for us — it refuses everything that needs a person — and
//! `Pair` on a keyboard fails with nothing on screen explaining why.
//!
//! # It claims no name on the bus, and that is not an oversight
//!
//! `AgentManager1.RegisterAgent` takes an object PATH, and bluetoothd tracks
//! the caller by its unique connection name. dbus's own default policy is
//! `<deny own="*"/>` and BlueZ's file opens `own` for root alone, so asking
//! for a well-known name here would fail — and the error would say "name has
//! no owner", which is the same thing an absent daemon says. The direction
//! that matters is already allowed: root's policy carries
//! `<allow send_interface="org.bluez.Agent1"/>`, which is what lets the
//! daemon call back into this object.
//!
//! # The question waits on the person, and the handlers are async
//!
//! bluetoothd gives an agent SIXTY seconds. The handlers are `async fn` so
//! that waiting parks a task instead of blocking the connection's executor —
//! blocking it would freeze the orders thread's own calls, including the
//! `CancelPairing` that takes the question back.

use std::sync::{Arc, Mutex};

use futures_channel::oneshot;
use futures_lite::FutureExt;

use crate::Event;

/// Where our agent object lives. Any path works; this one says whose it is.
pub const PATH: &str = "/org/ronoctua/BluetoothAgent";

/// What we can do about a question: show a number and take a yes or a no.
///
/// `KeyboardDisplay` is the honest description of a laptop — it can show six
/// digits AND have them typed. `NoInputNoOutput` would make every pairing
/// silent and unauthenticated, which is a different security promise.
pub const CAPABILITY: &str = "KeyboardDisplay";

/// How long we hold a question before answering no on our own. Under
/// bluetoothd's sixty seconds, so that the refusal is OURS and legible
/// rather than a timeout on the other side.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(55);

/// What the other side is asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// "Does it show these six digits?" — both sides compare, nobody types.
    Confirm(String),
    /// "Type these six digits on it" — a keyboard, which has no screen.
    Passkey(String),
    /// It wants a PIN typed here — old devices, and usually `0000`.
    Pin,
    /// It wants a PASSKEY typed here: six digits, as a number.
    ///
    /// A VARIANT OF ITS OWN and not `Pin`, because the sentence differs and
    /// the two requests do: a PIN is the device's own code, often four digits
    /// printed on it; a passkey is six digits BlueZ hands back as a number.
    /// `RequestPasskey` used to be labelled a PIN, which sent people looking
    /// for a code on a keyboard that has none.
    Typed,
    /// It only wants a yes: no number to compare.
    Authorize,
    /// A device already paired wants to use one more service.
    Service(String),
}

/// One question, as the screen has to draw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// The device's object path — the id the view acts on.
    pub device: String,
    /// What is being asked.
    pub asked: Asked,
}

impl Asked {
    /// Does answering it need something TYPED — a PIN, a passkey — rather
    /// than a yes or a no?
    #[must_use]
    pub const fn typed(&self) -> bool {
        matches!(self, Self::Pin | Self::Typed)
    }

    /// Is it only telling? `DisplayPasskey` and `DisplayPinCode` have no
    /// answer at all: the digits are typed on the OTHER device, and the
    /// pairing ends by itself.
    #[must_use]
    pub const fn only_tells(&self) -> bool {
        matches!(self, Self::Passkey(_))
    }
}

/// What the agent tells the footer with. `Sync` because the handlers are
/// futures the object server may move between threads.
type Feed = Arc<dyn Fn(Event) + Send + Sync>;

/// The waiting question, shared between the agent and whoever answers.
///
/// The serial is what keeps a timeout from clearing somebody else's
/// question: two can overlap by a hair — one times out at the instant the
/// next is registered — and a blind `take()` would leave the new one with no
/// sender, unanswerable and waiting out its own fifty-five seconds.
#[derive(Clone)]
pub struct Questions {
    pending: Waiting,
    next: Arc<std::sync::atomic::AtomicU64>,
}

/// The question that is waiting, if any, and the serial that says whose.
type Waiting = Arc<Mutex<Option<(u64, oneshot::Sender<Reply>)>>>;

/// What a person answered.
struct Reply {
    yes: bool,
    typed: String,
}

impl Questions {
    /// A handle with nothing waiting on it.
    #[must_use]
    pub fn idle() -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
            next: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// Answers whatever is waiting. A `no` for nothing is nothing.
    pub fn answer(&self, yes: bool, typed: &str) {
        let taken = self.pending.lock().ok().and_then(|mut p| p.take());
        if let Some((_, tell)) = taken {
            let _ = tell.send(Reply {
                yes,
                typed: typed.to_owned(),
            });
        }
    }

    /// Drops the question with THIS serial, and says whether it was still
    /// the one waiting.
    fn expire(&self, serial: u64) -> bool {
        let Ok(mut pending) = self.pending.lock() else {
            return false;
        };
        if pending.as_ref().is_some_and(|(s, _)| *s == serial) {
            *pending = None;
            return true;
        }
        false
    }

    /// Parks until somebody answers, or until [`PATIENCE`] runs out.
    ///
    /// A question that arrives while another waits ANSWERS THE OLD ONE with a
    /// no: bluetoothd asks one thing at a time, so a second question means
    /// the first is moot, and leaving it parked would leak a task.
    async fn ask(&self, feed: &Feed, question: Question) -> Option<Reply> {
        let (tell, hear) = oneshot::channel();
        let serial = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Some((_, old)) = self
            .pending
            .lock()
            .ok()
            .and_then(|mut p| p.replace((serial, tell)))
        {
            let _ = old.send(Reply {
                yes: false,
                typed: String::new(),
            });
        }
        feed(Event::Asked(question));
        let timeout = async {
            async_io::Timer::after(PATIENCE).await;
            Err(oneshot::Canceled)
        };
        let answer = hear.or(timeout).await.ok();
        // NOBODY ANSWERED: take the question off the screen, so that the
        // footer is not left showing something the daemon has forgotten —
        // but only if it is still MINE. A question that arrived in the
        // meantime is somebody else's, and clearing it would leave it
        // unanswerable.
        if answer.is_none() && self.expire(serial) {
            feed(Event::Unasked);
        }
        answer
    }
}

/// The errors bluetoothd understands from an agent.
///
/// The NAME is what it reads: `Rejected` is "the person said no", and
/// `Canceled` is "nobody is going to answer". A generic error would be
/// reported as an agent failure and would point the diagnosis at us.
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
pub enum Failure {
    /// The person said no.
    Rejected(String),
    /// Nobody answered.
    Canceled(String),
}

/// The object bluetoothd calls.
struct Agent {
    questions: Questions,
    feed: Feed,
}

#[zbus::interface(name = "org.bluez.Agent1")]
impl Agent {
    /// The daemon has given up on this pairing.
    async fn cancel(&self) {
        self.questions.answer(false, "");
        (self.feed)(Event::Unasked);
    }

    /// The daemon is done with us.
    async fn release(&self) {}

    /// Both sides show the same six digits; the person compares them.
    async fn request_confirmation(
        &self,
        device: zbus::zvariant::OwnedObjectPath,
        passkey: u32,
    ) -> Result<(), Failure> {
        // SIX DIGITS, ZERO-PADDED. The specification's passkey is a number
        // under a million, and `12345` on one screen against `012345` on the
        // other is a person deciding they do not match.
        self.wait(device, Asked::Confirm(format!("{passkey:06}")))
            .await
    }

    /// It has no screen: the digits are typed on it.
    ///
    /// # Why `entered` is not ignored
    ///
    /// bluetoothd calls this AGAIN on every keypress the other side reports,
    /// with `entered` counting them — six calls for a six-digit passkey. Each
    /// call used to raise a fresh question: in the view it reset the cursor,
    /// so the sentence somebody had just dismissed came straight back; and
    /// with the view closed each one became a NEW notification with a new id
    /// and `rewrite = false`, so a full passkey left seven critical cards
    /// standing, none of which anything withdrew.
    ///
    /// The first call is the one that has something to say. The rest are
    /// progress on a number already on screen.
    async fn display_passkey(
        &self,
        device: zbus::zvariant::OwnedObjectPath,
        passkey: u32,
        entered: u16,
    ) {
        if entered > 0 {
            return;
        }
        let question = Question {
            device: device.as_str().to_owned(),
            asked: Asked::Passkey(format!("{passkey:06}")),
        };
        // NOTHING TO ANSWER: this one only tells, and `Asked::only_tells` is
        // what the screen reads. It ends when the device pairs — the door
        // drops it on the reading that says so — or when `Cancel` arrives.
        (self.feed)(Event::Asked(question));
    }

    /// The same, for a device that wants a PIN string.
    async fn display_pin_code(&self, device: zbus::zvariant::OwnedObjectPath, pincode: String) {
        (self.feed)(Event::Asked(Question {
            device: device.as_str().to_owned(),
            asked: Asked::Passkey(pincode),
        }));
    }

    /// It wants a passkey typed HERE.
    async fn request_passkey(
        &self,
        device: zbus::zvariant::OwnedObjectPath,
    ) -> Result<u32, Failure> {
        let reply = self.answer(device, Asked::Typed).await?;
        reply
            .typed
            .trim()
            .parse::<u32>()
            .map_err(|_| Failure::Rejected("that is not a passkey".into()))
    }

    /// It wants a PIN typed here.
    async fn request_pin_code(
        &self,
        device: zbus::zvariant::OwnedObjectPath,
    ) -> Result<String, Failure> {
        let reply = self.answer(device, Asked::Pin).await?;
        Ok(reply.typed)
    }

    /// It only wants a yes.
    async fn request_authorization(
        &self,
        device: zbus::zvariant::OwnedObjectPath,
    ) -> Result<(), Failure> {
        self.wait(device, Asked::Authorize).await
    }

    /// A device we know wants one more service.
    async fn authorize_service(
        &self,
        device: zbus::zvariant::OwnedObjectPath,
        uuid: String,
    ) -> Result<(), Failure> {
        self.wait(device, Asked::Service(uuid)).await
    }
}

impl Agent {
    /// Asks, and turns the answer into the two names bluetoothd knows.
    async fn answer(
        &self,
        device: zbus::zvariant::OwnedObjectPath,
        asked: Asked,
    ) -> Result<Reply, Failure> {
        let question = Question {
            device: device.as_str().to_owned(),
            asked,
        };
        match self.questions.ask(&self.feed, question).await {
            None => Err(Failure::Canceled("nobody answered".into())),
            Some(reply) if !reply.yes => Err(Failure::Rejected("the answer was no".into())),
            Some(reply) => Ok(reply),
        }
    }

    /// The same, for the questions whose answer is only yes or no.
    async fn wait(
        &self,
        device: zbus::zvariant::OwnedObjectPath,
        asked: Asked,
    ) -> Result<(), Failure> {
        self.answer(device, asked).await.map(|_| ())
    }
}

/// Puts the agent on the bus and registers it with bluetoothd, against the
/// handle whoever raised the threads already holds.
///
/// Failure is not fatal and not returned: a machine with no bluetoothd has
/// nobody to register with, and everything else about the view still works.
/// It says so to the log, once.
pub fn serve(connection: &zbus::blocking::Connection, questions: &Questions, feed: Feed) {
    let agent = Agent {
        questions: questions.clone(),
        feed,
    };
    if let Err(e) = connection.object_server().at(PATH, agent) {
        tracing::warn!(error = %e, "the pairing agent could not be put on the bus");
        return;
    }
    register(connection);
}

/// Asks bluetoothd to take the agent that is already on the bus.
///
/// # Why this is not part of [`serve`]
///
/// Because the object goes on OUR connection once and stays there, while the
/// registration lives inside `bluetoothd` and dies with it. And it dies more
/// often than one would think: `nix/modules/bluetooth.nix` writes
/// `main.conf`, so any rebuild that touches it restarts the unit — and the
/// footer, which does not restart, was left with an agent bluetoothd had
/// never heard of. From the outside that is a pairing that fails saying "the
/// other side said no", with nothing on screen about an agent.
///
/// The other way in is a machine that booted with no radio: the unit is
/// skipped by its own `ConditionPathIsDirectory`, the first registration
/// fails, and plugging the dongle in drew every device and could still not
/// ask a single question.
///
/// So it is called again on every `NameOwnerChanged` for `org.bluez`, and it
/// is written to be safe to call twice: bluetoothd answers `AlreadyExists`,
/// which is a sentence in the journal and nothing else.
pub fn register(connection: &zbus::blocking::Connection) {
    let Ok(path) = zbus::zvariant::ObjectPath::try_from(PATH) else {
        return;
    };
    match crate::bluez::AgentManagerProxyBlocking::new(connection) {
        Ok(manager) => {
            if let Err(e) = manager.register_agent(&path, CAPABILITY) {
                tracing::info!(error = %e, "bluez did not take the pairing agent");
            } else if let Err(e) = manager.request_default_agent(&path) {
                tracing::debug!(error = %e, "another agent is the default one");
            } else {
                tracing::info!(capability = CAPABILITY, "the pairing agent is registered");
            }
        }
        Err(e) => tracing::debug!(error = %e, "bluez is not on the bus; no pairing agent"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timeout_clears_only_its_own_question() {
        let q = Questions::idle();
        let (mine, _hear) = oneshot::channel();
        *q.pending.lock().unwrap() = Some((7, mine));
        // Somebody else's serial: the question stays.
        assert!(!q.expire(6));
        assert!(q.pending.lock().unwrap().is_some());
        // Its own: it goes.
        assert!(q.expire(7));
        assert!(q.pending.lock().unwrap().is_none());
        // And expiring nothing is nothing.
        assert!(!q.expire(7));
    }

    #[test]
    fn an_answer_to_nothing_is_nothing() {
        let q = Questions::idle();
        // No panic, no wait: there is no question.
        q.answer(true, "123456");
        q.answer(false, "");
    }

    #[test]
    fn an_answer_reaches_the_question_that_is_waiting() {
        let q = Questions::idle();
        let (tell, hear) = oneshot::channel();
        *q.pending.lock().unwrap() = Some((1, tell));
        q.answer(true, "012345");
        let reply = futures_lite::future::block_on(hear).unwrap();
        assert!(reply.yes);
        assert_eq!(reply.typed, "012345");
        assert!(q.pending.lock().unwrap().is_none(), "the question stayed");
    }

    #[test]
    fn the_passkey_is_six_digits() {
        // The formatting the screen shows, which is what the other side
        // shows: a number, zero-padded to the specification's width.
        assert_eq!(format!("{:06}", 12_345_u32), "012345");
        assert_eq!(format!("{:06}", 0_u32), "000000");
        assert_eq!(format!("{:06}", 999_999_u32), "999999");
    }
}
