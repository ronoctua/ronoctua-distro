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
/// # The order is by how much the type TELLS US
///
/// The two that name UTF-8 come first, because a declared encoding needs no
/// interpretation at all. Then UTF-16, in its three spellings — the encoding
/// Java, Qt on Windows and Wine offer, and the one the owner asked about by
/// name. Then `text/plain` bare, which every modern application means as
/// UTF-8. Last `STRING`, which is Latin-1 by the ICCCM: exact, but it can only
/// carry 256 characters, so preferring it over a UTF-8 offer of the same
/// selection would throw away every accent on purpose.
///
/// `COMPOUND_TEXT` is deliberately NOT here. It is stateful, with embedded
/// escape sequences that switch character set mid-string; accepting it and
/// decoding it as anything simpler would put those sequences on screen as
/// letters. An application offering only that is refused, and the refusal is
/// reported — which is the honest end of a type nothing here can read.
///
/// Which one of these actually arrives is also what tells
/// `ronoctua_clipboard::decode` how to read the bytes, so the two lists cannot
/// drift: this is the list, and it is passed on with the payload.
const TEXT_KINDS: [&str; 8] = [
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain;charset=utf-16",
    "text/plain;charset=utf-16le",
    "text/plain;charset=utf-16be",
    "text/plain",
    "text/plain;charset=iso-8859-1",
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
/// It is the same criterion as the control socket and the compositor's IPC.
pub fn watch(channel: Sender<ronoctua_clipboard::Entry>) -> bool {
    let Ok(conn) = Connection::connect_to_env() else {
        tracing::warn!("no Wayland connection for the clipboard watcher");
        return false;
    };
    let Ok((globals, mut queue)) = registry_queue_init::<Watcher>(&conn) else {
        tracing::warn!("the Wayland registry was refused in the clipboard watcher");
        return false;
    };
    let qh = queue.handle();

    let manager: ZwlrDataControlManagerV1 = match globals.bind(&qh, 1..=2, ()) {
        Ok(g) => g,
        Err(e) => {
            // A compositor with no `zwlr_data_control` has no way of offering a
            // history to anyone — and it is honest to say so in the log instead of
            // staying silent waiting for events that never come.
            tracing::warn!(error = %e, "the compositor does not expose zwlr_data_control; no history");
            return false;
        }
    };
    let seat: wl_seat::WlSeat = match globals.bind(&qh, 1..=9, ()) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "no seat for the clipboard watcher");
            return false;
        }
    };
    let _device = manager.get_data_device(&seat, &qh, ());

    let mut state = Watcher {
        channel: Some(channel),
        ..Watcher::default()
    };
    std::thread::Builder::new()
        .name("clipboard".into())
        .spawn(move || {
            loop {
                if queue.blocking_dispatch(&mut state).is_err() {
                    tracing::info!("the clipboard watcher shut down");
                    return;
                }
            }
        })
        .expect("the watcher's thread cannot fail to be born");
    true
}

impl Watcher {
    /// Reads the offer that became the selection and sends it through the channel.
    fn collect_from(&mut self, offer: &ZwlrDataControlOfferV1, conn: &Connection) {
        let kinds = self.kinds.remove(offer).unwrap_or_default();

        // A SECRET DOES NOT GET PAST HERE. It is the only point where the filter
        // has the right answer: after reading, the text has already existed in the
        // process's memory. See `ronoctua_clipboard::Entry::is_secret`.
        if ronoctua_clipboard::Entry::is_secret(&kinds) {
            tracing::debug!("a copy from a password manager is ignored");
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
            tracing::debug!(?kinds, "a copy of an unsupported type is ignored");
            return;
        };
        let is_image = ronoctua_clipboard::image::TYPES.contains(&kind);

        let Ok((read_end, write_end)) = std::io::pipe() else {
            tracing::warn!("could not open the pipe to read the selection");
            return;
        };
        offer.receive(kind.to_owned(), write_end.as_fd());
        // CLOSE OUR WRITE END before reading. While it exists here, the EOF never
        // arrives — and the read would wait forever.
        drop(write_end);
        // FLUSH: the `receive` only asks; without sending the request, the other
        // side writes nothing.
        if let Err(e) = conn.flush() {
            tracing::warn!(error = %e, "the flush before reading the selection failed");
            return;
        }

        // BYTES, and not `read_to_string`. An image is not UTF-8, and reading it as
        // text would discard it with an error that looks like a read failure.
        let mut raw = Vec::new();
        let mut read_end = read_end;
        if let Err(e) = read_end.read_to_end(&mut raw) {
            // Whoever copied may have died before writing. It is not the user's
            // fault.
            tracing::debug!(error = %e, "could not read the selection");
            return;
        }

        let content = if is_image {
            ronoctua_clipboard::Content::Image(raw)
        } else {
            // THE TYPE SAYS HOW TO READ THE BYTES, and until this was written
            // the answer was always UTF-8: a copy from an application that
            // offers UTF-16 was dropped here, with a `debug!` line as the only
            // trace. Nothing on screen, nothing in the history, no way to tell
            // it from a copy that never happened.
            match ronoctua_clipboard::decode(kind, &raw) {
                Ok(t) => ronoctua_clipboard::Content::Text(t),
                Err(no) => {
                    // REFUSAL IS A VALUE. It goes down the same channel as an
                    // entry, so the footer can say what arrived and could not
                    // be read — a blank history and a refused type look the
                    // same on screen, and only one of them is worth a word.
                    tracing::debug!(
                        kind = no.kind,
                        bytes = no.bytes,
                        "the selection claims to be text in an encoding nothing here reads"
                    );
                    if let Some(c) = &self.channel {
                        let _ = c.send(ronoctua_clipboard::Entry {
                            content: ronoctua_clipboard::Content::Unreadable(no),
                            kinds,
                            pinned: false,
                        });
                    }
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
        offer: &ZwlrDataControlOfferV1,
        event: zwlr_data_control_offer_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_data_control_offer_v1::Event::Offer { mime_type } = event {
            state
                .kinds
                .entry(offer.clone())
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
