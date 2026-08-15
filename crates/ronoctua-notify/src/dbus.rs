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

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use zbus::zvariant::Value;

use crate::{Notification, Urgency};

/// The name the daemon claims on the session bus.
pub const NAME: &str = "org.freedesktop.Notifications";
/// The object path, fixed by the specification.
pub const PATH: &str = "/org/freedesktop/Notifications";

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
        _actions: Vec<String>,
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

        let n = Notification::fresh_one(
            id,
            app_name,
            Some(app_icon).filter(|s| !s.is_empty()),
            summary,
            body,
            urgency,
            expire_timeout,
        );
        tracing::info!(id, urgency = ?n.urgency, app = %n.app, "notificacao recebida");

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
    /// We announce only what we DELIVER. `actions` is left out until the buttons
    /// exist, and so is `persistence` — announcing a capability that does not
    /// exist makes the client rely on behaviour that never comes.
    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into()]
    }

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
    let (pronto_tx, pronto_rx) = std::sync::mpsc::channel();

    let nascimento = std::thread::Builder::new()
        .name("notificacoes".into())
        .spawn(move || {
            let servidor = Server {
                channel,
                next_id: Arc::new(AtomicU32::new(1)),
                _guard: Mutex::new(()),
            };

            let connection = zbus::blocking::connection::Builder::session()
                .and_then(|b| b.name(NAME))
                .and_then(|b| b.serve_at(PATH, servidor))
                .and_then(zbus::blocking::connection::Builder::build);

            match connection {
                Ok(c) => {
                    let _ = pronto_tx.send(true);
                    tracing::info!(name = NAME, "daemon de notificacoes no barramento");
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
                        "sem daemon de notificacoes (outro ja no barramento?)"
                    );
                    let _ = pronto_tx.send(false);
                }
            }
        });

    if nascimento.is_err() {
        return false;
    }
    // It waits for the verdict: returning `true` before knowing would make the caller
    // think there is a daemon when the name was refused.
    pronto_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap_or(false)
}
