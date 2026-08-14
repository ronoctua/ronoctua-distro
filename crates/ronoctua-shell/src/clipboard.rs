//! The clipboard watcher — the one that sees what was copied.
//!
//! # Why it is NOT `wl_data_device`
//!
//! Because `wl_data_device.selection` only reaches the client that has
//! **keyboard focus** on that seat. The footer at rest does not capture the
//! keyboard — if it did, no buffer would receive a key (§7.4) —, so it would
//! never have focus and would never see a copy. That is how the first version
//! of this went by: the device was created, nothing gave an error, and the
//! history stayed empty forever.
//!
//! The right protocol is **`zwlr_data_control_unstable_v1`**, which exists
//! exactly for this — its own description says "take the role of a clipboard
//! manager". It delivers the selection without requiring focus, and it is what
//! `wl-paste --watch` and the clipboard managers use underneath.
//!
//! # Why its OWN CONNECTION, in a thread
//!
//! Two reasons, and each one holds on its own:
//!
//! 1. **Reading an offer blocks.** Whoever writes on the other side is the
//!    program that copied, and it writes whenever it likes. A `read` in the
//!    main loop would freeze the footer — the same class as the
//!    `wait_with_output` that hung the buffer on copying.
//! 2. **The `Footer`'s `Dispatch` is generic.** SCTK's `delegate_dispatch2!`
//!    implements `Dispatch<I, U>` for every proxy whose `U` speaks
//!    `Dispatch2`; shoving a protocol of ours in there would fight that impl.
//!
//! It is the same shape as `ronoctua_notify::dbus`: a thread that listens and
//! sends through calloop's channel.

use std::io::Read as _;
use std::os::fd::AsFd as _;

use smithay_client_toolkit::reexports::calloop::channel::Sender;
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry, wl_seat},
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1::{self, ZwlrDataControlDeviceV1},
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_offer_v1::{self, ZwlrDataControlOfferV1},
};

/// What the watcher accepts, in ORDER OF PREFERENCE.
///
/// Explicit UTF-8 first, because it is the only one that declares the
/// encoding. `text/plain` with no charset is a guess that almost always gets
/// it right, and `STRING` is the X legacy that XWayland still emits.
const TEXT_KINDS: [&str; 4] = [
    "text/plain;charset=utf-8",
    "text/plain",
    "UTF8_STRING",
    "STRING",
];

/// The thread's state: the types of each live offer.
#[derive(Default)]
struct Watcher {
    /// The announced MIME types, per offer.
    ///
    /// The protocol sends the types BEFORE saying that the offer became the
    /// selection, so they need to be accumulated somewhere until then.
    kinds: std::collections::HashMap<ZwlrDataControlOfferV1, Vec<String>>,
    channel: Option<Sender<ronoctua_clipboard::Entry>>,
}

/// Brings up the thread that watches the clipboard.
///
/// It gives back `false` when there is no watching — **it is not fatal**:
/// without it the whole footer goes on working, only the history stays empty.
/// It is the same criterion as the control socket and niri's IPC.
pub fn watch(channel: Sender<ronoctua_clipboard::Entry>) -> bool {
    let Ok(conn) = Connection::connect_to_env() else {
        tracing::warn!("sem conexao Wayland para a vigia do clipboard");
        return false;
    };
    let Ok((globais, mut fila)) = registry_queue_init::<Watcher>(&conn) else {
        tracing::warn!("registry do Wayland recusado na vigia do clipboard");
        return false;
    };
    let qh = fila.handle();

    let manager: ZwlrDataControlManagerV1 = match globais.bind(&qh, 1..=2, ()) {
        Ok(g) => g,
        Err(e) => {
            // A compositor with no `zwlr_data_control` has no way of offering a
            // history to anyone — and it is honest to say so in the log instead of
            // staying silent waiting for events that never come.
            tracing::warn!(error = %e, "o compositor nao expoe zwlr_data_control; sem historico");
            return false;
        }
    };
    let seat: wl_seat::WlSeat = match globais.bind(&qh, 1..=9, ()) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "sem seat para a vigia do clipboard");
            return false;
        }
    };
    let _dispositivo = manager.get_data_device(&seat, &qh, ());

    let mut state = Watcher {
        channel: Some(channel),
        ..Watcher::default()
    };
    std::thread::Builder::new()
        .name("clipboard".into())
        .spawn(move || {
            loop {
                if fila.blocking_dispatch(&mut state).is_err() {
                    tracing::info!("a vigia do clipboard encerrou");
                    return;
                }
            }
        })
        .expect("a thread da vigia nao pode falhar em nascer");
    true
}

impl Watcher {
    /// Reads the offer that became the selection and sends it through the channel.
    fn collect_from(&mut self, oferta: &ZwlrDataControlOfferV1, conn: &Connection) {
        let kinds = self.kinds.remove(oferta).unwrap_or_default();

        // A SECRET DOES NOT GET PAST HERE. It is the only point where the filter
        // has the right answer: after reading, the text has already existed in the
        // process's memory. See `ronoctua_clipboard::Entry::is_secret`.
        if ronoctua_clipboard::Entry::is_secret(&kinds) {
            tracing::debug!("copia de gerenciador de senha ignorada");
            return;
        }
        // IMAGE BEFORE TEXT.
        //
        // Whoever copies an image almost always offers `text/plain` along with it —
        // a file name, or a URL. Choosing the text there would give an entry that
        // looks right and loses what was copied.
        let kind = ronoctua_clipboard::image::preferred_type(&kinds).or_else(|| {
            TEXT_KINDS
                .iter()
                .find(|t| kinds.iter().any(|d| d.eq_ignore_ascii_case(t)))
                .copied()
        });
        let Some(kind) = kind else {
            // The log records WHAT was refused: "nothing shows up" and "we refused
            // this type" are fixed in different places.
            tracing::debug!(?kinds, "copia de tipo nao suportado ignorada");
            return;
        };
        let is_image = ronoctua_clipboard::image::TYPES.contains(&kind);

        let Ok((leitura, escrita)) = std::io::pipe() else {
            tracing::warn!("nao consegui abrir o cano para ler a selecao");
            return;
        };
        oferta.receive(kind.to_owned(), escrita.as_fd());
        // CLOSE OUR WRITE END before reading. While it exists here, the EOF never
        // arrives — and the read would wait forever.
        drop(escrita);
        // FLUSH: the `receive` only asks; without sending the request, the other
        // side writes nothing.
        if let Err(e) = conn.flush() {
            tracing::warn!(error = %e, "flush antes de ler a selecao falhou");
            return;
        }

        // BYTES, and not `read_to_string`. An image is not UTF-8, and reading it as
        // text would discard it with an error that looks like a read failure.
        let mut raw = Vec::new();
        let mut leitura = leitura;
        if let Err(e) = leitura.read_to_end(&mut raw) {
            // Whoever copied may have died before writing. It is not the user's
            // fault.
            tracing::debug!(error = %e, "nao consegui ler a selecao");
            return;
        }

        let content = if is_image {
            ronoctua_clipboard::Content::Image(raw)
        } else {
            match String::from_utf8(raw) {
                Ok(t) => ronoctua_clipboard::Content::Text(t),
                Err(_) => {
                    tracing::debug!(kind, "a selecao dizia ser texto e nao e UTF-8");
                    return;
                }
            }
        };
        if let Some(c) = &self.channel {
            let _ = c.send(ronoctua_clipboard::Entry {
                content,
                kinds,
                pinned: false,
            });
        }
    }
}

impl Dispatch<ZwlrDataControlDeviceV1, ()> for Watcher {
    fn event(
        state: &mut Self,
        _: &ZwlrDataControlDeviceV1,
        event: zwlr_data_control_device_v1::Event,
        (): &(),
        conn: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            // The offer IS BORN here, and its types arrive afterwards, one by one.
            zwlr_data_control_device_v1::Event::DataOffer { id } => {
                state.kinds.insert(id, Vec::new());
            }
            zwlr_data_control_device_v1::Event::Selection { id: Some(o) } => {
                state.collect_from(&o, conn);
                o.destroy();
            }
            // An empty selection (someone cleared it) and X's PRIMARY selection, which
            // is the text one selects with the mouse. The primary changes on every drag
            // of a selection — keeping it would fill the history with junk.
            _ => {}
        }
    }

    fn event_created_child(
        _: u16,
        qh: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        qh.make_data::<ZwlrDataControlOfferV1, _>(())
    }
}

impl Dispatch<ZwlrDataControlOfferV1, ()> for Watcher {
    fn event(
        state: &mut Self,
        oferta: &ZwlrDataControlOfferV1,
        event: zwlr_data_control_offer_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_data_control_offer_v1::Event::Offer { mime_type } = event {
            state
                .kinds
                .entry(oferta.clone())
                .or_default()
                .push(mime_type);
        }
    }
}

impl Dispatch<ZwlrDataControlManagerV1, ()> for Watcher {
    fn event(
        _: &mut Self,
        _: &ZwlrDataControlManagerV1,
        _: <ZwlrDataControlManagerV1 as wayland_client::Proxy>::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Watcher {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Watcher {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
