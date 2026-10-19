//! The directory saying it moved, instead of being asked once a second.
//!
//! # Why this is not `crate::watch`
//!
//! Because that module answers *how do I follow MY OWN long copy, and how do I
//! stop it*. This one answers *who else touched the directory I am looking at*.
//! They share the word and nothing else: one is a switch a copy reads between
//! bites, the other is a descriptor the kernel writes into.
//!
//! # Why an event and not a clock
//!
//! `listing::stamp` answers "has the directory's mtime moved", and the footer
//! asks it once a second. It is a fair question with two holes no interval
//! closes:
//!
//! 1. **A file rewritten in place does not move the DIRECTORY's mtime.** The
//!    row's size and date are stale and every poll agrees they are fine.
//! 2. **`vfat` stamps with a granularity of two seconds.** A file landing in
//!    the same bucket as the reading is invisible for ever — and a USB stick is
//!    exactly what this manager exists to look at.
//!
//! The cost runs the other way from what one expects: this thread waits with no
//! deadline, so a directory nobody touches costs **nothing at all**, while a
//! clock wakes its process once a second for its whole life. With several
//! windows open, that difference is the whole point.
//!
//! What inotify cannot see is a change made by ANOTHER MACHINE on a network
//! filesystem. The footer keeps its one-second `stat` for that; it was already
//! paying for the tick.
//!
//! # Why the report carries nothing
//!
//! `ronoctua_disks::watch` sends the whole `Rack` because re-reading udisks2
//! costs a bus round trip and one stick emits four signals in milliseconds.
//! Here the opposite holds: the answer is `View::reread`, which the door must
//! run anyway on its own thread — it is the only place that knows the sort, the
//! filter and which name the cursor is on. So the message is a nudge, and being
//! a nudge it is **idempotent**: an event misattributed to a directory just left
//! costs one wasted re-read, never a wrong screen.
//!
//! And the nudge must NOT be answered with `stale()`. That is the mtime again,
//! and `CLOSE_WRITE` and `ATTRIB` — the two events this mask was widened for —
//! do not move it. A door that second-guessed the kernel with a worse oracle
//! would drop exactly the news this module exists to carry.
//!
//! # Why every inotify call happens on the watcher's thread
//!
//! The thread lives inside a blocking wait, so the door cannot hand it a new
//! directory the ordinary way. The tempting shortcut is to share the descriptor
//! and let the door call `add_watch` itself — which opens a race with the reader
//! over which watch a queued event belonged to.
//!
//! Instead the door sends the path down an `mpsc` and rings a pipe, which is one
//! of the two descriptors the thread waits on. Dropping the [`Notice`] closes
//! the pipe's writing end, the wait comes back with `HUP`, and the thread
//! leaves — so ending it costs no syscall that can fail, which is what a `Drop`
//! wants. A thread parked on a descriptor nobody can close would stay parked for
//! the rest of the session; the player already paid for that mistake once.

use std::ffi::OsString;
use std::io::{Read as _, Write as _};
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use rustix::event::{PollFlags, Timespec};
use rustix::fd::OwnedFd;
use rustix::fs::inotify;

/// How long the thread waits for silence before reporting a burst's tail.
///
/// It is a HOLD-OFF and not a delay: the first change of a burst goes out at
/// once, and this only bounds how often the ones behind it may. A quarter of a
/// second is what turns an editor's atomic save — a write, a rename and a
/// permission change within a few milliseconds — into one further re-read
/// rather than three.
const QUIET: Duration = Duration::from_millis(250);

/// The longest a burst may hold its tail back.
///
/// Without a ceiling, copying a directory into the one on screen is an unbroken
/// stream and the tail never lands. With it, a copy that never goes quiet still
/// refreshes about twice a second — where the footer's clock managed once, so
/// this is never the worse of the two.
const BURST: Duration = Duration::from_secs(1);

/// How much of a burst a watch may absorb before answering again.
///
/// # Why this is a choice and not a constant
///
/// Because the two answers a change can have cost wildly different things. A
/// listing's answer is a `reread` — a `readdir`, an `lstat` and a MIME lookup
/// PER ENTRY — so copying a thousand files into the open directory has to
/// become ONE re-read or the window cannot keep up; that is what `QUIET` and
/// `BURST` are for, and the arithmetic is in [`Watcher::hold_off`].
///
/// A PREFERENCE file's answer is a repaint, and its "burst" is a person
/// holding an arrow key. Measured on this machine: with the hold-off, the
/// first change lands in 5 ms and the next in **253** — the quarter second
/// the thread spends waiting for silence that is not coming. The theme
/// chooser previews by writing the file every keystroke, and a quarter of a
/// second per step is what the owner reported as a bad preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    /// The leading edge at once, then the tail of the burst — once.
    Coalesced,
    /// Every change as it lands. For a watch whose answer is cheap and whose
    /// changes are a person's keystrokes.
    AtOnce,
}

/// The most directories one watch holds under its trees
/// ([`Notice::on_dirs`]).
///
/// A tree is watched directory by directory — inotify has no recursive
/// watch — and each one is a watch out of `max_user_watches`. A repository's
/// `refs` is a handful of folders; a mirror that fetched every pull request
/// is thousands, and a bound is what keeps one browser from spending the
/// session's watches. Past it the rest are not heard, and the log says so.
pub const TREE_MOST: usize = 512;

/// What decides whether a path under a [`Tree`] is heard: `true` for one
/// that is not — a directory it names is not watched, nor anything under
/// it, and an event about a path it names is not reported.
pub type Unheard = std::sync::Arc<dyn Fn(&Path) -> bool + Send + Sync>;

/// A tree [`Notice::on_trees`] hears: a directory and every directory under
/// it, the ones made later included — but what `unheard` names.
#[derive(Clone)]
pub struct Tree {
    /// The directory.
    pub root: PathBuf,
    /// The most directories held under it, its OWN: past it the rest of
    /// this tree is not heard, and no other tree pays for it. `None`: one
    /// of the trees that share [`TREE_MOST`] between them — what
    /// [`Notice::on_dirs`] makes of every tree it is given.
    pub most: Option<usize>,
    /// What under it is not heard; `None`: everything is.
    pub unheard: Option<Unheard>,
}

impl Tree {
    /// A tree as [`Notice::on_dirs`] hears it: whole, sharing [`TREE_MOST`].
    #[must_use]
    pub const fn shared(root: PathBuf) -> Self {
        Self {
            root,
            most: None,
            unheard: None,
        }
    }

    /// Whether `path` under it is not heard.
    fn unheard(&self, path: &Path) -> bool {
        self.unheard.as_ref().is_some_and(|f| f(path))
    }
}

impl std::fmt::Debug for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tree")
            .field("root", &self.root)
            .field("most", &self.most)
            .field("unheard", &self.unheard.is_some())
            .finish()
    }
}

/// The read buffer, in bytes.
///
/// An `inotify_event` is sixteen bytes plus the name, so this holds upwards of a
/// hundred at a time. It is not a limit on anything: whatever does not fit stays
/// in the kernel's queue and comes out of the next read.
const BUFFER: usize = 4096;

/// What is asked of the kernel, on the CURRENT directory and never recursively.
///
/// One watch per open directory is what keeps this inside `max_user_watches`
/// however big the tree is; a recursive watch on a home directory is how that
/// limit is normally reached.
///
/// `MODIFY` is deliberately absent. It fires on every `write`, so a log being
/// appended to would be a hose; `CLOSE_WRITE` is the moment a listing cares
/// about, which is when the writer let go. `ATTRIB` is present because the
/// permission, owner and group columns (`C-d`) change without the directory
/// changing at all.
///
/// `EXCL_UNLINK` stops events for a file that is unlinked but still open, and
/// `ONLYDIR` makes a path that is not a directory a refusal here rather than a
/// surprise later.
const WANTED: inotify::WatchFlags = inotify::WatchFlags::CREATE
    .union(inotify::WatchFlags::DELETE)
    .union(inotify::WatchFlags::MOVED_FROM)
    .union(inotify::WatchFlags::MOVED_TO)
    .union(inotify::WatchFlags::ATTRIB)
    .union(inotify::WatchFlags::CLOSE_WRITE)
    .union(inotify::WatchFlags::DELETE_SELF)
    .union(inotify::WatchFlags::MOVE_SELF)
    .union(inotify::WatchFlags::ONLYDIR)
    .union(inotify::WatchFlags::EXCL_UNLINK);

/// The door's handle on a directory that reports itself.
///
/// Dropping it ends the thread and closes both descriptors.
pub struct Notice {
    /// Where the thread is told to look. `None` means "nowhere".
    orders: mpsc::Sender<Option<PathBuf>>,
    /// How the thread is woken to hear it — and, when this is dropped, how it
    /// learns to leave.
    ring: std::io::PipeWriter,
    /// What was last asked for, so asking again costs nothing.
    ///
    /// The door calls [`Notice::look_at`] after every keypress rather than
    /// having to know which gestures navigate, and this is what makes that free.
    listing: Option<PathBuf>,
}

impl std::fmt::Debug for Notice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notice")
            .field("listing", &self.listing)
            .finish_non_exhaustive()
    }
}

impl Notice {
    /// Opens the descriptors and raises the thread.
    ///
    /// The fallible part happens HERE, on the caller's thread, so a door that
    /// cannot have a watcher learns it now and can arrange its own floor — the
    /// arrangement `ronoctua_disks::watch` already uses. `on_stir` returning
    /// `false` ends the thread, which is how a closed channel says "the door
    /// left".
    ///
    /// `sidebar` is the DIRECTORY the sidebar persists into, and it is taken
    /// once because it never changes for the life of a view. The directory and
    /// not the file: `sidebar::store::write_to` writes a scratch file and
    /// renames it over, so the inode a file watch sat on is gone by the time the
    /// new content is there.
    ///
    /// # Errors
    ///
    /// The process is out of descriptors, or out of inotify instances
    /// (`/proc/sys/fs/inotify/max_user_instances`), or the thread could not be
    /// raised.
    pub fn watching<F>(sidebar: Option<&Path>, on_stir: F) -> Result<Self>
    where
        F: Fn() -> bool + Send + 'static,
    {
        Self::raise(
            sidebar.map(Path::to_path_buf).into_iter().collect(),
            Vec::new(),
            None,
            Pace::Coalesced,
            on_stir,
        )
    }

    /// [`Notice::watching`], for a door that LISTS a folder: the `/private`
    /// vault's status folder is heard too, when this machine has one. What a
    /// listing may read inside the vault is what that status says, so a
    /// vault opened, locked or suspended at sleep is a stir like a file
    /// landing — the door reads again and draws the vault's row anew.
    ///
    /// # Errors
    ///
    /// As [`Notice::watching`].
    pub fn for_listing<F>(sidebar: Option<&Path>, on_stir: F) -> Result<Self>
    where
        F: Fn() -> bool + Send + 'static,
    {
        let vault = Path::new(ronoctua_private::STATUS_FILE)
            .parent()
            .filter(|d| d.is_dir())
            .map(Path::to_path_buf);
        Self::raise(
            sidebar
                .map(Path::to_path_buf)
                .into_iter()
                .chain(vault)
                .collect(),
            Vec::new(),
            None,
            Pace::Coalesced,
            on_stir,
        )
    }

    /// Watches SEVERAL FIXED DIRECTORIES, whole — a repository's git
    /// directory, today — and SEVERAL TREES: a directory and every directory
    /// under it, the ones made later included (a repository's `refs`, where
    /// a branch called `feature/x` is a folder `feature` that did not exist
    /// before it was pushed).
    ///
    /// The same thread, doorbell and `Drop` as [`Notice::watching`], with no
    /// listing to point and no name to narrow by: every event in any of them
    /// counts. A directory that does not exist is skipped with a warning, as
    /// a listing that went away is, and the others go on reporting. A tree
    /// holds at most [`TREE_MOST`] directories: past it, the rest are not
    /// heard, and the log says so.
    ///
    /// # Errors
    ///
    /// As [`Notice::watching`].
    pub fn on_dirs<F>(dirs: &[PathBuf], trees: &[PathBuf], pace: Pace, on_stir: F) -> Result<Self>
    where
        F: Fn() -> bool + Send + 'static,
    {
        Self::raise(
            dirs.to_vec(),
            trees.iter().cloned().map(Tree::shared).collect(),
            None,
            pace,
            on_stir,
        )
    }

    /// [`Notice::on_dirs`], with each tree saying what of it is not heard
    /// and how many directories it may hold of its own ([`Tree`]): a tree
    /// in a home whose folder of packages is thousands of directories hears
    /// the person's files beside them and not the packages, and the other
    /// trees keep their watches whatever that one holds. In the order given:
    /// a tree that shares [`TREE_MOST`] with others is grown after those
    /// before it.
    ///
    /// An event in a directory that is also one of `dirs` is always heard.
    ///
    /// # Errors
    ///
    /// As [`Notice::watching`].
    pub fn on_trees<F>(dirs: &[PathBuf], trees: Vec<Tree>, pace: Pace, on_stir: F) -> Result<Self>
    where
        F: Fn() -> bool + Send + 'static,
    {
        Self::raise(dirs.to_vec(), trees, None, pace, on_stir)
    }

    /// The one constructor behind the doors: the fixed directories (the
    /// sidebar's, the file's, or a repository's), the trees, an optional name
    /// that narrows what counts, and the thread.
    fn raise<F>(
        fixed: Vec<PathBuf>,
        trees: Vec<Tree>,
        only: Option<OsString>,
        pace: Pace,
        on_stir: F,
    ) -> Result<Self>
    where
        F: Fn() -> bool + Send + 'static,
    {
        // NONBLOCK so draining ends with `WOULDBLOCK` instead of parking inside
        // the drain; CLOEXEC so the manager's children — ffmpeg, the root
        // helper — do not inherit a descriptor they have no use for.
        let fd = inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK)
            .context("could not open inotify")?;
        let (reader, ring) = std::io::pipe().context("could not open the notice's pipe")?;
        // THE DOORBELL NEVER BLOCKS THE DRAWING LOOP. `add_watch` on a hung
        // network mount parks the watcher's thread, and a blocking write into a
        // pipe nobody is draining would park the door behind it — a file
        // manager frozen by the filesystem it was merely looking at.
        //
        // Losing a ring costs nothing, and that is not luck: a full pipe means
        // bytes are ALREADY waiting, so the thread has not woken yet — and when
        // it does it drains the whole channel, this order included.
        if let Ok(flags) = rustix::fs::fcntl_getfl(&ring) {
            let _ = rustix::fs::fcntl_setfl(&ring, flags | rustix::fs::OFlags::NONBLOCK);
        }
        let (orders, told) = mpsc::channel();
        std::thread::Builder::new()
            .name("ronoctua-notice".to_owned())
            .spawn(move || {
                Watcher {
                    fd,
                    ring: reader,
                    told,
                    listing: None,
                    fixed,
                    fixed_wds: Vec::new(),
                    trees,
                    grown: Vec::new(),
                    only,
                    pace,
                    held: Vec::new(),
                    listing_wd: None,
                }
                .run(&on_stir);
            })
            .context("could not start the notice thread")?;
        Ok(Self {
            orders,
            ring,
            listing: None,
        })
    }

    /// Watches ONE file in a directory, by name — the theme's, today.
    ///
    /// The same thread, doorbell and `Drop` as [`Notice::watching`]; what
    /// differs is the filter: only an event whose name is `name` counts, so a
    /// door that watches `theme.json` in the configuration directory is not
    /// woken by the sidebar file beside it. The directory and not the file, for
    /// the reason `watching` gives about the sidebar: the writer renames a
    /// scratch file over the real one, and a watch on the inode would be
    /// watching the old file. `MOVED_TO` is that rename arriving.
    ///
    /// `pace` is the caller's, and it matters: a preference file that a
    /// PREVIEW writes wants [`Pace::AtOnce`], or every change after the first
    /// lands a quarter of a second late. See [`Pace`].
    ///
    /// # Errors
    ///
    /// As [`Notice::watching`].
    pub fn on_file<F>(dir: &Path, name: &str, pace: Pace, on_stir: F) -> Result<Self>
    where
        F: Fn() -> bool + Send + 'static,
    {
        Self::raise(
            vec![dir.to_path_buf()],
            Vec::new(),
            Some(OsString::from(name)),
            pace,
            on_stir,
        )
    }

    /// Points the watch at a directory.
    ///
    /// Asking for the one already watched returns without a syscall and without
    /// a message.
    pub fn look_at(&mut self, dir: &Path) {
        if self.listing.as_deref() == Some(dir) {
            return;
        }
        self.listing = Some(dir.to_path_buf());
        self.tell(self.listing.clone());
    }

    /// Stops watching the listing, without ending the thread.
    ///
    /// This is what the footer does when the manager closes: it drops the watch
    /// but keeps the thread, which is cheaper than tearing one down and raising
    /// it again on the next `LEADER .`, and has nothing to get wrong in between.
    /// A thread waiting on watches nobody is touching costs no wake-ups at all.
    ///
    /// The sidebar's watch stays, and deliberately: `LEADER a` pins an
    /// application with no manager open, and that is precisely when the other
    /// door needs to hear about it.
    pub fn rest(&mut self) {
        if self.listing.is_none() {
            return;
        }
        self.listing = None;
        self.tell(None);
    }

    /// What is being watched.
    #[must_use]
    pub fn watching_now(&self) -> Option<&Path> {
        self.listing.as_deref()
    }

    /// Hands an order over and rings for it to be taken.
    fn tell(&mut self, order: Option<PathBuf>) {
        if self.orders.send(order).is_err() {
            // The thread is gone. Ringing a bell nobody hears would only fill a
            // pipe.
            return;
        }
        // ONE BYTE, and its value means nothing: the pipe is a doorbell, not a
        // queue. The order itself travelled by channel, where a `PathBuf` needs
        // no encoding and a name that is not UTF-8 survives.
        let _ = self.ring.write(&[1]);
    }
}

/// What one wait came back with.
enum Waited {
    /// The wait itself failed, or the door has gone. Nothing to retry.
    Over,
    /// Nothing happened before the deadline.
    Quiet,
    /// One or both descriptors have something.
    Ready {
        /// The kernel wrote events.
        stirred: bool,
        /// The door rang.
        told: bool,
    },
}

/// The thread's own state. None of it is shared.
struct Watcher {
    /// The inotify instance.
    fd: OwnedFd,
    /// The doorbell. Its writing end closing is how this thread learns to leave.
    ring: std::io::PipeReader,
    /// Where orders arrive.
    told: mpsc::Receiver<Option<PathBuf>>,
    /// The directory on screen.
    listing: Option<PathBuf>,
    /// The directory the sidebar persists into — or, for [`Notice::on_file`],
    /// the one the file lives in, or [`Notice::on_dirs`]'s. Fixed for the
    /// life of the watch.
    fixed: Vec<PathBuf>,
    /// The watches the fixed directories hold: an event in one is heard
    /// whatever a tree says of it.
    fixed_wds: Vec<i32>,
    /// The trees [`Notice::on_dirs`] or [`Notice::on_trees`] was given:
    /// each root and every directory under it.
    trees: Vec<Tree>,
    /// Every directory held under a tree, by its watch, and the tree's
    /// place in `trees` — so a directory made inside one is known to be
    /// that tree's, and is held in its turn.
    grown: Vec<(i32, PathBuf, usize)>,
    /// The one file name that counts, when the watch is on a file. `None`
    /// means every event in a held directory is ours.
    only: Option<OsString>,
    /// How much of a burst this watch absorbs. See [`Pace`].
    pace: Pace,
    /// Every watch descriptor held.
    ///
    /// A list and not two fields because inotify keys a watch by INODE: asking
    /// for the same directory twice hands back the SAME descriptor, which
    /// happens the moment somebody browses the directory the sidebar persists
    /// to. Dropping them all and asking again — see [`Watcher::rearm`] — is what
    /// makes that case need no special sentence.
    held: Vec<i32>,
    /// The listing's own watch, while it is held. `None` beside a listing is
    /// one that could not be held — a folder of the `/private` vault while
    /// it is shut, or one not there yet — and it is asked for again at the
    /// next stir ([`Watcher::run`]).
    listing_wd: Option<i32>,
}

impl Watcher {
    /// Waits, takes orders, drains events, reports — while the door is there.
    fn run(mut self, on_stir: &dyn Fn() -> bool) {
        let mut buffer = [MaybeUninit::<u8>::uninit(); BUFFER];
        // The sidebar is wanted from the first moment; the listing arrives with
        // the first order.
        self.rearm();
        loop {
            // ASLEEP, with no deadline. This is the line that makes an open
            // window cost nothing while nothing happens.
            let (stirred, told) = match self.wait(None) {
                Waited::Over => return,
                // Not reachable without a deadline, and cheaper to treat as a
                // spurious wake-up than to prove impossible.
                Waited::Quiet => continue,
                Waited::Ready { stirred, told } => (stirred, told),
            };
            if told && !self.orders_taken() {
                return;
            }
            if !stirred || !self.drain(&mut buffer) {
                continue;
            }
            // A LISTING NOT HELD is asked for again, before the door reads:
            // a folder of the `/private` vault refused while it was shut is
            // held once its status (a fixed watch of `Notice::for_listing`)
            // says it opened — `Notice::look_at` swallows the same path, so
            // the door cannot ask for it again — and the reading the door does
            // next finds the watch already in place.
            if self.listing.is_some() && self.listing_wd.is_none() {
                self.rearm();
            }
            // THE LEADING EDGE. One file appearing is reported at once: holding
            // it for the quiet time would make a manager that is merely
            // debouncing look like a manager that is thinking.
            if !on_stir() {
                return;
            }
            // AND THE TAIL, for a watch that wants one. `AtOnce` goes straight
            // back to the wait: what is still queued comes out of the next
            // read, one report each, which is the whole point of it.
            if self.pace == Pace::Coalesced && !self.hold_off(&mut buffer, on_stir) {
                return;
            }
        }
    }

    /// Absorbs the rest of a burst, and reports its tail once.
    ///
    /// This is the whole reason the coalescing lives on the thread: sleeping is
    /// free here, and the door does one `View::reread` per burst instead of one
    /// per file. `reread` is a `readdir` plus an `lstat` and a MIME lookup PER
    /// ENTRY, so answering every event would be quadratic in the size of the
    /// directory for the one gesture that matters — copying a lot of files into
    /// it.
    fn hold_off(&mut self, buffer: &mut [MaybeUninit<u8>], on_stir: &dyn Fn() -> bool) -> bool {
        let since = Instant::now();
        let mut more = false;
        loop {
            let left = BURST.saturating_sub(since.elapsed());
            if left.is_zero() {
                break;
            }
            match self.wait(Some(QUIET.min(left))) {
                Waited::Over => return false,
                Waited::Quiet => break,
                Waited::Ready { stirred, told } => {
                    if told && !self.orders_taken() {
                        return false;
                    }
                    if stirred {
                        more |= self.drain(buffer);
                    }
                }
            }
        }
        !more || on_stir()
    }

    /// One wait on both descriptors.
    fn wait(&self, hush: Option<Duration>) -> Waited {
        let deadline = hush.map(|d| Timespec {
            tv_sec: i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(d.subsec_nanos()),
        });
        loop {
            let mut fds = [
                rustix::event::PollFd::new(&self.fd, PollFlags::IN),
                rustix::event::PollFd::new(&self.ring, PollFlags::IN),
            ];
            match rustix::event::poll(&mut fds, deadline.as_ref()) {
                Ok(0) => return Waited::Quiet,
                Ok(_) => {
                    let bell = fds[1].revents();
                    // THE DOOR IS GONE. Dropping the `Notice` closed the writing
                    // end, and `HUP` is how that arrives — no message to miss and
                    // no syscall in a `Drop` that could have failed.
                    if bell.intersects(PollFlags::HUP | PollFlags::ERR) {
                        return Waited::Over;
                    }
                    return Waited::Ready {
                        stirred: fds[0]
                            .revents()
                            .intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR),
                        told: bell.contains(PollFlags::IN),
                    };
                }
                // A signal is a wake-up, not a failure — the rule the player's
                // poll already follows.
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => {
                    tracing::warn!(error = %e, "the notice's wait failed; this directory stops reporting itself");
                    return Waited::Over;
                }
            }
        }
    }

    /// Takes every order waiting, and says whether to carry on.
    ///
    /// BEFORE the events are drained, and that ordering is what makes a
    /// re-pointing race harmless: a watch dropped here leaves its queued events
    /// with no owner in [`Watcher::held`], and [`Watcher::drain`] discards those.
    fn orders_taken(&mut self) -> bool {
        // The doorbell is emptied whatever it holds: what a byte means is "there
        // is something in the channel", never how many.
        let mut rung = [0_u8; 8];
        match self.ring.read(&mut rung) {
            // The writing end closed between the poll and here.
            Ok(0) => return false,
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => {
                tracing::warn!(error = %e, "the notice could not read its doorbell");
                return false;
            }
        }
        let mut moved = false;
        loop {
            match self.told.try_recv() {
                Ok(where_) => {
                    self.listing = where_;
                    moved = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return false,
            }
        }
        if moved {
            self.rearm();
        }
        true
    }

    /// Drops every watch and asks for the ones wanted now.
    ///
    /// Whole rather than differential, and that is not laziness: inotify hands
    /// back the SAME descriptor for two paths that are one inode, so "remove
    /// just the listing's" is a sentence that can silently take the sidebar's
    /// watch with it. Removing all and asking again cannot express that mistake.
    /// It only runs when a path actually changed — `Notice::look_at` swallows
    /// the repeats — so it is two syscalls on a keypress that navigates.
    fn rearm(&mut self) {
        for wd in self.held.drain(..) {
            // A directory that was deleted has already had its watch removed by
            // the kernel, and asking again is `EINVAL`. Nothing to report.
            let _ = inotify::remove_watch(&self.fd, wd);
        }
        self.grown.clear();
        self.fixed_wds.clear();
        self.listing_wd = match self.listing.clone() {
            Some(dir) => self.hold(&dir),
            None => None,
        };
        for dir in self.fixed.clone() {
            if let Some(wd) = self.hold(&dir) {
                self.fixed_wds.push(wd);
            }
        }
        for tree in 0..self.trees.len() {
            let root = self.trees[tree].root.clone();
            self.grow(tree, &root);
        }
    }

    /// Asks for one directory, and gives back its watch.
    fn hold(&mut self, dir: &Path) -> Option<i32> {
        // NOT IN THE /private VAULT unless it is open (`ronoctua_private`):
        // the path walked to its watch would freeze this thread in a
        // suspended one. A vault that opens later moves its status file,
        // which a listing's watch hears ([`Notice::for_listing`]), and this
        // thread asks for the listing again at that stir ([`Watcher::run`]).
        if !ronoctua_private::may_touch(dir) {
            return None;
        }
        match inotify::add_watch(&self.fd, dir, WANTED) {
            Ok(wd) => {
                if !self.held.contains(&wd) {
                    self.held.push(wd);
                }
                Some(wd)
            }
            // Not fatal and not silent. A directory just deleted, one that
            // cannot be read, and a machine at its watch limit all land here,
            // and in every case the door goes on working — it simply stops
            // hearing about this one.
            Err(e) => {
                tracing::warn!(error = %e, directory = %dir.display(), "could not watch the directory");
                None
            }
        }
    }

    /// Whether the tree at `tree` holds all it may: its own `most`, or —
    /// one that shares — [`TREE_MOST`] among every tree that shares.
    fn full(&self, tree: usize) -> (bool, usize) {
        match self.trees[tree].most {
            Some(most) => (
                self.grown.iter().filter(|(_, _, t)| *t == tree).count() >= most,
                most,
            ),
            None => (
                self.grown
                    .iter()
                    .filter(|(_, _, t)| self.trees[*t].most.is_none())
                    .count()
                    >= TREE_MOST,
                TREE_MOST,
            ),
        }
    }

    /// Holds `root` and every directory under it — a tree's (`tree`, its
    /// place in `trees`), or a directory just made inside one — but what
    /// the tree does not hear. Symbolic links are not followed: a link out
    /// of the tree is not the tree.
    fn grow(&mut self, tree: usize, root: &Path) {
        let mut todo = vec![root.to_path_buf()];
        while let Some(dir) = todo.pop() {
            if self.grown.iter().any(|(_, d, _)| d == &dir) {
                continue;
            }
            if self.trees[tree].unheard(&dir) {
                continue;
            }
            let (full, most) = self.full(tree);
            if full {
                tracing::warn!(directory = %dir.display(), most, "a tree too large to hear whole; this part of it is not watched");
                return;
            }
            let Some(wd) = self.hold(&dir) else {
                continue;
            };
            self.grown.push((wd, dir.clone(), tree));
            let Ok(each) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in each.flatten() {
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    todo.push(e.path());
                }
            }
        }
    }

    /// Reads everything the kernel has, and says whether any of it was ours.
    fn drain(&mut self, buffer: &mut [MaybeUninit<u8>]) -> bool {
        let mut ours = false;
        let mut gone: Vec<i32> = Vec::new();
        // DIRECTORIES MADE INSIDE A TREE, held once the kernel's queue is read
        // — before the door is told, so whatever lands in them after this is
        // heard, and whatever landed before is on disk for the reading the
        // report asks for.
        let mut born: Vec<(usize, PathBuf)> = Vec::new();
        {
            let mut reader = inotify::Reader::new(&self.fd, buffer);
            loop {
                let event = match reader.next() {
                    Ok(e) => e,
                    // Everything the kernel had has now been looked at.
                    Err(rustix::io::Errno::WOULDBLOCK) => break,
                    Err(rustix::io::Errno::INTR) => continue,
                    Err(e) => {
                        tracing::warn!(error = %e, "the notice could not read its events");
                        break;
                    }
                };
                let flags = event.events();
                // THE QUEUE OVERFLOWED, and what was dropped could have been
                // anything. Re-reading is the only honest answer; carrying on as
                // if nothing happened is how a listing lies quietly.
                if flags.contains(inotify::ReadFlags::QUEUE_OVERFLOW) {
                    ours = true;
                    continue;
                }
                let wd = event.wd();
                // A watch that is over — dropped by us a moment ago, or by the
                // kernel because the directory went away.
                if flags.contains(inotify::ReadFlags::IGNORED) {
                    gone.push(wd);
                    continue;
                }
                // AN EVENT WITH NO OWNER IS DISCARDED. This is the arm that makes
                // re-pointing safe: orders are taken first, so anything queued
                // against the directory just left is no longer in `held`.
                if !self.held.contains(&wd) {
                    continue;
                }
                let grown = self.grown.iter().find(|(w, _, _)| *w == wd);
                let named = grown
                    .zip(event.file_name())
                    .map(|((_, parent, tree), name)| {
                        (
                            *tree,
                            parent.join(std::ffi::OsStr::from_bytes(name.to_bytes())),
                        )
                    });
                // WHAT A TREE DOES NOT HEAR is no event of its — but in a
                // directory that is also a fixed one, it is that one's.
                if let Some((tree, path)) = &named
                    && self.trees[*tree].unheard(path)
                    && !self.fixed_wds.contains(&wd)
                    && self.listing_wd != Some(wd)
                {
                    continue;
                }
                if flags.contains(inotify::ReadFlags::ISDIR)
                    && flags.intersects(inotify::ReadFlags::CREATE | inotify::ReadFlags::MOVED_TO)
                    && let Some(born_here) = named
                {
                    born.push(born_here);
                }
                // A WATCH ON ONE FILE hears the whole directory and keeps the
                // one name; the rest is the sidebar file, a scratch file, and
                // whatever else lives beside it.
                ours |= match &self.only {
                    None => true,
                    Some(name) => event
                        .file_name()
                        .is_some_and(|f| f.to_bytes() == name.as_encoded_bytes()),
                };
            }
        }
        self.held.retain(|wd| !gone.contains(wd));
        self.grown.retain(|(wd, _, _)| !gone.contains(wd));
        self.fixed_wds.retain(|wd| !gone.contains(wd));
        // THE LISTING'S WATCH OVER — its folder deleted, or the vault it is
        // in locked and unmounted: asked for again at the next stir.
        if self.listing_wd.is_some_and(|wd| gone.contains(&wd)) {
            self.listing_wd = None;
        }
        for (tree, dir) in born {
            self.grow(tree, &dir);
        }
        ours
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// A directory of this test's own, emptied first.
    ///
    /// Keyed by the process id like `serve::scratch`, so two runs of the suite
    /// at once do not read each other's files.
    fn scratch(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("ronoctua-notice-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// A one-file notice whose reports are counted, and the counter.
    fn counted_file(dir: &Path, name: &str, pace: Pace) -> (Notice, Arc<AtomicUsize>) {
        let seen = Arc::new(AtomicUsize::new(0));
        let mine = Arc::clone(&seen);
        let notice = Notice::on_file(dir, name, pace, move || {
            mine.fetch_add(1, Ordering::SeqCst);
            true
        })
        .unwrap();
        (notice, seen)
    }

    /// A notice whose reports are counted, and the counter.
    fn counted(sidebar: Option<&Path>) -> (Notice, Arc<AtomicUsize>) {
        let seen = Arc::new(AtomicUsize::new(0));
        let mine = Arc::clone(&seen);
        let notice = Notice::watching(sidebar, move || {
            mine.fetch_add(1, Ordering::SeqCst);
            true
        })
        .unwrap();
        (notice, seen)
    }

    /// How long a test may wait for something that should be immediate.
    ///
    /// Generous because it is a CEILING and not a sleep: every wait below
    /// returns the moment the count moves, so a fast machine pays none of it.
    const PATIENCE: Duration = Duration::from_secs(5);

    /// Waits for the count to pass `was`, and gives back what it reached.
    ///
    /// NEVER a fixed sleep. One long enough for a loaded machine makes a slow
    /// suite, and one short enough for an idle machine makes a test that fails
    /// on somebody else's laptop for no reason it can name — the rule
    /// `serve::a_daemon` already follows for its socket.
    fn past(seen: &Arc<AtomicUsize>, was: usize, patience: Duration) -> usize {
        let deadline = Instant::now() + patience;
        loop {
            let now = seen.load(Ordering::SeqCst);
            if now > was || Instant::now() >= deadline {
                return now;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Keeps nudging until the watch is live, so that what follows is not a
    /// race.
    ///
    /// `look_at` hands the path to another thread, so a test that touched a file
    /// immediately afterwards could beat `add_watch` to it. Retrying the
    /// stimulus is what makes "the watch is now armed" an OBSERVED fact rather
    /// than an assumed one.
    fn armed(seen: &Arc<AtomicUsize>, dir: &Path) -> bool {
        let deadline = Instant::now() + PATIENCE;
        let mut n = 0_u32;
        loop {
            let was = seen.load(Ordering::SeqCst);
            n += 1;
            std::fs::write(dir.join(format!("arming-{n}")), b"x").unwrap();
            if past(seen, was, Duration::from_millis(100)) > was {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
        }
    }

    /// Waits until the watcher is back to its wait with no deadline.
    ///
    /// The hold-off is what makes this necessary, and it is not a wrinkle of the
    /// tests: a report puts the thread into a window where the NEXT change is
    /// reported at the TRAILING edge, deliberately. A test that wants to measure
    /// the leading edge must let that window close first — and it does so by
    /// watching the count stop moving, not by sleeping a number.
    fn settled(seen: &Arc<AtomicUsize>) {
        let deadline = Instant::now() + PATIENCE;
        let mut last = seen.load(Ordering::SeqCst);
        let mut since = Instant::now();
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
            let now = seen.load(Ordering::SeqCst);
            if now == last {
                if since.elapsed() > BURST + QUIET {
                    return;
                }
            } else {
                last = now;
                since = Instant::now();
            }
        }
    }

    #[test]
    // THE DEFECT THIS CATCHES: a listing that could not be held when the door
    // pointed at it — a folder of the /private vault while it was shut — is
    // never held after, because `look_at` swallows the same path: the vault
    // opened, and files landing in it were never heard. A stir of a fixed
    // watch (the vault's status, here the sidebar's folder) asks for it again.
    fn a_listing_not_held_is_held_at_the_next_stir() {
        let root = scratch("held-later");
        let side = root.join("side");
        std::fs::create_dir_all(&side).unwrap();
        let later = root.join("later");
        let (mut notice, seen) = counted(Some(&side));
        notice.look_at(&later);
        assert!(armed(&seen, &side), "the fixed watch never came alive");
        settled(&seen);
        std::fs::create_dir_all(&later).unwrap();
        // NOT HEARD YET: nothing holds it.
        let was = seen.load(Ordering::SeqCst);
        std::fs::write(later.join("early"), b"x").unwrap();
        assert_eq!(
            past(&seen, was, Duration::from_millis(300)),
            was,
            "a folder nobody held was heard"
        );
        // THE FIXED WATCH STIRS (the vault's status moving): the listing is
        // held from here on.
        let was = seen.load(Ordering::SeqCst);
        std::fs::write(side.join("status"), b"open").unwrap();
        assert!(
            past(&seen, was, PATIENCE) > was,
            "the fixed watch went deaf"
        );
        assert!(
            armed(&seen, &later),
            "the listing was never held after the stir"
        );
        settled(&seen);
        let was = seen.load(Ordering::SeqCst);
        std::fs::write(later.join("landed"), b"x").unwrap();
        assert!(
            past(&seen, was, PATIENCE) > was,
            "a file landing in the listing went unheard"
        );
        drop(notice);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    // THE DEFECT THIS CATCHES: the window door never re-read at all. A file
    // arriving in the open directory needed the person to leave it and come
    // back, which is the report this whole module was written for.
    fn a_file_created_in_the_watched_directory_stirs_the_door() {
        let dir = scratch("created");
        let (mut notice, seen) = counted(None);
        notice.look_at(&dir);
        assert!(armed(&seen, &dir), "the watch never came alive");

        let was = seen.load(Ordering::SeqCst);
        std::fs::write(dir.join("brand-new"), b"hello").unwrap();
        assert!(
            past(&seen, was, PATIENCE) > was,
            "a file was created and nothing said so"
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: answering the nudge with `stale()`. Writing to a
    // file that already exists moves the FILE's mtime and NOT the directory's,
    // so the poll the footer has always used says no — and the size column goes
    // on lying for as long as the manager is open.
    fn a_file_written_in_place_stirs_though_the_directory_did_not_change() {
        let dir = scratch("in-place");
        let file = dir.join("ledger");
        std::fs::write(&file, b"one").unwrap();
        let before = crate::listing::stamp(&dir);

        let (mut notice, seen) = counted(None);
        notice.look_at(&dir);
        assert!(armed(&seen, &dir), "the watch never came alive");
        // The arming wrote files, which DID move the directory. Take the reading
        // that matters from here.
        let quiet = crate::listing::stamp(&dir);

        let was = seen.load(Ordering::SeqCst);
        std::fs::write(&file, b"one and a half").unwrap();
        assert!(
            past(&seen, was, PATIENCE) > was,
            "a file grew and nothing said so"
        );
        // BOTH STATES, and this is the assertion that pins the design: the
        // directory looks untouched to the oracle the clock uses.
        assert_eq!(
            crate::listing::stamp(&dir),
            quiet,
            "the directory's own stamp moved after all — this test proves nothing"
        );
        assert!(
            before.is_some(),
            "the scratch directory has no stamp at all"
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: one report per file. The door answers with
    // `View::reread`, which is a `readdir` plus an `lstat` and a MIME lookup PER
    // ENTRY — so a report per file makes copying N files cost O(N²), on the one
    // gesture where the listing is largest.
    fn a_hundred_files_arriving_at_once_are_a_handful_of_reports() {
        let dir = scratch("burst");
        let (mut notice, seen) = counted(None);
        notice.look_at(&dir);
        assert!(armed(&seen, &dir), "the watch never came alive");

        let was = seen.load(Ordering::SeqCst);
        let started = Instant::now();
        for n in 0..100 {
            std::fs::write(dir.join(format!("of-many-{n}")), b"x").unwrap();
        }
        let took = started.elapsed();
        assert!(past(&seen, was, PATIENCE) > was, "a burst said nothing");
        // Let the tail land before counting.
        std::thread::sleep(BURST + QUIET * 2);
        let reports = seen.load(Ordering::SeqCst) - was;

        // A CEILING FROM THE CLOCK, never an exact number: how many bursts a
        // hundred writes fall into depends on the machine, and a test that
        // demanded one exact answer would be a test about this laptop.
        let bursts = took.as_millis() / BURST.as_millis() + 1;
        let most = usize::try_from(bursts).unwrap_or(1) * 2 + 2;
        assert!(
            (1..=most).contains(&reports),
            "a hundred files became {reports} reports; expected between 1 and {most}"
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: coalescing that only reports the trailing edge.
    // One file appearing a quarter of a second late reads as a manager that is
    // thinking, and it contradicts the rule the player already follows — what
    // arrives IS the clock.
    fn the_first_change_is_reported_at_once() {
        let dir = scratch("leading");
        let (mut notice, seen) = counted(None);
        notice.look_at(&dir);
        assert!(armed(&seen, &dir), "the watch never came alive");
        // The arming was itself a burst, and the hold-off it opened is exactly
        // what this test must not measure.
        settled(&seen);

        let was = seen.load(Ordering::SeqCst);
        let started = Instant::now();
        std::fs::write(dir.join("now"), b"x").unwrap();
        assert!(past(&seen, was, PATIENCE) > was, "nothing was reported");
        assert!(
            started.elapsed() < QUIET,
            "the first change waited {:?}, which is the hold-off and not the report",
            started.elapsed()
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: `remove_watch` forgotten when the path moves, so
    // every directory ever visited goes on reporting for the rest of the
    // session — and each one of those reports costs a full re-read of a
    // directory nobody is looking at.
    fn looking_somewhere_else_stops_reporting_the_place_one_left() {
        let left = scratch("left");
        let arrived = scratch("arrived");
        let (mut notice, seen) = counted(None);
        notice.look_at(&left);
        assert!(armed(&seen, &left), "the first watch never came alive");
        notice.look_at(&arrived);
        // Observing the SECOND watch alive is what proves the order was taken;
        // without it this test would only be asserting that a race went its way.
        assert!(armed(&seen, &arrived), "the second watch never came alive");
        settled(&seen);

        let was = seen.load(Ordering::SeqCst);
        std::fs::write(left.join("nobody-is-looking"), b"x").unwrap();
        assert_eq!(
            past(&seen, was, QUIET * 4),
            was,
            "the directory that was left is still reporting"
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: `inotify_add_watch` is keyed on the INODE, so
    // watching the sidebar's own directory as the listing hands back the SAME
    // descriptor — and re-pointing the listing away then removes the sidebar's
    // watch too, silently, for the rest of the session. Nothing on screen would
    // say why a pin stopped crossing between the two doors.
    fn the_sidebar_survives_the_listing_walking_through_its_directory() {
        let store = scratch("store");
        let elsewhere = scratch("elsewhere");
        let (mut notice, seen) = counted(Some(&store));
        notice.look_at(&store);
        assert!(armed(&seen, &store), "the watch never came alive");
        notice.look_at(&elsewhere);
        assert!(
            armed(&seen, &elsewhere),
            "the second watch never came alive"
        );

        let was = seen.load(Ordering::SeqCst);
        std::fs::write(store.join("sidebar.json"), b"{}").unwrap();
        assert!(
            past(&seen, was, PATIENCE) > was,
            "the sidebar's watch went with the listing's"
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: a thread parked in a wait nothing can end. One
    // per dropped notice is one per test and one per closed window, and this
    // repository has already been OOM-killed six times in twenty minutes by
    // tests that each held on to something.
    fn a_notice_that_is_dropped_lets_its_thread_go() {
        let dir = scratch("dropped");
        let alive = Arc::new(());
        let held = Arc::clone(&alive);
        let mut notice = Notice::watching(None, move || {
            // Only to own the `Arc`: it dies with the closure, and the closure
            // dies with the thread.
            let _ = &held;
            true
        })
        .unwrap();
        notice.look_at(&dir);
        drop(notice);

        let deadline = Instant::now() + PATIENCE;
        while Arc::strong_count(&alive) > 1 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            Arc::strong_count(&alive),
            1,
            "the notice was dropped and its thread stayed"
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: resting taking the sidebar with it. The footer
    // rests the watch whenever the manager closes — including on losing focus —
    // and `LEADER a` pins an application with no manager open at all, which is
    // exactly when the other door must still hear about it.
    fn resting_drops_the_listing_and_keeps_the_sidebar() {
        let store = scratch("resting-store");
        let dir = scratch("resting-dir");
        let (mut notice, seen) = counted(Some(&store));
        notice.look_at(&dir);
        assert!(armed(&seen, &dir), "the watch never came alive");
        notice.rest();
        assert_eq!(notice.watching_now(), None);
        settled(&seen);
        let was = seen.load(Ordering::SeqCst);
        std::fs::write(dir.join("after-the-rest"), b"x").unwrap();
        assert_eq!(
            past(&seen, was, QUIET * 4),
            was,
            "resting left the listing reporting"
        );

        let was = seen.load(Ordering::SeqCst);
        std::fs::write(store.join("sidebar.json"), b"{}").unwrap();
        assert!(
            past(&seen, was, PATIENCE) > was,
            "resting took the sidebar's watch with it"
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: a preview that answers a keystroke a quarter of
    // a second later.
    //
    // The theme chooser previews by WRITING `theme.json` on every arrow, and
    // every other door repaints from a watch on it. With the listing's
    // coalescing, the first change lands at once and each one behind it waits
    // for `QUIET` — measured here before the fix, 5 ms and then 253. The owner
    // reported it as a preview that lags.
    //
    // Both halves are asserted, because only the pair says the choice is the
    // right one: the fast watch has to be fast, and the coalescing one has to
    // still coalesce — it is what keeps a thousand files copied into an open
    // directory from costing a thousand re-reads.
    fn a_preview_watch_answers_at_once_and_a_listing_still_absorbs_a_burst() {
        // Comfortably under `QUIET`, and twenty times the 5 ms measured: this
        // is a CEILING on the answer, not a sleep waiting for one.
        const PROMPTLY: Duration = Duration::from_millis(100);
        assert!(
            PROMPTLY < QUIET,
            "the window this measures in is longer than the hold-off it is measuring"
        );
        for (pace, prompt) in [(Pace::AtOnce, true), (Pace::Coalesced, false)] {
            let dir = scratch(if prompt { "at-once" } else { "coalesced" });
            let (_notice, seen) = counted_file(&dir, "theme.json", pace);
            let write = |what: &[u8]| ronoctua_xdg::write_atomically(&dir, "theme.json", what);
            // Armed, and then given time to be back at its wait with no
            // deadline — the leading edge is prompt under BOTH paces.
            let deadline = Instant::now() + PATIENCE;
            loop {
                let was = seen.load(Ordering::SeqCst);
                write(b"arming").unwrap();
                if past(&seen, was, Duration::from_millis(100)) > was {
                    break;
                }
                assert!(Instant::now() < deadline, "the watch never came alive");
            }
            settled(&seen);

            // The leading edge, which is the same either way.
            let was = seen.load(Ordering::SeqCst);
            write(b"one").unwrap();
            let led = past(&seen, was, PATIENCE);
            assert!(led > was, "the leading edge was never reported ({pace:?})");

            // AND THE ONE BEHIND IT, which is the whole difference.
            write(b"two").unwrap();
            let after = past(&seen, led, PROMPTLY);
            assert_eq!(
                after > led,
                prompt,
                "{pace:?}: a change right behind the leading edge \
                 {} inside {PROMPTLY:?}",
                if prompt { "did not land" } else { "landed" }
            );
            // The coalescing one is not LOSING it — it is holding it, and the
            // tail still arrives. A watch that dropped changes would pass the
            // line above and be a much worse defect.
            if !prompt {
                assert!(
                    past(&seen, led, PATIENCE) > led,
                    "the held change never arrived at all"
                );
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    // THE DEFECT THIS CATCHES: a watch on ONE file that hears the whole
    // directory. Every door's live theme rides on `theme.json`; the sidebar
    // file and the writer's own scratch file live beside it, and a filter that
    // let them through would repaint five processes for every sidebar pin.
    fn a_one_file_watch_hears_its_file_and_nothing_beside_it() {
        let dir = scratch("one-file");
        let (_notice, seen) = counted_file(&dir, "theme.json", Pace::AtOnce);
        // Armed when the named file itself rings: the `arming-N` files the
        // directory helper writes are exactly what this watch must ignore.
        let deadline = Instant::now() + PATIENCE;
        loop {
            let was = seen.load(Ordering::SeqCst);
            ronoctua_xdg::write_atomically(&dir, "theme.json", b"{\"theme\":\"owl\"}").unwrap();
            if past(&seen, was, Duration::from_millis(100)) > was {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the one-file watch never came alive"
            );
        }
        settled(&seen);

        // A neighbour, and a scratch file: neither rings.
        let was = seen.load(Ordering::SeqCst);
        std::fs::write(dir.join("files-sidebar.json"), b"{}").unwrap();
        std::fs::write(dir.join("theme.json.new"), b"half").unwrap();
        assert_eq!(
            past(&seen, was, Duration::from_millis(400)),
            was,
            "a file beside the watched one rang the door"
        );

        // The atomic rename over the file is what counts.
        let was = seen.load(Ordering::SeqCst);
        ronoctua_xdg::write_atomically(&dir, "theme.json", b"{\"theme\":\"forest\"}").unwrap();
        assert!(
            past(&seen, was, PATIENCE) > was,
            "the watched file was replaced and nothing said so"
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: a repository watched in one of its places. A
    // commit moves `refs/heads/<branch>` and a `git add` rewrites the index in
    // the git directory itself, so a watch on only one of them leaves the git
    // line stale after the other — and a missing folder (a fresh repository
    // has no `logs/`) must not take the others down with it.
    fn several_fixed_directories_each_report_and_a_missing_one_is_skipped() {
        let git = scratch("dirs-git");
        let heads = git.join("refs/heads");
        std::fs::create_dir_all(&heads).unwrap();
        let seen = Arc::new(AtomicUsize::new(0));
        let mine = Arc::clone(&seen);
        let _notice = Notice::on_dirs(
            &[git.clone(), heads.clone(), git.join("logs")],
            &[],
            Pace::AtOnce,
            move || {
                mine.fetch_add(1, Ordering::SeqCst);
                true
            },
        )
        .unwrap();
        assert!(
            armed(&seen, &git),
            "the git directory's watch never came alive"
        );
        assert!(armed(&seen, &heads), "the refs' watch never came alive");
        let was = seen.load(Ordering::SeqCst);
        ronoctua_xdg::write_atomically(&heads, "main", b"0123\n").unwrap();
        assert!(
            past(&seen, was, PATIENCE) > was,
            "a branch moved and nothing said so"
        );
        let _ = std::fs::remove_dir_all(&git);
    }

    #[test]
    // THE DEFECT THIS CATCHES: a tree watched only as it was when the watch
    // began. Pushing `feature/x` makes `refs/remotes/origin/feature` and
    // writes the branch INSIDE it; a watch on the folders that existed hears
    // the folder being made and nothing written into it afterwards — the
    // arrows of the git line stale from the second push on.
    fn a_tree_hears_the_directories_made_in_it_after_the_watch_began() {
        let refs = scratch("tree-refs");
        let origin = refs.join("remotes/origin");
        std::fs::create_dir_all(&origin).unwrap();
        let seen = Arc::new(AtomicUsize::new(0));
        let mine = Arc::clone(&seen);
        let _notice = Notice::on_dirs(&[], std::slice::from_ref(&refs), Pace::AtOnce, move || {
            mine.fetch_add(1, Ordering::SeqCst);
            true
        })
        .unwrap();
        // A DIRECTORY THAT WAS THERE, deep in the tree.
        assert!(
            armed(&seen, &origin),
            "the tree's inner folder was not heard"
        );
        // AND ONE MADE NOW, two levels at once — its making is heard where
        // it was made, and then the report is let go of, so that what is
        // counted next is only what is written INSIDE it.
        let feature = origin.join("feature/deeper");
        std::fs::create_dir_all(&feature).unwrap();
        settled(&seen);
        let was = seen.load(Ordering::SeqCst);
        std::fs::write(feature.join("x"), b"0123\n").unwrap();
        assert!(
            past(&seen, was, PATIENCE) > was,
            "a folder made in the tree after the watch began was never heard"
        );
        // NOT A LINK OUT OF IT.
        let outside = scratch("tree-outside");
        std::os::unix::fs::symlink(&outside, refs.join("out")).unwrap();
        settled(&seen);
        let was = seen.load(Ordering::SeqCst);
        std::fs::write(outside.join("far"), b"x").unwrap();
        assert_eq!(
            past(&seen, was, Duration::from_millis(300)),
            was,
            "a link out of the tree was followed"
        );
        let _ = std::fs::remove_dir_all(&refs);
        let _ = std::fs::remove_dir_all(&outside);
    }

    /// A tree's notice whose reports are counted, and the counter.
    fn counted_trees(trees: Vec<Tree>) -> (Notice, Arc<AtomicUsize>) {
        let seen = Arc::new(AtomicUsize::new(0));
        let mine = Arc::clone(&seen);
        let notice = Notice::on_trees(&[], trees, Pace::AtOnce, move || {
            mine.fetch_add(1, Ordering::SeqCst);
            true
        })
        .unwrap();
        (notice, seen)
    }

    #[test]
    // THE DEFECT THIS CATCHES: one tree — a person's addition of thousands of
    // directories — taking every watch, so a tree that holds its own goes
    // unheard; and a write in a folder a tree skips costing the door a
    // re-read nobody needs.
    fn a_tree_of_its_own_hears_beside_a_full_one_and_not_what_it_skips() {
        let big = scratch("trees-big");
        let small = scratch("trees-small");
        for n in 0..TREE_MOST + 8 {
            std::fs::create_dir(big.join(format!("d{n}"))).unwrap();
        }
        std::fs::create_dir_all(small.join("inner")).unwrap();
        std::fs::create_dir_all(small.join("packages/a/b")).unwrap();
        let skip: Unheard = Arc::new(|p: &Path| {
            p.components()
                .any(|c| c.as_os_str() == "packages" || c.as_os_str() == "history")
        });
        let (_notice, seen) = counted_trees(vec![
            Tree::shared(big.clone()),
            Tree {
                root: small.clone(),
                most: Some(8),
                unheard: Some(skip),
            },
        ]);
        // THE TREE OF ITS OWN KEEPS ITS WATCHES, the shared one full.
        assert!(
            armed(&seen, &small.join("inner")),
            "a tree of its own went unheard because a shared one was full"
        );
        assert!(armed(&seen, &big), "the shared tree's root was not heard");
        // WHAT THE TREE SKIPS is not watched, and a file it names beside
        // the person's is not reported.
        settled(&seen);
        let was = seen.load(Ordering::SeqCst);
        std::fs::write(small.join("packages/a/b/x.el"), b";;").unwrap();
        std::fs::write(small.join("history"), b"()").unwrap();
        assert_eq!(
            past(&seen, was, Duration::from_millis(400)),
            was,
            "what the tree skips was heard"
        );
        let _ = std::fs::remove_dir_all(&big);
        let _ = std::fs::remove_dir_all(&small);
    }

    #[test]
    // WHAT `on_dirs` WAS, it stays — the git line's, the agents', the
    // stick's: its trees share one budget, in the order given, and hear
    // everything in them.
    fn trees_on_dirs_share_one_budget_as_before() {
        let first = scratch("shared-first");
        let second = scratch("shared-second");
        for n in 0..TREE_MOST {
            std::fs::create_dir(first.join(format!("d{n}"))).unwrap();
        }
        std::fs::create_dir(second.join("late")).unwrap();
        let seen = Arc::new(AtomicUsize::new(0));
        let mine = Arc::clone(&seen);
        let _notice = Notice::on_dirs(
            &[],
            &[first.clone(), second.clone()],
            Pace::AtOnce,
            move || {
                mine.fetch_add(1, Ordering::SeqCst);
                true
            },
        )
        .unwrap();
        assert!(armed(&seen, &first), "the first tree was not heard");
        settled(&seen);
        let was = seen.load(Ordering::SeqCst);
        std::fs::write(second.join("late/x"), b"x").unwrap();
        assert_eq!(
            past(&seen, was, Duration::from_millis(400)),
            was,
            "the second tree was heard past the shared budget: `on_dirs` changed"
        );
        let _ = std::fs::remove_dir_all(&first);
        let _ = std::fs::remove_dir_all(&second);
    }
}
