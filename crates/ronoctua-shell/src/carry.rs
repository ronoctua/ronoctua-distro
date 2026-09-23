//! ENTRIES CARRIED OUT OF THE LISTING, and what a drop lands on.
//!
//! # Why this is `wl_data_device` and not something of our own
//!
//! Because the moment a drag begins the COMPOSITOR takes the pointer: our
//! surface stops receiving motion and starts receiving `wl_data_device.enter`,
//! `motion` and `drop`. There is no "inside the window" drag to write
//! separately — dropping on our own surface is the case where the destination
//! happens to be us, and it arrives through exactly the same four events as a
//! drop from Firefox.
//!
//! That is also what makes this worth doing rather than simulating: the same
//! twenty lines that let the manager drop into its own sidebar let it drop
//! into another window of itself, into a terminal, and into any program that
//! reads `text/uri-list` — which is every file manager, every browser and every
//! toolkit.
//!
//! # Why `text/uri-list` and nothing else
//!
//! It is the freedesktop standard for exactly this and what GTK and Qt both
//! offer and accept. A private type would be a drag that only works between two
//! of our own windows, which is the half of the feature nobody notices missing
//! until they try it.
//!
//! # What this module is NOT
//!
//! It is not a clipboard. `crate::clipboard` reads the SELECTION, and it uses
//! `zwlr_data_control` for a reason written out at length there: the footer
//! never has keyboard focus, and `wl_data_device.selection` only reaches the
//! client that does. A DRAG is different — it follows the POINTER — so the
//! standard protocol is the right one here and the wrong one there.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use smithay_client_toolkit::data_device_manager::{
    DataDeviceManagerState, ReadPipe, WritePipe, data_device::DataDevice, data_source::DragSource,
};
use smithay_client_toolkit::globals::GlobalData;
use smithay_client_toolkit::reexports::client::{
    Dispatch, QueueHandle,
    globals::GlobalList,
    protocol::{
        wl_data_device::WlDataDevice, wl_data_device_manager::DndAction,
        wl_data_source::WlDataSource, wl_seat::WlSeat, wl_surface::WlSurface,
    },
};

/// The one type this offers and accepts.
pub const URI_LIST: &str = "text/uri-list";

/// HOW LONG A DROP MAY TAKE TO ARRIVE.
///
/// Whoever writes the other end of the pipe is the program that was dragged
/// FROM, and it writes whenever it likes. A read with no deadline in the event
/// loop is the shell frozen by another application's slow morning — the same
/// class as the `wait_with_output` that once hung a buffer on a copy, and the
/// reason `motion.rs` polls with a deadline rather than blocking.
///
/// A quarter of a second. A `text/uri-list` is a few hundred bytes and lives
/// in the writer's pipe buffer already; anything that has not arrived by then
/// is not going to.
const PATIENCE: std::time::Duration = std::time::Duration::from_millis(250);

/// The drag-and-drop machinery a door owns.
///
/// One per door, because a `wl_data_device` is per SEAT and per CLIENT and each
/// door is its own client. What it holds is the manager, ONE DEVICE PER SEAT,
/// and the source while a drag of ours is running.
///
/// Per seat because every cursor is a seat (`crate::seats`), and a drag is
/// started on the device of the seat whose button was pressed — the serial
/// belongs to that seat, and the compositor refuses a drag begun on another
/// seat's device with nothing said on this side. One device, replaced by
/// whichever seat arrived last, was exactly that refusal for every cursor but
/// the last.
#[derive(Debug)]
pub struct Carrying {
    manager: DataDeviceManagerState,
    devices: Vec<(WlSeat, DataDevice)>,
    source: Option<DragSource>,
    /// The paths this door is OFFERING, frozen when the drag began.
    ///
    /// Frozen because the marks can move while the compositor is carrying:
    /// a re-read prunes them, and what was picked up must be what is dropped.
    offering: Vec<PathBuf>,
}

impl Carrying {
    /// Binds the manager, or answers `None` where the compositor has none.
    ///
    /// ABSENCE IS NOT AN ERROR, which is the rule the coverage fonts and the
    /// atlas already follow: a compositor without `wl_data_device_manager`
    /// gives a manager that cannot drag, and everything else goes on working.
    pub fn bind<D>(globals: &GlobalList, qh: &QueueHandle<D>) -> Option<Self>
    where
        D: Dispatch<
                smithay_client_toolkit::reexports::client::protocol::wl_data_device_manager::WlDataDeviceManager,
                GlobalData,
            > + 'static,
    {
        match DataDeviceManagerState::bind(globals, qh) {
            Ok(manager) => Some(Self {
                manager,
                devices: Vec::new(),
                source: None,
                offering: Vec::new(),
            }),
            Err(e) => {
                tracing::debug!(error = %e, "no data device manager; nothing can be dragged");
                None
            }
        }
    }

    /// The seat arrived, so the device can be made.
    pub fn took_a_seat<D>(&mut self, qh: &QueueHandle<D>, seat: &WlSeat)
    where
        D: Dispatch<
                WlDataDevice,
                smithay_client_toolkit::data_device_manager::data_device::DataDeviceData,
            > + 'static,
    {
        use smithay_client_toolkit::reexports::client::Proxy as _;
        if self.devices.iter().any(|(s, _)| s.id() == seat.id()) {
            return;
        }
        let device = self.manager.get_data_device(qh, seat);
        self.devices.push((seat.clone(), device));
    }

    /// STARTS A DRAG, with the paths frozen and the action the modifier asked
    /// for.
    ///
    /// `serial` is the PRESS's own, which is what the protocol requires: a
    /// drag begun from any other serial is refused by the compositor with no
    /// error anywhere on this side.
    pub fn begin<D>(
        &mut self,
        qh: &QueueHandle<D>,
        seat: Option<&WlSeat>,
        surface: &WlSurface,
        serial: u32,
        paths: Vec<PathBuf>,
        copies: bool,
    ) where
        D: Dispatch<
                WlDataSource,
                smithay_client_toolkit::data_device_manager::data_source::DataSourceData<()>,
            > + 'static,
    {
        use smithay_client_toolkit::reexports::client::Proxy as _;
        // THE PRESSING SEAT'S DEVICE — see the struct.
        let device = seat
            .and_then(|seat| self.devices.iter().find(|(s, _)| s.id() == seat.id()))
            .or_else(|| self.devices.first())
            .map(|(_, d)| d);
        let Some(device) = device else {
            return;
        };
        if paths.is_empty() {
            return;
        }
        // BOTH ACTIONS ARE OFFERED AND ONE IS PREFERRED, because the
        // destination has a say: dropping into a different filesystem, or into
        // a program that can only copy, has to work. What the modifier decides
        // is what we ASK for.
        let wanted = if copies {
            DndAction::Copy
        } else {
            DndAction::Move | DndAction::Copy
        };
        let source = self
            .manager
            .create_drag_and_drop_source(qh, [URI_LIST], wanted);
        source.start_drag(device, surface, None, serial);
        // WHAT LEFT THE LISTING, in the log. A drag is the one gesture whose
        // whole middle belongs to the compositor: between the press and the
        // drop nothing on this side can be photographed, so the two ends say
        // what they were.
        tracing::info!(carrying = paths.len(), copies, "a drag of files begins");
        self.offering = paths;
        self.source = Some(source);
    }

    /// Writes the list the compositor asked for, and closes the pipe.
    pub fn send(&self, mime: &str, mut pipe: WritePipe) {
        if mime != URI_LIST {
            return;
        }
        let said = uri_list(&self.offering);
        if let Err(e) = pipe.write_all(said.as_bytes()) {
            tracing::debug!(error = %e, "the drag's list could not be written");
        }
    }

    /// WHAT THIS DOOR IS OFFERING, when the drag is its own.
    ///
    /// # Why a drop on ourselves does not read the pipe
    ///
    /// Because it cannot. `wl_data_offer.receive` hands back a read end and
    /// asks the COMPOSITOR to tell the source to write — and when the source
    /// is this same client, that instruction arrives as a Wayland EVENT.
    /// Reading inside `drop_performed` blocks the very loop that would deliver
    /// it, so the read times out, the fd closes, and the write that finally
    /// runs finds a broken pipe.
    ///
    /// Measured, in exactly that order: `files were let go over the listing
    /// dropped=0`, then `the drag's list could not be written error=Broken
    /// pipe`. Nothing was wrong with the encoding or with the protocol — a
    /// single-threaded client that is both ends of a drag has to take the
    /// short way round, and it already holds the answer.
    ///
    /// `None` when the drag belongs to somebody else, and then the pipe is the
    /// only source of truth — read with a deadline, which is what makes a slow
    /// foreign writer cost a quarter of a second instead of the session.
    #[must_use]
    pub fn offering(&self) -> Option<&[PathBuf]> {
        self.source.as_ref().map(|_| self.offering.as_slice())
    }

    /// The drag is over, whether it landed or not.
    pub fn done(&mut self) {
        self.source = None;
        self.offering.clear();
    }
}

/// READS A `text/uri-list` FROM AN OFFER, with a deadline.
///
/// `None` when nothing readable arrived in time — which is an answer and not a
/// failure: the drop simply does nothing, and the line says so.
#[must_use]
pub fn read(pipe: ReadPipe) -> Option<Vec<PathBuf>> {
    use std::os::fd::AsFd as _;
    let mut pipe = pipe;
    let mut said = Vec::new();
    let mut chunk = [0_u8; 4096];
    let until = std::time::Instant::now() + PATIENCE;
    let fd = pipe.as_fd().try_clone_to_owned().ok()?;
    loop {
        let left = until.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            break;
        }
        let mut poll = [rustix::event::PollFd::new(
            &fd,
            rustix::event::PollFlags::IN,
        )];
        #[allow(clippy::cast_possible_truncation)]
        let ready = rustix::event::poll(&mut poll, Some(&left.try_into().ok()?)).ok()?;
        if ready == 0 {
            break;
        }
        match pipe.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => said.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&said);
    let paths = paths_of(&text);
    (!paths.is_empty()).then_some(paths)
}

/// THE PATHS AS A `text/uri-list`.
///
/// `CRLF` between them and after the last, which is what RFC 2483 says and what
/// every other toolkit writes. A path is percent-encoded — see [`quoted`].
#[must_use]
pub fn uri_list(paths: &[PathBuf]) -> String {
    let mut out = String::new();
    for path in paths {
        out.push_str("file://");
        out.push_str(&quoted(path));
        out.push_str("\r\n");
    }
    out
}

/// THE PATHS A `text/uri-list` NAMES, and only the ones on this machine.
///
/// A line beginning with `#` is a comment by the RFC, and anything that is not
/// a `file:` URI is somebody else's resource — a web address dropped from a
/// browser is not a file, and treating it as one would make a path out of a
/// URL, which is the one thing that must not happen to it.
#[must_use]
pub fn paths_of(text: &str) -> Vec<PathBuf> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.strip_prefix("file://"))
        // AND THE AUTHORITY IS DROPPED ONLY WHEN IT IS THIS MACHINE.
        //
        // `file://host/path` names a file on `host`. Empty and `localhost` are
        // the two spellings of "here"; anything else is a machine we cannot
        // reach and must not pretend to.
        .filter_map(|rest| {
            let rest = rest.strip_prefix("localhost").unwrap_or(rest);
            rest.starts_with('/').then_some(rest)
        })
        .map(|rest| PathBuf::from(unquoted(rest)))
        .collect()
}

/// A path, percent-encoded for a URI.
///
/// # Why it is written out rather than taken from a crate
///
/// Because the rule is short and the alternative is a dependency for it. What
/// must be encoded is everything that is not an *unreserved* character or a
/// path separator: RFC 3986 leaves `A-Z a-z 0-9 - . _ ~` alone, and this keeps
/// `/` as well because it is the structure of the path rather than data in it.
/// Everything else — the space, the `#` that would start a fragment, the `%`
/// itself, and every byte of a name that is not ASCII — goes out as `%XX`.
fn quoted(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt as _;
    let mut out = String::new();
    for byte in path.as_os_str().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(char::from(*byte));
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// The other direction, byte by byte.
///
/// A `%` with anything but two hex digits after it is left alone rather than
/// dropped: a name really containing a per-cent sign, written by a program that
/// did not encode it, is a name we can still open.
fn unquoted(said: &str) -> std::ffi::OsString {
    use std::os::unix::ffi::OsStringExt as _;
    let raw = said.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%'
            && i + 2 < raw.len()
            && let Some(byte) = std::str::from_utf8(&raw[i + 1..=i + 2])
                .ok()
                .and_then(|two| u8::from_str_radix(two, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(raw[i]);
        i += 1;
    }
    std::ffi::OsString::from_vec(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_goes_out_and_comes_back_the_same() {
        // EVERY CHARACTER A NAME IS ALLOWED TO HAVE, which is every byte but
        // `/` and NUL — the space and the `#` most of all, because one ends a
        // field in half the formats there are and the other starts a comment
        // in this one.
        let paths = vec![
            PathBuf::from("/home/someone/a file.txt"),
            PathBuf::from("/home/someone/100% done#1.txt"),
            PathBuf::from("/home/someone/ação.txt"),
        ];
        let said = uri_list(&paths);
        assert!(
            said.lines().all(|l| l.starts_with("file:///")),
            "a line is not a file URI: {said:?}"
        );
        assert!(!said.contains(' '), "a space went out unencoded");
        assert_eq!(paths_of(&said), paths);
    }

    #[test]
    fn what_is_not_a_file_on_this_machine_is_not_a_path() {
        // A URL DROPPED FROM A BROWSER IS NOT A FILE, and making a path out of
        // one is the single thing that must not happen to a URL. Nor is a file
        // on another machine, which `file://host/path` names and which nothing
        // here can reach.
        let said = "# a comment\r\n\
                    https://example.invalid/page\r\n\
                    file://elsewhere/home/a.txt\r\n\
                    file://localhost/home/b.txt\r\n\
                    file:///home/c.txt\r\n";
        assert_eq!(
            paths_of(said),
            vec![PathBuf::from("/home/b.txt"), PathBuf::from("/home/c.txt")]
        );
    }
}
