//! A live terminal: the engine on a thread of its own, everything through channels — §7.10.
//!
//! # Why the thread exists BEFORE there is a reason for it
//!
//! §7.10 is explicit: `libghostty-vt`'s types are `!Send`/`!Sync`, and
//! each `Terminal` must live **on a dedicated thread with all access through
//! channels, from the start**. With `alacritty_terminal` that is no bother —
//! it would cross a thread without complaining. Making the swap later would force
//! touching every caller on the same day the new engine arrives, which is the
//! worst possible day to change two things.
//!
//! Here the engine is **built inside the thread** and never leaves it: what is
//! passed in is a function, and what comes back is a [`Grid`], which is pure data. A
//! `!Send` engine fits this without a single line of change.
//!
//! # Why the PTY is not here
//!
//! The PTY is a file descriptor: it crosses threads, and tying it to the
//! engine would make the VT thread block on a read. Whoever reads the PTY calls
//! [`Terminal::feed`] — they are two separate problems, and §7.10 only
//! requires isolating the second.

use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;

use crate::{Engine, Grid};

/// What gets asked of the engine's thread.
///
/// An `enum` and not methods: the boundary is a queue, and a queue only transports
/// data. It is the same reason as the buffer protocol's `Effect`.
enum Request {
    /// Bytes coming from the PTY.
    Feed(Vec<u8>),
    /// A new size, in cells.
    Resize(u16, u16),
    /// Send the grid back through this channel.
    Grid(Sender<Grid>),
}

/// Builds the engine. It runs **inside** the thread.
///
/// It is the type that makes `!Send` irrelevant: one does not pass an engine across the
/// boundary, one passes the recipe for making one. A function pointer, and not
/// `Box<dyn Fn>`, because the recipe does not need to capture anything — and if it
/// did, that would be state crossing the boundary through the back door.
pub type Factory = fn(u16, u16) -> Box<dyn Engine>;

/// A running terminal.
///
/// `Send + Sync` even when the engine is not: it does not hold the engine, it holds
/// one end of a channel.
pub struct Terminal {
    requests: Option<Sender<Request>>,
    thread: Option<JoinHandle<()>>,
    name: &'static str,
}

impl Terminal {
    /// Starts the thread and builds the engine inside it.
    ///
    /// # Errors
    ///
    /// The system refuses to create the thread.
    pub fn fresh(columns: u16, lines: u16, fabrica: Factory) -> std::io::Result<Self> {
        let (tx, rx) = channel::<Request>();
        // The name travels through a channel of its own because only the engine knows it, and the
        // engine only exists after the thread has started.
        let (tx_nome, rx_nome) = channel::<&'static str>();

        let thread = std::thread::Builder::new()
            .name("ronoctua-vt".into())
            .spawn(move || {
                let mut engine = fabrica(columns, lines);
                // If nobody is listening any more, the thread may as well exit.
                if tx_nome.send(engine.name()).is_err() {
                    return;
                }
                // A `for` over the receiver: it ends on its own when the last
                // `Sender` is dropped. It is what makes the handle's `Drop` end the
                // thread with no signal, flag or `Request::Close` — a request
                // to shut down would only be obeyed if the queue were empty,
                // and then shutdown would depend on the load.
                for p in rx {
                    match p {
                        Request::Feed(b) => engine.feed(&b),
                        Request::Resize(c, l) => engine.resize(c, l),
                        // A `send` that fails is a caller that gave up
                        // waiting. It is not an error, and there is nobody to tell.
                        Request::Grid(answer) => drop(answer.send(engine.grid())),
                    }
                }
            })?;

        let name = rx_nome
            .recv()
            .map_err(|_| std::io::Error::other("a thread do motor morreu antes de construi-lo"))?;

        Ok(Self {
            requests: Some(tx),
            thread: Some(thread),
            name,
        })
    }

    /// Which engine is underneath. It is what the differential diff prints.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Delivers bytes from the PTY. **It does not block** — the queue absorbs.
    ///
    /// # Errors
    ///
    /// The engine's thread died.
    pub fn feed(&self, bytes: &[u8]) -> Result<(), Dead> {
        self.send_it(Request::Feed(bytes.to_vec()))
    }

    /// A new size.
    ///
    /// # Errors
    ///
    /// The engine's thread died.
    pub fn resize(&self, columns: u16, lines: u16) -> Result<(), Dead> {
        self.send_it(Request::Resize(columns, lines))
    }

    /// The grid right now. **It blocks** until the thread answers.
    ///
    /// Blocking is deliberate: whoever asks for the grid is going to draw it, and drawing the
    /// grid from two frames ago is worse than waiting for it. The queue is
    /// FIFO, so what comes back already includes every earlier `feed` — without
    /// that there would be a race between typing and seeing.
    ///
    /// # Errors
    ///
    /// The engine's thread died.
    pub fn grid(&self) -> Result<Grid, Dead> {
        let (tx, rx) = channel();
        self.send_it(Request::Grid(tx))?;
        rx.recv().map_err(|_| Dead)
    }

    fn send_it(&self, p: Request) -> Result<(), Dead> {
        self.requests
            .as_ref()
            .ok_or(Dead)?
            .send(p)
            .map_err(|_| Dead)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // It drops the `Sender` BEFORE the join: that is what makes the thread's `for`
        // end. Without this the join would wait forever — and the failure
        // mode would be the whole shell freezing when a terminal is closed.
        self.requests = None;
        if let Some(t) = self.thread.take() {
            drop(t.join());
        }
    }
}

/// The engine's thread is no longer there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dead;

impl std::fmt::Display for Dead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a thread do motor de terminal nao esta mais rodando")
    }
}

impl std::error::Error for Dead {}

#[cfg(test)]
mod tests {
    use super::{Factory, Terminal};

    fn alacritty(c: u16, l: u16) -> Box<dyn crate::Engine> {
        Box::new(crate::alacritty::Alacritty::fresh(c, l))
    }

    #[test]
    fn the_handle_crosses_a_thread_even_with_a_non_send_engine() {
        // THIS is the test that justifies the module. alacritty's `Term`
        // would cross; libghostty-vt's would not. If someone one day stores the
        // engine inside the `Terminal`, this stops compiling — which is
        // exactly when one wants to find out.
        const fn demands<T: Send + Sync>() {}
        demands::<Terminal>();
    }

    #[test]
    fn the_factory_is_what_crosses_the_boundary() {
        // A function pointer, not a ready-made engine: it is what allows building
        // a `!Send` type on the other side.
        const fn demands_send<T: Send>() {}
        demands_send::<Factory>();
    }

    #[test]
    fn what_is_fed_in_shows_up_on_the_grid() {
        let t = Terminal::fresh(20, 3, alacritty).unwrap();
        t.feed(b"oi").unwrap();
        let g = t.grid().unwrap();
        assert!(
            g.canonical_f().starts_with("oi"),
            "a grade nao tem o que foi alimentado: {:?}",
            g.canonical_f()
        );
    }

    #[test]
    fn the_queues_order_is_the_arrival_order() {
        // The grid may only come back AFTER every earlier `feed`. Without the
        // FIFO there would be a race between typing and seeing — the failure mode is a
        // character that shows up a frame late, and only sometimes.
        let t = Terminal::fresh(20, 3, alacritty).unwrap();
        for c in [b"a", b"b", b"c"] {
            t.feed(c).unwrap();
        }
        assert!(t.grid().unwrap().canonical_f().starts_with("abc"));
    }

    #[test]
    fn resizing_reaches_the_engine() {
        let t = Terminal::fresh(80, 24, alacritty).unwrap();
        t.resize(10, 2).unwrap();
        let g = t.grid().unwrap();
        assert_eq!((g.columns, g.lines), (10, 2));
    }

    #[test]
    fn the_name_comes_from_the_engine_built_in_there() {
        // The name cannot be passed in from outside: it would be possible to lie about
        // which engine is running, and the differential diff prints this name.
        let t = Terminal::fresh(10, 1, alacritty).unwrap();
        assert_eq!(t.name(), "alacritty");
    }

    #[test]
    fn dropping_the_handle_ends_the_thread() {
        // Without the `Sender` being dropped in `Drop`, the `join` would wait forever — and
        // the symptom would be the whole shell freezing when a terminal is closed.
        let t = Terminal::fresh(10, 1, alacritty).unwrap();
        t.feed(b"x").unwrap();
        drop(t); // if the join hangs, the test never ends
    }
}
