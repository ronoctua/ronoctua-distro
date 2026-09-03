//! The `org.freedesktop.Notifications` daemon — section 4.2.
//!
//! # Blocking, on a thread
//!
//! zbus has an async API and a blocking one. We use the **blocking** one, on a
//! thread of its own, with a channel to the main loop — the same shape as niri's
//! event stream (`daemon.rs` in the shell). Bringing an async executor inside
//! a calloop loop would add a second concurrency model
//! to solve a problem a thread already solves.
//!
//! # What this module does NOT decide
//!
//! Node policy at all. It translates D-Bus into [`crate::Notification`] and sends it
//! down the channel; what decides what appears, for how long and in what order is
//! [`crate::Queue`], which does not know D-Bus exists.

// `#[zbus(signal)]` expands into helper methods with no doc comment. The macro
// writes them and gives them spans of its own, so an `#[allow]` on the item, on
// the impl or on the attribute does not reach them — all three were tried.
//
// The exemption is therefore on this module and nowhere wider: everything the
// crate exports elsewhere stays under `warn(missing_docs)`, and every public
// item WRITTEN here still carries its own documentation, which is what the lint
// is for.
#![allow(missing_docs)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};

use zbus::zvariant::Value;

use crate::action::Action;
use crate::{Notification, Urgency};

/// The name the daemon claims on the session bus.
pub const NAME: &str = "org.freedesktop.Notifications";
/// The object path, fixed by the specification.
pub const PATH: &str = "/org/freedesktop/Notifications";

/// Why a notification was closed, as the specification numbers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    /// Its time ran out.
    Expired = 1,
    /// The person dismissed it.
    ByHand = 2,
    /// `CloseNotification` was called.
    ByRequest = 3,
}

/// The live connection, so the footer can answer BACK to the bus.
///
/// # Why a static, and not something handed around
///
/// Because the connection is created on the daemon's own thread and has to stay
/// alive there — dropping it releases the name, and the bus starts refusing
/// messages with nothing here noticing. The footer needs it for two messages
/// that travel the other way (`ActionInvoked`, `NotificationClosed`), and
/// threading a handle back out through the channel would give a second thing to
/// keep in step with the thread's lifetime.
///
/// `OnceLock` and not a `Mutex<Option<_>>`: it is written once, when the name
/// is claimed, and never again.
static BUS: OnceLock<zbus::blocking::Connection> = OnceLock::new();

/// What arrives from the bus.
#[derive(Debug, Clone)]
pub enum Event {
    /// A new notification, or one replacing another.
    New(Box<Notification>),
    /// A request to close by id.
    Close(u32),
    /// `systemd-oomd` closed a cgroup to keep the machine alive.
    ///
    /// It does not come from THIS bus — see [`crate::oom`], which listens on the
    /// system one. It shares the enum because what the footer does with it is
    /// the same: it becomes a notification. A second channel and a second bridge
    /// would be two paths to keep in step for one destination.
    ///
    /// The cgroup path arrives RAW. This crate does not know what a buffer is
    /// and does not translate; the shell resolves both.
    OomKilled {
        /// The cgroup path systemd killed, e.g.
        /// `/user.slice/…/app.slice/ronoctua-buffer-ab.scope`.
        cgroup: String,
        /// Why.
        reason: crate::oom::Reason,
    },
}

/// One hint, when it is a non-empty string.
///
/// A hint is a `Value` of whatever type the sender chose, and a client that
/// sends the wrong one gets its hint ignored rather than its whole notification
/// refused: a hint that took the message down would be worse than one nobody
/// read.
fn text_hint(hints: &HashMap<String, Value<'_>>, name: &str) -> Option<String> {
    let raw = hints.get(name)?.try_clone().ok()?;
    String::try_from(raw).ok().filter(|s| !s.is_empty())
}

/// The object exported on the bus.
struct Server {
    channel: Sender<Event>,
    next_id: Arc<AtomicU32>,
    /// It keeps the `Sender` behind a mutex because zbus requires `Sync` on the
    /// exported object, and `mpsc::Sender` is not.
    _guard: Mutex<()>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    /// `Notify` — the method that matters.
    ///
    /// A non-zero `replaces_id` replaces an existing notification,
    /// and the specification demands **reusing the id**. It is what a progress
    /// clock uses to update without piling up messages.
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &mut self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            // It starts at 1: the specification reserves 0 for "none".
            self.next_id.fetch_add(1, Ordering::Relaxed).max(1)
        };

        let urgency = hints
            .get("urgency")
            .and_then(|v| u8::try_from(v).ok())
            .map_or(Urgency::Normal, Urgency::from_protocol);

        // TAGS, SO AN APP OF OURS CAN NAME ITSELF AT THE SOURCE.
        //
        // This is the whole of "one of the distro's apps raises a customised
        // notification": it sends `--hint=string:tags:transfer,media` and the
        // catalogue recognises it by the tag, exactly as the out-of-memory one
        // does from inside the shell. Without it, an app that is not the shell
        // would have to be recognised by its summary — an English string, in a
        // system that translates.
        //
        // A hint and not a new method: the specification's `Notify` is what
        // every client already speaks, and a method of our own would make
        // OUR apps the ones that need a special library.
        //
        // Comma separated, trimmed, empties dropped. An app that sends
        // nonsense here gets a notification with no tags rather than an error:
        // this is a hint, and a hint that refuses the whole message would be
        // worse than one that is ignored.
        let tags: Vec<String> = text_hint(&hints, "tags")
            .map(|s| {
                s.split(',')
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default();

        // WHERE THE IMAGE IS, and there are two places by convention.
        //
        // The compositor puts a screenshot's `file://` URL straight into
        // `app_icon`, which is what our screenshot entry reads. libnotify does
        // not: `notify-send -i` loads the file and sends the standard
        // `image-path` hint instead, leaving `app_icon` empty.
        //
        // Reading only the first made every action that needs a file vanish for
        // every libnotify client, and the symptom was a band two lines tall
        // where three were expected. `image_path` with an underscore is the
        // spelling of the older specification, and clients still send it.
        let icon = Some(app_icon)
            .filter(|s| !s.is_empty())
            .or_else(|| text_hint(&hints, "image-path"))
            .or_else(|| text_hint(&hints, "image_path"));

        let mut n = Notification::fresh_one(id, app_name, icon, summary, body, urgency)
            .asking(expire_timeout);
        // THE ACTIONS WERE BEING DROPPED ON THE FLOOR, and the parameter was
        // called `_actions` to say so. An application that offers a button and
        // gets no `ActionInvoked` waits for ever — and `GetCapabilities` was
        // honest about it, which is why nothing broke and nothing worked.
        n.actions = Action::from_protocol(&actions);
        n.tags = tags;
        tracing::info!(
            id,
            urgency = ?n.urgency,
            app = %n.app,
            actions = n.actions.len(),
            tags = n.tags.len(),
            // THE ICON FIELD, RAW. It is where a `file://` URL arrives — the
            // compositor puts a screenshot's path there — and when the actions
            // that need a file all vanish, this line is the difference between
            // "the sender did not send one" and "we did not read it".
            icon = n.icon.as_deref().unwrap_or("-"),
            "notification received"
        );

        // A send error only happens with the main loop already dead. There is nothing
        // to log in a shell that is on its way out.
        let _ = self.channel.send(Event::New(Box::new(n)));
        id
    }

    /// `CloseNotification`.
    fn close_notification(&mut self, id: u32) {
        let _ = self.channel.send(Event::Close(id));
    }

    /// `GetCapabilities`.
    ///
    /// We announce only what we DELIVER — announcing a capability that does not
    /// exist makes the client rely on behaviour that never comes.
    ///
    /// `actions` went in when the buttons did, together with the
    /// `ActionInvoked` signal that answers them. `persistence` because a
    /// notification survives leaving the screen: it stays unread, it stays in
    /// the manager, and it stays on disk for fifteen days.
    ///
    /// Still left out: `body-markup` (the band draws text, not markup),
    /// `body-images` and `icon-multi`.
    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into(), "actions".into(), "persistence".into()]
    }

    /// `ActionInvoked` — the person pressed one of the sender's buttons.
    #[zbus(signal)]
    async fn action_invoked(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;

    /// `NotificationClosed` — with the reason, as the specification numbers it.
    #[zbus(signal)]
    async fn notification_closed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    /// `GetServerInformation`.
    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "ronoctua".into(),
            "ronoctua-distro".into(),
            env!("CARGO_PKG_VERSION").into(),
            // The version of the specification implemented.
            "1.2".into(),
        )
    }
}

/// Starts the daemon on a thread.
///
/// It returns `false` when the name cannot be claimed — **that is not fatal**:
/// another notification daemon already running, or no session bus. The
/// footer goes on working without notifications, and taking everything down would trade one
/// feature for none.
pub fn listen(channel: Sender<Event>) -> bool {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();

    let birth = std::thread::Builder::new()
        .name("notifications".into())
        .spawn(move || {
            let server = Server {
                channel,
                next_id: Arc::new(AtomicU32::new(1)),
                _guard: Mutex::new(()),
            };

            let connection = zbus::blocking::connection::Builder::session()
                .and_then(|b| b.name(NAME))
                .and_then(|b| b.serve_at(PATH, server))
                .and_then(zbus::blocking::connection::Builder::build);

            match connection {
                Ok(c) => {
                    // BEFORE announcing success: the footer may answer the bus
                    // the moment it learns there is one, and a `None` here
                    // would look exactly like "no bus at all".
                    let _ = BUS.set(c.clone());
                    let _ = ready_tx.send(true);
                    tracing::info!(name = NAME, "the notification daemon is on the bus");
                    // The connection has to LIVE: dropping it undoes the name's
                    // registration, and the bus starts refusing the messages with
                    // nothing here noticing. Sleeping is the way to keep the
                    // thread and the object alive without burning CPU — zbus serves
                    // the calls on its own threads.
                    loop {
                        std::thread::park();
                        // `park` may return spontaneously; the `_` keeps
                        // the connection alive for the whole scope.
                        let _ = &c;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "no notification daemon (is another already on the bus?)"
                    );
                    let _ = ready_tx.send(false);
                }
            }
        });

    if birth.is_err() {
        return false;
    }
    // It waits for the verdict: returning `true` before knowing would make the caller
    // think there is a daemon when the name was refused.
    ready_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap_or(false)
}

/// Tells the sender that one of ITS buttons was pressed.
///
/// The bus is the only way back: an application that offered an action is
/// blocked on this signal, and without it the button is a button that does
/// nothing — which is worse than not having offered it.
///
/// Silent when there is no bus. The footer runs perfectly well without one, and
/// a warning per click would be noise about a situation that is not a fault.
pub fn action_was_invoked(id: u32, action: &str) {
    let Some(c) = BUS.get() else { return };
    if let Err(e) = c.emit_signal(None::<&str>, PATH, NAME, "ActionInvoked", &(id, action)) {
        tracing::warn!(id, action, error = %e, "could not report the action");
    }
}

/// Tells the sender that the notification is gone, and why.
///
/// The specification numbers the reasons, and clients act on them: a progress
/// notification that sees `Expired` will send another, and one that sees
/// `ByHand` will stop. Sending the wrong number is worse than sending none.
pub fn notification_was_closed(id: u32, reason: Closed) {
    let Some(c) = BUS.get() else { return };
    let n = reason as u32;
    if let Err(e) = c.emit_signal(None::<&str>, PATH, NAME, "NotificationClosed", &(id, n)) {
        tracing::warn!(id, reason = n, error = %e, "could not announce the closing");
    }
}
