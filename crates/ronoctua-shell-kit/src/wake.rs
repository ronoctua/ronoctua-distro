//! A DOOR'S LOOP, WOKEN from a thread — what a thread calls once what it was
//! given to do is done.
//!
//! # Why a module of its own
//!
//! It was born in the player's subtitles, and then the file picker, the
//! network, the software centre, the stick, Syncthing and the camera's
//! settings all came to hand their threads one: a door's own plumbing,
//! which no view should have to name another view to reach.
//!
//! # Where a thread hands what it made ([`Out`])
//!
//! A window has a loop of its own, and a thread of its hands its results to
//! a channel of that loop. A REGION of the footer has none: it lives in the
//! footer's loop and is reached through it, so its threads hand theirs to a
//! queue of the region's and wake it ([`Wake`]). [`Out`] is either, and the
//! threads that draw a preview, a thumbnail or a globe take it — so one
//! thread serves both doors without knowing which it serves.

/// What a thread calls when what it was given to do is done: the door's loop
/// woken — a channel of that loop, in each door — to take the result where
/// it was left.
#[derive(Clone)]
pub struct Wake(std::sync::Arc<dyn Fn() + Send + Sync>);

impl Wake {
    /// A wake that calls `f`.
    #[must_use]
    pub fn new(f: impl Fn() + Send + Sync + 'static) -> Self {
        Self(std::sync::Arc::new(f))
    }

    /// Wakes the door — what a thread calls once what it left is ready.
    pub fn call(&self) {
        (self.0)();
    }
}

impl std::fmt::Debug for Wake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Wake")
    }
}

/// WHERE A THREAD HANDS WHAT IT MADE — a channel of a door's loop, or any
/// function (a region of the footer's queue, which wakes it). See the module.
pub struct Out<T>(std::sync::Arc<dyn Fn(T) -> bool + Send + Sync>);

/// The other end is gone: nobody will read what a thread hands on, and the
/// thread may stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gone;

impl std::fmt::Display for Gone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("nobody reads what this thread hands on any more")
    }
}

impl std::error::Error for Gone {}

impl<T> Out<T> {
    /// A sink that calls `f` — `false` when nobody is there any more.
    #[must_use]
    pub fn new(f: impl Fn(T) -> bool + Send + Sync + 'static) -> Self {
        Self(std::sync::Arc::new(f))
    }

    /// Hands `t` on.
    ///
    /// # Errors
    ///
    /// [`Gone`] when nobody reads it any more: the thread may stop.
    pub fn send(&self, t: T) -> Result<(), Gone> {
        if (self.0)(t) { Ok(()) } else { Err(Gone) }
    }

    /// The same sink, for something else that becomes a `T` on the way —
    /// a film's frame handed on as what the panels heard.
    #[must_use]
    pub fn map<U>(&self, into: impl Fn(U) -> T + Send + Sync + 'static) -> Out<U>
    where
        T: 'static,
    {
        let out = std::sync::Arc::clone(&self.0);
        Out(std::sync::Arc::new(move |u| out(into(u))))
    }
}

impl<T> Clone for Out<T> {
    fn clone(&self) -> Self {
        Self(std::sync::Arc::clone(&self.0))
    }
}

impl<T> std::fmt::Debug for Out<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Out")
    }
}

/// A channel of a door's own loop, as a sink — what a window hands its
/// threads.
impl<T: Send + 'static> From<smithay_client_toolkit::reexports::calloop::channel::Sender<T>>
    for Out<T>
{
    fn from(sender: smithay_client_toolkit::reexports::calloop::channel::Sender<T>) -> Self {
        let sender = std::sync::Mutex::new(sender);
        Self::new(move |t| sender.lock().is_ok_and(|s| s.send(t).is_ok()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a sink whose reader is gone says it was heard — a thread
    /// that should stop would go on working for nobody — or a mapped sink
    /// does not reach the one it was made from.
    #[test]
    fn a_sink_says_when_nobody_reads_it() {
        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        let out = Out::new(move |n| tx.send(n).is_ok());
        assert_eq!(out.send(3), Ok(()));
        let doubled: Out<u16> = out.map(|n: u16| u32::from(n) * 2);
        assert_eq!(doubled.send(5), Ok(()));
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![3, 10]);
        drop(rx);
        assert_eq!(out.send(1), Err(Gone));
        assert_eq!(doubled.send(1), Err(Gone));
    }

    /// FAILS IF: a channel of a loop, made a sink, does not carry what it is
    /// handed, or says it was heard once its loop's end is gone.
    #[test]
    fn a_loops_channel_is_a_sink() {
        use smithay_client_toolkit::reexports::calloop::channel;
        let (tx, rx) = channel::channel::<&str>();
        let out: Out<&str> = tx.into();
        assert_eq!(out.send("made"), Ok(()));
        assert_eq!(rx.try_recv().ok(), Some("made"));
        drop(rx);
        assert_eq!(out.send("again"), Err(Gone));
    }
}
