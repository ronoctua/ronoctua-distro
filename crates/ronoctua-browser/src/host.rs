//! The native-messaging host Chrome starts for our extension.
//!
//! # Not a daemon of ours
//!
//! Chrome starts this process when the extension opens its port — which it
//! does only while a kiosk window exists — and ends it by closing the pipe.
//! Nothing here restarts, and nothing outlives the port: the end of stdin is
//! the end of the process.
//!
//! # Three hops, and this is the middle one
//!
//! The extension speaks Chrome's framing on stdin and stdout; the footer
//! listens on a Unix socket, one JSON object per line
//! ([`crate::serve`]). The host relays between the two, and adds the one thing
//! only a process beside the profile can read: which extensions are pinned and
//! which page each one's button opens ([`crate::profile`]).
//!
//! # A footer that restarts is found again
//!
//! The socket thread connects, and when there is no footer it waits for the
//! socket to APPEAR — inotify on the runtime directory, never a poll. On every
//! connection it re-sends the last whole state, so a footer that restarted
//! draws the kiosk at once instead of waiting for the next tab to change. The
//! extension is told `footer up` or `down` each time, so a kiosk page born
//! while nobody listens leaves for its address at once.
//!
//! # Checked here too
//!
//! An order carrying an address is checked before it is relayed
//! ([`crate::url`]): a `javascript:` address sent to a tab runs script in the
//! page, and the footer is not the only program that could reach the socket
//! in principle. The extension checks again.

use crate::native;
use crate::profile::Reader;
use crate::wire::{Done, Envelope, HostSaid, Order, Said};
use anyhow::Result;
use rustix::fs::inotify;
use std::io::{BufRead as _, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// The largest frame the extension may send: the whole state of a profile's
/// kiosks. A few hundred tabs fit many times over.
pub const CEILING: u32 = 4 * 1024 * 1024;

/// The longest line the footer may send. An order is an address at most.
const LINE_CEILING: u64 = 64 * 1024;

/// A socket that appeared is bound before it listens, and a connection in
/// between is refused: after the creation event, a few short tries. Bounded,
/// and only after an event — this is not a poll.
const SETTLE_TRIES: u32 = 20;
const SETTLE_STEP: Duration = Duration::from_millis(25);

/// A footer connection shorter than this ended as it was made — a footer
/// refusing what it was given, or dying as it starts — and the next try
/// waits ([`FIRST_PAUSE`], doubling up to [`LAST_PAUSE`]). One that lived
/// longer puts the pause back to nothing. Only after a connection ended:
/// this is not a poll either.
const QUICK: Duration = Duration::from_secs(1);
const FIRST_PAUSE: Duration = Duration::from_millis(100);
const LAST_PAUSE: Duration = Duration::from_secs(5);

/// The host: where frames go, and what it knows.
pub struct Host {
    /// Chrome's end: every frame to the extension goes through this lock, so
    /// two threads never interleave the bytes of two frames.
    out: Mutex<Box<dyn Write + Send>>,
    /// Everything else, under one lock.
    inner: Mutex<Inner>,
    /// `$XDG_CONFIG_HOME` (or `~/.config`), where the profiles are.
    config_home: Option<PathBuf>,
}

struct Inner {
    /// The last state the extension sent, as enriched — what a footer that
    /// connects is given first.
    last: Option<Said>,
    /// The footer's end, while one is connected.
    footer: Option<UnixStream>,
    /// The popup pages already read, by extension and version.
    reader: Reader,
}

impl Host {
    /// A host writing its frames to `out` and reading the profiles under
    /// `config_home`.
    #[must_use]
    pub fn new(out: Box<dyn Write + Send>, config_home: Option<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            out: Mutex::new(out),
            inner: Mutex::new(Inner {
                last: None,
                footer: None,
                reader: Reader::default(),
            }),
            config_home,
        })
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// One frame to the extension.
    fn to_extension<T: serde::Serialize>(&self, value: &T) {
        let mut out = self
            .out
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Err(e) = native::write_message(&mut *out, value) {
            // Chrome closed the pipe: stdin ends too, and that ends the host.
            tracing::warn!(error = %e, "a frame the extension could not be given");
        }
    }

    /// One line to the footer, when one is connected. A write that fails
    /// drops the connection; the socket thread notices the end and waits for
    /// the next footer.
    ///
    /// A LINE THE FOOTER WOULD REFUSE (`serve::CEILING`) is not sent at all:
    /// it would end the connection, and the next one would be given the same
    /// line again — a loop across three processes. The footer keeps the
    /// state it has, and the log says why.
    fn to_footer(inner: &mut Inner, said: &Said) {
        let Some(stream) = inner.footer.as_mut() else {
            return;
        };
        let written = serde_json::to_vec(said)
            .map_err(std::io::Error::from)
            .and_then(|mut line| {
                line.push(b'\n');
                if line.len() > crate::serve::CEILING {
                    tracing::warn!(
                        bytes = line.len(),
                        "a line past the footer's ceiling was kept back"
                    );
                    return Ok(());
                }
                stream.write_all(&line)
            });
        if let Err(e) = written {
            tracing::warn!(error = %e, "the footer went away mid-line");
            let _ = stream.shutdown(std::net::Shutdown::Both);
            inner.footer = None;
        }
    }

    /// What the extension said: a state is enriched, kept and relayed; an
    /// answer is relayed. Returns the state's profile, for the watch on its
    /// `Preferences`.
    fn heard_from_extension(&self, frame: serde_json::Value) -> Option<String> {
        let said = match serde_json::from_value::<Said>(frame) {
            Ok(said) => said,
            Err(e) => {
                // A newer extension's word: said in the log, never guessed at.
                tracing::warn!(error = %e, "a frame from the extension that is not ours");
                return None;
            }
        };
        let mut inner = self.inner();
        match said {
            Said::State(mut state) => {
                if let Some(home) = &self.config_home {
                    inner.reader.enrich(&mut state, home);
                }
                let profile = state.profile.clone();
                let said = Said::State(state);
                Self::to_footer(&mut inner, &said);
                inner.last = Some(said);
                profile
            }
            done @ Said::Done(_) => {
                Self::to_footer(&mut inner, &done);
                None
            }
        }
    }

    /// The profile's preferences changed: the last state, enriched again,
    /// goes to the footer when it came out different.
    pub fn preferences_changed(&self) {
        let Some(home) = &self.config_home else {
            return;
        };
        let mut inner = self.inner();
        let Some(Said::State(state)) = inner.last.clone() else {
            return;
        };
        let mut fresh = state.clone();
        inner.reader.enrich(&mut fresh, home);
        if fresh != state {
            let said = Said::State(fresh);
            Self::to_footer(&mut inner, &said);
            inner.last = Some(said);
        }
    }

    /// Reads the extension until Chrome closes the pipe. `on_profile` hears
    /// each state's profile (the caller watches its files).
    ///
    /// # Errors
    ///
    /// When the framing breaks (a length past [`CEILING`], a short frame, a
    /// frame that is not JSON): the stream cannot be resynchronised, and the
    /// extension opens a new port.
    pub fn relay_extension(
        &self,
        mut input: impl Read,
        mut on_profile: impl FnMut(&str),
    ) -> Result<()> {
        while let Some(frame) =
            native::read_message_within::<serde_json::Value>(&mut input, CEILING)?
        {
            if let Some(profile) = self.heard_from_extension(frame) {
                on_profile(&profile);
            }
        }
        Ok(())
    }

    /// One line from the footer: an order, relayed when it is one this build
    /// knows and its address is one a kiosk may open; otherwise refused back
    /// to the footer by its serial, and never relayed.
    fn heard_from_footer(&self, line: &[u8]) {
        let refuse = |serial: u64, why: &str| {
            let mut inner = self.inner();
            let done = Said::Done(Done {
                serial,
                ok: false,
                why: Some(why.to_owned()),
            });
            Self::to_footer(&mut inner, &done);
        };
        let envelope = match serde_json::from_slice::<Envelope>(line) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(error = %e, "a line from the footer that is not an order");
                if let Some(serial) = serde_json::from_slice::<serde_json::Value>(line)
                    .ok()
                    .and_then(|v| v.get("serial")?.as_u64())
                {
                    refuse(serial, "refused-order");
                }
                return;
            }
        };
        if !allowed(&envelope.order) {
            tracing::warn!(
                serial = envelope.serial,
                "an order with an address a kiosk may not open"
            );
            refuse(envelope.serial, "refused-url");
            return;
        }
        self.to_extension(&envelope);
    }

    /// Serves one footer connection until it ends: the last state first, the
    /// extension told the footer is up, then every order it sends; at the end,
    /// the extension is told it is down.
    pub fn serve_footer(&self, stream: UnixStream) {
        if let Some(reading) = self.attach(stream) {
            self.read_footer(reading);
        }
    }

    /// A footer connected: it becomes the one states go to, is given the
    /// last state whole, and the extension hears it is up. Returns the
    /// connection's reading end.
    fn attach(&self, stream: UnixStream) -> Option<UnixStream> {
        let reading = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "the footer's connection could not be read");
                return None;
            }
        };
        {
            let mut inner = self.inner();
            inner.footer = Some(stream);
            if let Some(last) = inner.last.clone() {
                Self::to_footer(&mut inner, &last);
            }
        }
        tracing::info!("the footer is connected");
        self.to_extension(&HostSaid::Footer { up: true });
        Some(reading)
    }

    /// Every order of one footer connection, until it ends; then the
    /// extension hears the footer is down.
    fn read_footer(&self, reading: UnixStream) {
        let mut reader = BufReader::new(reading);
        let mut line = Vec::new();
        loop {
            line.clear();
            match (&mut reader)
                .take(LINE_CEILING + 1)
                .read_until(b'\n', &mut line)
            {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if u64::try_from(line.len()).unwrap_or(u64::MAX) > LINE_CEILING {
                tracing::warn!("the footer sent a line past the ceiling");
                break;
            }
            let trimmed = line.trim_ascii();
            if !trimmed.is_empty() {
                self.heard_from_footer(trimmed);
            }
        }
        if let Some(stream) = self.inner().footer.take() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        tracing::info!("the footer went away");
        self.to_extension(&HostSaid::Footer { up: false });
    }

    /// Connects to the footer's socket for as long as the process lives:
    /// served while it answers, awaited (by inotify) while it does not.
    pub fn footer_loop(&self, path: &Path) {
        self.footer_loop_from(path, None, false);
    }

    /// [`Host::footer_loop`], starting from a first attempt already made:
    /// `first` is its connection, and `said_down` whether the extension was
    /// already told there is none.
    fn footer_loop_from(&self, path: &Path, mut first: Option<UnixStream>, mut said_down: bool) {
        let mut pause = Duration::ZERO;
        loop {
            if let Some(stream) = first.take().or_else(|| connect(path)) {
                said_down = false;
                let since = std::time::Instant::now();
                self.serve_footer(stream);
                // A CONNECTION THAT ENDED AS IT WAS MADE is not called back
                // at once: each turn would resend the state, make the
                // extension take a new one and flip it between "footer up"
                // and "down" — which lets every kiosk being born go unpaired.
                pause = next_pause(pause, since.elapsed());
                std::thread::sleep(pause);
                continue;
            }
            if !said_down {
                self.to_extension(&HostSaid::Footer { up: false });
                said_down = true;
            }
            match wait_for(path) {
                Ok(stream) => first = Some(stream),
                Err(e) => {
                    tracing::warn!(error = %e, "the footer's socket cannot be awaited; the kiosk stays unpaired");
                    return;
                }
            }
        }
    }
}

/// The pause before the next connection, after one that lasted `lasted`:
/// nothing after one that lived, and after one that ended as it was made,
/// twice the last pause, from [`FIRST_PAUSE`] up to [`LAST_PAUSE`].
fn next_pause(last: Duration, lasted: Duration) -> Duration {
    if lasted >= QUICK {
        Duration::ZERO
    } else {
        (last * 2).clamp(FIRST_PAUSE, LAST_PAUSE)
    }
}

/// Whether an order's address, if it has one, is one a kiosk may be sent to.
fn allowed(order: &Order) -> bool {
    match order {
        Order::Navigate { url, .. } => crate::url::allowed(url),
        Order::Create { url, .. } => url.as_deref().is_none_or(crate::url::allowed),
        Order::OpenExtension { url, .. } => crate::url::extension_page(url),
        _ => true,
    }
}

/// A connection to the footer, or `None` when nobody answers there.
fn connect(path: &Path) -> Option<UnixStream> {
    UnixStream::connect(path).ok()
}

/// Waits until the socket at `path` answers, and returns THAT connection: a
/// watch on its directory, one more try (the footer may have come up before
/// the watch did), then every creation of that name — each followed by a few
/// short tries, because a socket file exists a moment before it listens. A
/// probe that connected and hung up would be a host the footer saw come and
/// go for nothing.
fn wait_for(path: &Path) -> Result<UnixStream> {
    let dir = path.parent().unwrap_or_else(|| Path::new("/"));
    let leaf = path
        .file_name()
        .map(std::ffi::OsStr::as_encoded_bytes)
        .unwrap_or_default()
        .to_vec();
    let fd = inotify::init(inotify::CreateFlags::CLOEXEC)?;
    inotify::add_watch(
        &fd,
        dir,
        inotify::WatchFlags::CREATE | inotify::WatchFlags::MOVED_TO | inotify::WatchFlags::ONLYDIR,
    )?;
    if let Some(stream) = connect(path) {
        return Ok(stream);
    }
    let mut buffer = [std::mem::MaybeUninit::<u8>::uninit(); 4096];
    loop {
        let mut reader = inotify::Reader::new(&fd, &mut buffer);
        let mut ours = false;
        loop {
            match reader.next() {
                Ok(event) => {
                    ours |= event
                        .file_name()
                        .is_some_and(|n| n.to_bytes() == leaf.as_slice());
                }
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(e.into()),
            }
            if reader.is_buffer_empty() {
                break;
            }
        }
        if ours {
            for _ in 0..SETTLE_TRIES {
                if let Some(stream) = connect(path) {
                    return Ok(stream);
                }
                std::thread::sleep(SETTLE_STEP);
            }
        }
    }
}

/// Watches a profile directory for Chrome rewriting its preferences
/// (Chrome writes a whole new file and renames it over the old one), and
/// calls `changed` after each.
///
/// # Errors
///
/// When the directory cannot be watched.
pub fn watch_preferences(dir: &Path, changed: impl Fn()) -> Result<()> {
    let fd = inotify::init(inotify::CreateFlags::CLOEXEC)?;
    inotify::add_watch(
        &fd,
        dir,
        inotify::WatchFlags::MOVED_TO
            | inotify::WatchFlags::CLOSE_WRITE
            | inotify::WatchFlags::ONLYDIR,
    )?;
    let mut buffer = [std::mem::MaybeUninit::<u8>::uninit(); 4096];
    loop {
        let mut reader = inotify::Reader::new(&fd, &mut buffer);
        let mut ours = false;
        loop {
            match reader.next() {
                Ok(event) => {
                    ours |= event.file_name().is_some_and(|n| {
                        crate::profile::PREFERENCE_FILES
                            .iter()
                            .any(|f| n.to_bytes() == f.as_bytes())
                    });
                }
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(e.into()),
            }
            if reader.is_buffer_empty() {
                break;
            }
        }
        if ours {
            changed();
        }
    }
}

/// Where the profiles live: `$XDG_CONFIG_HOME`, or `~/.config`.
#[must_use]
pub fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

/// The whole host, as Chrome runs it: the footer's socket on a thread, the
/// profile's preferences on another once a state names the profile, and the
/// extension's pipe here until Chrome closes it.
///
/// # Errors
///
/// When the extension's framing breaks.
pub fn run() -> Result<()> {
    let home = config_home();
    let host = Host::new(Box::new(std::io::stdout()), home.clone());
    match crate::serve::socket_path() {
        Some(path) => {
            // THE FIRST ATTEMPT HERE, before the pipe is read: a kiosk page
            // born with no footer is told so before anything else, and never
            // waits on a thread that has not run yet.
            let first = connect(&path);
            let said_down = first.is_none();
            if said_down {
                host.to_extension(&HostSaid::Footer { up: false });
            }
            let footer = Arc::clone(&host);
            std::thread::Builder::new()
                .name("browser-footer".into())
                .spawn(move || footer.footer_loop_from(&path, first, said_down))?;
        }
        None => {
            tracing::warn!("no runtime directory: there is no footer to reach");
            host.to_extension(&HostSaid::Footer { up: false });
        }
    }
    let mut watching: Option<String> = None;
    host.relay_extension(std::io::stdin().lock(), |profile| {
        if watching.is_some() {
            return;
        }
        let Some(dir) = home
            .as_deref()
            .and_then(|h| crate::profile::profile_dir(h, profile))
        else {
            return;
        };
        watching = Some(profile.to_owned());
        let host = Arc::clone(&host);
        let spawned = std::thread::Builder::new()
            .name("browser-preferences".into())
            .spawn(move || {
                if let Err(e) = watch_preferences(&dir, || host.preferences_changed()) {
                    tracing::warn!(error = %e, "the profile's preferences cannot be watched");
                }
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "the preferences watch did not start");
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{Extension, Kiosk, State, VERSION};

    /// A `Write` whose bytes a test can read back.
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("lock").extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Shared {
        fn frames(&self) -> Vec<serde_json::Value> {
            let bytes = self.0.lock().expect("lock").clone();
            let mut reading = bytes.as_slice();
            let mut out = Vec::new();
            while let Some(v) = native::read_message(&mut reading).expect("frames") {
                out.push(v);
            }
            out
        }
    }

    fn state(nonce: &str) -> State {
        State {
            v: VERSION,
            profile: Some("Default".into()),
            kiosks: vec![Kiosk {
                window: 4,
                nonce: nonce.into(),
                ..Kiosk::default()
            }],
            ..State::default()
        }
    }

    fn framed(values: &[serde_json::Value]) -> Vec<u8> {
        let mut out = Vec::new();
        for v in values {
            native::write_message(&mut out, v).expect("frame");
        }
        out
    }

    /// A footer connection: attached HERE, so the test knows it is the one
    /// states go to, and read on a thread until the test hangs up.
    fn serve(host: &Arc<Host>, ours: UnixStream) -> std::thread::JoinHandle<()> {
        let reading = host.attach(ours).expect("attached");
        let host = Arc::clone(host);
        std::thread::spawn(move || host.read_footer(reading))
    }

    fn read_line(from: &mut BufReader<UnixStream>) -> serde_json::Value {
        let mut line = String::new();
        from.read_line(&mut line).expect("line");
        serde_json::from_str(&line).expect("json")
    }

    #[test]
    fn the_end_of_stdin_ends_the_host() {
        let host = Host::new(Box::new(Shared::default()), None);
        host.relay_extension(&[][..], |_| {})
            .expect("an empty pipe is a clean end");
        // A frame that breaks the framing is an error, not a hang.
        let mut broken = 50_u32.to_le_bytes().to_vec();
        broken.extend_from_slice(b"{}");
        assert!(host.relay_extension(broken.as_slice(), |_| {}).is_err());
    }

    #[test]
    fn the_last_state_is_given_to_every_footer_that_connects() {
        let out = Shared::default();
        let host = Host::new(Box::new(out.clone()), None);
        let first = serde_json::to_value(Said::State(state("aa"))).expect("json");
        let second = serde_json::to_value(Said::State(state("bb"))).expect("json");
        let mut profiles = Vec::new();
        host.relay_extension(framed(&[first, second.clone()]).as_slice(), |p| {
            profiles.push(p.to_owned());
        })
        .expect("relay");
        assert_eq!(profiles, ["Default", "Default"]);

        for _ in 0..2 {
            let (ours, theirs) = UnixStream::pair().expect("pair");
            let thread = serve(&host, ours);
            let mut footer = BufReader::new(theirs);
            assert_eq!(read_line(&mut footer), second, "the LAST state, whole");
            drop(footer);
            thread.join().expect("served");
        }
        let said: Vec<_> = out.frames();
        assert_eq!(
            said,
            [
                serde_json::json!({"said": "footer", "up": true}),
                serde_json::json!({"said": "footer", "up": false}),
                serde_json::json!({"said": "footer", "up": true}),
                serde_json::json!({"said": "footer", "up": false}),
            ],
            "the extension hears the footer come and go"
        );
    }

    #[test]
    fn a_refused_address_never_reaches_the_extension() {
        let out = Shared::default();
        let host = Host::new(Box::new(out.clone()), None);
        let (ours, theirs) = UnixStream::pair().expect("pair");
        let thread = serve(&host, ours);
        let mut writing = theirs.try_clone().expect("clone");
        let mut footer = BufReader::new(theirs);
        for line in [
            r#"{"serial":1,"order":"navigate","tab":3,"url":"javascript:alert(1)"}"#,
            r#"{"serial":2,"order":"create","window":4,"url":"file:///etc/passwd"}"#,
            r#"{"serial":3,"order":"open-extension","window":4,"url":"https://evil.example/"}"#,
            r#"{"serial":4,"order":"format-disk"}"#,
            r#"{"serial":5,"order":"navigate","tab":3,"url":"https://a.example/"}"#,
            r#"{"serial":6,"order":"create","window":4}"#,
        ] {
            writing.write_all(line.as_bytes()).expect("write");
            writing.write_all(b"\n").expect("write");
        }
        for (serial, why) in [
            (1, "refused-url"),
            (2, "refused-url"),
            (3, "refused-url"),
            (4, "refused-order"),
        ] {
            assert_eq!(
                read_line(&mut footer),
                serde_json::json!({"said": "done", "serial": serial, "ok": false, "why": why})
            );
        }
        drop(writing);
        drop(footer);
        thread.join().expect("served");
        let frames = out.frames();
        let orders: Vec<_> = frames
            .iter()
            .filter(|f| f.get("serial").is_some())
            .collect();
        assert_eq!(
            orders,
            [
                &serde_json::json!({"serial": 5, "order": "navigate", "tab": 3, "url": "https://a.example/"}),
                &serde_json::json!({"serial": 6, "order": "create", "window": 4, "url": null}),
            ],
            "only the two good orders were relayed"
        );
        let all = serde_json::to_string(&frames).expect("json");
        assert!(!all.contains("javascript") && !all.contains("file://") && !all.contains("evil"));
    }

    #[test]
    fn answers_and_states_reach_the_footer_and_strangers_do_not() {
        let host = Host::new(Box::new(Shared::default()), None);
        let (ours, theirs) = UnixStream::pair().expect("pair");
        let thread = serve(&host, ours);
        let mut footer = BufReader::new(theirs);
        // A word this build does not know is left in the log; the answer
        // after it still arrives.
        let frames = framed(&[
            serde_json::json!({"said": "shout"}),
            serde_json::json!({"said": "done", "serial": 9, "ok": true}),
        ]);
        host.relay_extension(frames.as_slice(), |_| {})
            .expect("relay");
        assert_eq!(
            read_line(&mut footer),
            serde_json::json!({"said": "done", "serial": 9, "ok": true})
        );
        drop(footer);
        thread.join().expect("served");
    }

    #[test]
    fn a_footer_that_appears_later_is_found_without_polling() {
        let dir = std::env::temp_dir().join(format!("ronoctua-host-wait-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("browser.sock");
        let out = Shared::default();
        let host = Host::new(Box::new(out.clone()), None);
        host.relay_extension(
            framed(&[serde_json::to_value(Said::State(state("cc"))).expect("json")]).as_slice(),
            |_| {},
        )
        .expect("relay");
        let looping = Arc::clone(&host);
        let waiting = path.clone();
        std::thread::spawn(move || looping.footer_loop(&waiting));
        // The footer comes up AFTER the host found nobody there and said so —
        // which it does just before it starts waiting.
        for _ in 0..200 {
            if !out.frames().is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind");
        let (stream, _) = listener.accept().expect("the host connects");
        let mut footer = BufReader::new(stream);
        assert_eq!(read_line(&mut footer)["kiosks"][0]["nonce"], "cc");
        let told: Vec<_> = out.frames();
        assert_eq!(told[0], serde_json::json!({"said": "footer", "up": false}));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a frame the host takes whole — at its own ceiling — comes
    /// out of the enrichment as a line the footer refuses: the footer would
    /// drop the connection, and be given the same line on every reconnection.
    #[test]
    fn every_frame_the_host_takes_is_a_line_the_footer_takes() {
        // AS THE EXTENSION SENDS IT: no `popup`, no `pinned_at` — the host
        // writes both into every extension on the way.
        let extensions: Vec<serde_json::Value> = (0..2000)
            .map(|i| {
                serde_json::json!({
                    "id": format!("{i:a>32}"),
                    "name": "x",
                    "version": "1",
                    "enabled": true,
                    "options_url": null,
                })
            })
            .collect();
        let frame = |url: &str| {
            serde_json::json!({
                "said": "state",
                "v": VERSION,
                "profile": "Default",
                "kiosks": [{"window": 4, "nonce": "ff", "tabs": [{"id": 1, "index": 0, "url": url}]}],
                "extensions": extensions,
            })
        };
        let bare = serde_json::to_vec(&frame("")).expect("json").len();
        let limit = usize::try_from(CEILING).expect("fits");
        let url = "a".repeat(limit - bare - 64);
        let whole = frame(&url);
        assert!(serde_json::to_vec(&whole).expect("json").len() <= limit);

        let host = Host::new(Box::new(Shared::default()), None);
        let (ours, theirs) = UnixStream::pair().expect("pair");
        let reading = host.attach(ours).expect("attached");
        let footer = std::thread::spawn(move || {
            let mut line = String::new();
            BufReader::new(theirs).read_line(&mut line).expect("line");
            line.len()
        });
        host.relay_extension(framed(&[whole]).as_slice(), |_| {})
            .expect("the host takes the frame whole");
        let line = footer.join().expect("read");
        drop(reading);
        assert!(line > limit, "the enrichment did grow it: {line}");
        assert!(
            line <= crate::serve::CEILING,
            "a {line}-byte line, and the footer takes {}",
            crate::serve::CEILING
        );
    }

    /// FAILS IF: a line the footer would refuse is sent anyway — the footer
    /// ends the connection on it — or the connection is dropped for it.
    #[test]
    fn a_line_past_the_footers_ceiling_is_kept_back() {
        let host = Host::new(Box::new(Shared::default()), None);
        let (ours, theirs) = UnixStream::pair().expect("pair");
        // A WRITE THAT WOULD BLOCK FOR EVER — nobody reads `theirs` — fails
        // in a second instead, and a failed write drops the connection.
        ours.set_write_timeout(Some(Duration::from_secs(1)))
            .expect("timeout");
        host.inner().footer = Some(ours);
        let mut huge = state("ee");
        huge.kiosks[0].tabs = vec![crate::wire::Tab {
            url: "a".repeat(crate::serve::CEILING),
            ..crate::wire::Tab::default()
        }];
        Host::to_footer(&mut host.inner(), &Said::State(huge));
        assert!(host.inner().footer.is_some(), "the connection stays");
        theirs.set_nonblocking(true).expect("nonblocking");
        let mut byte = [0_u8; 1];
        assert!(
            matches!((&theirs).read(&mut byte), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
            "nothing was written"
        );
        // AND WHAT FITS still goes.
        Host::to_footer(&mut host.inner(), &Said::State(state("ff")));
        theirs.set_nonblocking(false).expect("blocking");
        let mut footer = BufReader::new(theirs);
        assert_eq!(read_line(&mut footer)["kiosks"][0]["nonce"], "ff");
    }

    /// FAILS IF: a footer that hangs up as soon as it is reached is called
    /// back at once, in a loop — or never again.
    #[test]
    fn a_footer_that_hangs_up_at_once_is_not_called_back_at_once() {
        let dir = std::env::temp_dir().join(format!("ronoctua-host-pause-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("browser.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let host = Host::new(Box::new(Shared::default()), None);
        let looping = Arc::clone(&host);
        std::thread::spawn(move || looping.footer_loop(&path));
        let until = std::time::Instant::now() + Duration::from_millis(700);
        let mut reached = 0;
        while std::time::Instant::now() < until {
            match listener.accept() {
                // THE FOOTER REFUSES IT, at once.
                Ok((stream, _)) => {
                    reached += 1;
                    drop(stream);
                }
                Err(_) => std::thread::sleep(Duration::from_millis(1)),
            }
        }
        drop(listener);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(reached >= 2, "it does try again: {reached}");
        assert!(reached <= 6, "{reached} connections in 0.7 s");
        // AND A CONNECTION THAT LIVED puts the pause back to nothing.
        assert_eq!(next_pause(LAST_PAUSE, QUICK), Duration::ZERO);
        assert_eq!(next_pause(Duration::ZERO, Duration::ZERO), FIRST_PAUSE);
        assert_eq!(next_pause(LAST_PAUSE, Duration::ZERO), LAST_PAUSE);
    }

    #[test]
    fn a_state_is_enriched_on_its_way_to_the_footer() {
        let config =
            std::env::temp_dir().join(format!("ronoctua-host-enrich-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&config);
        let profile = config.join("google-chrome").join("Default");
        std::fs::create_dir_all(&profile).expect("dir");
        let id = "abcdefghijklmnopabcdefghijklmnop";
        std::fs::write(
            profile.join("Preferences"),
            format!(r#"{{"extensions":{{"pinned_extensions":["{id}"]}}}}"#),
        )
        .expect("write");
        let host = Host::new(Box::new(Shared::default()), Some(config.clone()));
        let mut s = state("dd");
        s.extensions = vec![Extension {
            id: id.into(),
            name: "x".into(),
            ..Extension::default()
        }];
        host.relay_extension(
            framed(&[serde_json::to_value(Said::State(s)).expect("json")]).as_slice(),
            |_| {},
        )
        .expect("relay");
        let pinned = |host: &Host| match host.inner().last.clone() {
            Some(Said::State(s)) => s.extensions[0].pinned_at,
            _ => None,
        };
        assert_eq!(pinned(&host), Some(0));
        // Unpinned in Chrome: the preferences change and the state follows.
        std::fs::write(
            profile.join("Preferences"),
            r#"{"extensions":{"pinned_extensions":[]}}"#,
        )
        .expect("write");
        host.preferences_changed();
        assert_eq!(pinned(&host), None);
        let _ = std::fs::remove_dir_all(&config);
    }
}
