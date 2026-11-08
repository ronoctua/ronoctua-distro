//! The watch thread: what the modules that LISTEN hear.
//!
//! # Event, not clock
//!
//! A module that watches answers [`Watcher::sources`] once, and this thread
//! turns each source into something that WAKES it:
//!
//! - **`Source::Journal`** — ONE `journalctl --follow` for every module that
//!   asked for a match ([`ronoctua_journal`]), started only if one did, at the
//!   loosest priority any of them asked for, from where it was left. Each
//!   entry is offered to every match, in process.
//! - **`Source::Path`** — inotify, through `ronoctua_files::notice` (the one
//!   thread that already waits on the kernel for the file manager). See
//!   "A path" below.
//! - **`Source::Tree`** — a `Source::Path` whose inside is the whole tree
//!   (`ronoctua_files::notice::Notice::on_dirs`): every folder under it,
//!   the ones made later included. See "A tree" below.
//! - **`Source::Lines`** — a long-lived child, by its store path, whose every
//!   line is heard; restarted with a backoff when it ends.
//! - **`Heard::Wake`** — the one clock, armed for the earliest moment a module
//!   asked for ([`Watcher::next_wake`]) and not at all when none did: then the
//!   thread blocks until something is heard.
//!
//! Everything arrives on one channel and is handed to the modules on this one
//! thread, in order — so a module's `heard` is never called twice at once, and
//! it never has to lock against itself.
//!
//! # Rung from outside
//!
//! A wake armed by an action, or a watcher switched on by a preference, is a
//! change this thread would never see by itself: nothing it waits on moved.
//! So the core RINGS it ([`crate::hub::Core::ring_on`]) — the one bell,
//! coalesced: a thousand rings while it is busy are one message — and it
//! looks again at who is on and when each wants waking. A module that came on
//! is `started`, to catch up, and told what its sources are doing now.
//!
//! # Why not the runner's thread
//!
//! Because the runner blocks for an hour inside a rebuild, and a process the
//! kernel killed during that hour is exactly what a watch on the journal
//! exists to write down. The journal is behind a lock of its own (`crate::hub`)
//! so a record written here does not wait for the run to end.
//!
//! # A burst is one publication
//!
//! Everything waiting on the channel is handed out before the mind is
//! published, and the mind is published at most once per [`PUBLISH_EVERY`]
//! while things keep coming: the first at once, the rest together at the end
//! of the window. Five hundred journal lines were five hundred whole minds to
//! every screen, and the loop that draws spends its time on those.
//!
//! # A source that is down is said, and so is its return
//!
//! A journal that cannot be read, a path that cannot be watched, a child that
//! keeps dying: each reaches the module as [`Heard::Lost`] — the journal's as a
//! word of [`Refusal`] (`brain-journal-why-<word>`), the others in the
//! machine's words — so its page refuses with a sentence instead of going
//! quiet. And when it is followed again, [`Heard::Back`], or the page would go
//! on refusing after the restart worked. A follower that ENDS after it had
//! been following is not down: it is started again from its cursor, and
//! nothing was missed. What journalctl itself said goes to the log.
//!
//! # What a module missed is read for it alone
//!
//! A module switched off hears nothing, and the one cursor moves on without
//! it, because it is everybody's. So a module that keeps its own place
//! ([`Watcher::pass`]) is read what came after it by a PASS: a journalctl that
//! ends ([`ronoctua_journal::catch_up_arguments`]), on a thread of its own,
//! handed over in one message ([`Watcher::passed`]). A person asks for one
//! ([`Acted::Pass`]), or the module comes back on; the brain's own start never
//! does, because the follower's cursor already covers that — save for a pass
//! the brain stopped in the middle of, which only it was reading: the module
//! says it is owed ([`Watcher::owed`](crate::module::Watcher::owed)), and it
//! is read again up to where the follower took over ([`Pass::until`](crate::module::Pass::until)).
//!
//! It touches nothing of the follower's. The shared cursor says where the
//! FOLLOWER is, and a pass read somewhere else, for somebody else.
//!
//! A module that came back on hears the present at the same time, and a line
//! both of them read is heard once. What the follower brought first is left
//! out of the pass, and what the pass handed first is skipped when the
//! follower brings it — so a pass that read every line the follower brought
//! says so ([`Passed::ahead`]), being the only one left to move the module's
//! place over what is skipped. A line is known by its cursor and never by when
//! it was written: a clock stepped back makes a later line look older, and the
//! time a line carries would drop lines nobody heard.
//!
//! # The cursor comes last
//!
//! It says a line was heard, and a restart starts after it. So what depends on
//! that line is written first: what each module keeps of it
//! ([`Watcher::keep`]), then the notes. The daemon has no signal handler, so a
//! real stop never reaches [`Watching`]'s `Drop`, and the turn in which the
//! cursor is written is the one moment the two can be made to agree.
//!
//! # A path
//!
//! Watched from its PARENT, narrowed to its name — its birth, its removal, and
//! for a file every write to it — and, while it is a directory, from INSIDE
//! too: what happens to what it holds. A directory born after the start is
//! watched inside from the moment its birth is heard (`crun` makes
//! `$XDG_RUNTIME_DIR/crun` when the first container starts, and a module that
//! heard only the birth would hear no container), and one deleted and made
//! again is watched again: the watch inside carries the directory's identity.
//! The module is told after the watch is raised, so what it reads then plus
//! what the watch hears after is everything.
//!
//! # A tree
//!
//! The same, with the inside being every folder under it, however deep: a
//! folder made below it is held by the notice's own thread the moment the
//! kernel says it was made, before the module is told — so a file written
//! in a folder born after the start is heard (the belongings' plain part:
//! `state/person/software/` is made by the first declaration, and every
//! declaration after it is written inside).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::BufRead as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use ronoctua_files::notice::{Notice, Pace};
use ronoctua_journal::{
    BOOT_ID, Backfill, Cursor, Entry, Follow, Keeper, Match, Priority, Refusal, Shares,
};

use crate::hub::{Bell, Core, rehearsing_here};
use crate::module::{Heard, Passed, Source};
use crate::moment::Moment;

/// The file the journal's cursor is kept in, under the state directory.
pub const CURSOR_FILE: &str = "brain-journal-cursor";

/// How often, at most, the cursor is written while the journal moves.
///
/// Five seconds: a restart hears at most that much again — never less than
/// it missed — and a burst of a thousand lines costs one write.
pub const CURSOR_EVERY: Duration = Duration::from_secs(5);

/// The first wait before a source that ended is started again; it doubles
/// up to [`MOST_BACKOFF`] while the source keeps failing.
pub const FIRST_BACKOFF: Duration = Duration::from_secs(1);

/// The longest wait between two tries of a source that keeps failing.
pub const MOST_BACKOFF: Duration = Duration::from_secs(300);

/// How long a started source has to stay up, without a word, to count as
/// FOLLOWED — what tells a journalctl on a quiet journal from one that gave
/// up at once. Half a second: journalctl refuses a journal it may not read,
/// or a cursor it cannot seek to, in milliseconds.
pub const SETTLE: Duration = Duration::from_millis(500);

/// How often, at most, the mind is published while things keep being heard.
/// See this module's header.
pub const PUBLISH_EVERY: Duration = Duration::from_millis(500);

/// The longest the thread waits for a wake without looking at the clock
/// again. Its wait is measured on a clock that stops while the machine
/// sleeps, and a wake for eight o'clock armed at seven on a laptop that slept
/// from ten past seven to nine would ring at ten to ten. Armed only while a
/// wake is wanted.
pub const WAKE_CHECK: Duration = Duration::from_secs(30);

/// The most messages handed out before the mind is published — so a journal
/// that never stops talking is still published, and still asked about
/// switches.
const DRAIN_MOST: usize = 1024;

/// How long a child has to live for its end to count as a new episode — its
/// backoff starts from the first wait again, and its restart says nothing.
const LIVED: Duration = Duration::from_secs(60);

/// Where journal entries come from — journalctl, or what a test hands in.
pub trait Journalist: Send + Sync {
    /// Starts one journalctl with these arguments
    /// ([`ronoctua_journal::follow_arguments`],
    /// [`ronoctua_journal::pass_arguments`],
    /// [`ronoctua_journal::catch_up_arguments`] or
    /// [`ronoctua_journal::probe_arguments`]).
    ///
    /// # Errors
    ///
    /// It could not be started.
    fn open(&self, args: &[String]) -> Result<Box<dyn Stream>, Refusal>;
}

/// What an opened journal gives.
pub trait Stream: Send {
    /// The next entry; blocks until there is one.
    ///
    /// # Errors
    ///
    /// Its end, or that it could not read ([`Follow::next_entry`]).
    fn next_entry(&mut self) -> Result<Entry, Refusal>;

    /// Whether it is still running `within` after it started
    /// ([`Follow::settled`]). A stream that cannot tell says it is.
    fn settled(&mut self, _within: Duration) -> bool {
        true
    }

    /// Whether it exited with a failure, known once it ended
    /// ([`Follow::failed`]). A stream that cannot tell says it did not.
    fn failed(&self) -> bool {
        false
    }

    /// What it said about itself, for the log.
    fn said(&self) -> String {
        String::new()
    }
}

/// The real journal: journalctl, by its store path.
#[derive(Debug, Clone)]
pub struct Journalctl(pub PathBuf);

impl Journalist for Journalctl {
    fn open(&self, args: &[String]) -> Result<Box<dyn Stream>, Refusal> {
        Ok(Box::new(Follow::spawn(&self.0, args)?))
    }
}

impl Stream for Follow {
    fn next_entry(&mut self) -> Result<Entry, Refusal> {
        Self::next_entry(self)
    }

    fn settled(&mut self, within: Duration) -> bool {
        Self::settled(self, within)
    }

    fn failed(&self) -> bool {
        Self::failed(self)
    }

    fn said(&self) -> String {
        Self::said(self)
    }
}

/// What reaches the one thread that hands things to modules.
enum Msg {
    /// A journal entry the follower read.
    Entry(Entry),
    /// The past, read by the pass: each journal ear's share, by its index,
    /// and the cursor of the last entry the pass read.
    Past {
        shares: Vec<(usize, Vec<Entry>)>,
        last: Option<Cursor>,
    },
    /// The journal is not being followed, and this is why.
    JournalDown(Refusal),
    /// The journal is followed again.
    JournalUp,
    /// A pass asked for one module ended. See this module's header.
    Passed {
        /// Whose.
        module: &'static str,
        /// What each of its journal sources took, by the source's index.
        shares: Vec<(usize, Vec<Entry>)>,
        /// How many lines its rings pushed out.
        lost: u64,
        /// The cursor of the last entry it read.
        last: Option<Cursor>,
        /// Why it could not read, when it could not.
        why: Option<Refusal>,
    },
    /// journalctl refused the saved cursor: the next start is a first one.
    Forget,
    /// Something changed at the path of this ear.
    Path(usize),
    /// A line from the child of this ear.
    Line(usize, String),
    /// This ear's source is down, in the machine's words.
    Down(usize, String),
    /// This ear's source is followed again.
    Up(usize),
    /// Something may have changed outside this thread: look again.
    Rung,
    /// The thread is to end.
    Stop,
}

/// One source of one module, and what its module was told about it.
struct Ear {
    /// Whose.
    module: &'static str,
    /// Which of its sources — what `Heard` says back.
    source: usize,
    /// What it listens to.
    kind: Source,
    /// Why it is down now, in the words the module is told; `None` while it
    /// is followed. A journal ear reads the journal's instead.
    down: Option<String>,
    /// What the module was last told: `Some(why)` after a `Lost`, `None`
    /// before any and after a `Back`.
    told: Option<String>,
}

/// The watch thread's builder: the core, and where the journal comes from.
pub struct Watch {
    core: Arc<Core>,
    journalist: Option<Arc<dyn Journalist>>,
    cursor_dir: Option<PathBuf>,
    backoff: Duration,
}

impl std::fmt::Debug for Watch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watch")
            .field("cursor_dir", &self.cursor_dir)
            .field("backoff", &self.backoff)
            .finish_non_exhaustive()
    }
}

/// The running watch thread. Dropping it takes its bell down, ends the thread
/// and writes the cursor and the waiting notes down.
pub struct Watching {
    core: Arc<Core>,
    bell: Bell,
    tx: Sender<Msg>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for Watching {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watching").finish_non_exhaustive()
    }
}

impl Drop for Watching {
    fn drop(&mut self) {
        self.core.ring_off(&self.bell);
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Msg::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Watch {
    /// The daemon's: journalctl by the store path the unit hands in
    /// ([`ronoctua_journal::VARIABLE`]), the cursor under the state
    /// directory.
    #[must_use]
    pub fn new(core: Arc<Core>) -> Self {
        let journalist =
            ronoctua_journal::program().map(|p| Arc::new(Journalctl(p)) as Arc<dyn Journalist>);
        let cursor_dir = core.store().state.clone();
        Self {
            core,
            journalist,
            cursor_dir,
            backoff: FIRST_BACKOFF,
        }
    }

    /// Another journal — `None` is a machine with no journalctl, which every
    /// journal source hears as [`Refusal::NoProgram`].
    #[must_use]
    pub fn reading(mut self, journalist: Option<Arc<dyn Journalist>>) -> Self {
        self.journalist = journalist;
        self
    }

    /// Keeps the cursor somewhere else; `None` keeps it nowhere.
    #[must_use]
    pub fn keeping_cursor_in(mut self, dir: Option<PathBuf>) -> Self {
        self.cursor_dir = dir;
        self
    }

    /// Starts the backoff from another wait — what a test does.
    #[must_use]
    pub const fn backing_off(mut self, first: Duration) -> Self {
        self.backoff = first;
        self
    }

    /// Starts the thread — and nothing at all when no module watches.
    ///
    /// # Errors
    ///
    /// A thread could not be raised.
    pub fn start(self) -> std::io::Result<Option<Watching>> {
        let mut ears = Vec::new();
        // A MODULE THE SYSTEM SWITCHED OFF IS NEVER LISTENED FOR: not even its
        // sources are asked, so a journal nobody else wants is never followed.
        let listening = || {
            self.core
                .modules()
                .all()
                .filter(|m| !self.core.modules().off(m.manifest().id))
        };
        for m in listening() {
            let Some(w) = m.watcher() else { continue };
            for (source, kind) in w.sources().into_iter().enumerate() {
                ears.push(Ear {
                    module: m.manifest().id,
                    source,
                    kind,
                    down: None,
                    told: None,
                });
            }
        }
        if !listening().any(|m| m.watcher().is_some()) {
            return Ok(None);
        }
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let keeper = Keeper::at(self.cursor_dir.clone(), CURSOR_FILE, CURSOR_EVERY);

        // THE JOURNAL, once for everybody — and only when somebody asked.
        let journal: Vec<(usize, &Match)> = ears
            .iter()
            .enumerate()
            .filter_map(|(at, e)| match &e.kind {
                Source::Journal(m) => Some((at, m)),
                _ => None,
            })
            .collect();
        if !journal.is_empty() {
            match self.journalist.clone() {
                None => {
                    let _ = tx.send(Msg::JournalDown(Refusal::NoProgram));
                }
                Some(journalist) => {
                    let follower = Follower {
                        journalist,
                        after: keeper.saved(),
                        loosest: ronoctua_journal::loosest(journal.iter().map(|(_, m)| *m)),
                        backfilling: journal
                            .iter()
                            .filter(|(_, m)| m.backfill.most() > 0)
                            .map(|(at, m)| (*at, (*m).clone()))
                            .collect(),
                        tx: tx.clone(),
                        stop: Arc::clone(&stop),
                        backoff: self.backoff,
                        core: Arc::clone(&self.core),
                    };
                    std::thread::Builder::new()
                        .name("journal".to_owned())
                        .spawn(move || follower.run())?;
                }
            }
        }

        // PATHS AND TREES, through the kernel.
        let mut paths = BTreeMap::new();
        for (at, ear) in ears.iter().enumerate() {
            let (path, tree) = match &ear.kind {
                Source::Path(path) => (path, false),
                Source::Tree(path) => (path, true),
                Source::Journal(_) | Source::Lines { .. } => continue,
            };
            let (watch, why) = PathWatch::raise(path, tree, &tx, at);
            if let Some(why) = why {
                let _ = tx.send(Msg::Down(at, why));
            }
            paths.insert(at, watch);
        }

        // CHILDREN, one thread each.
        for (at, ear) in ears.iter().enumerate() {
            let Source::Lines { argv } = &ear.kind else {
                continue;
            };
            let (argv, tx, stop, backoff) =
                (argv.clone(), tx.clone(), Arc::clone(&stop), self.backoff);
            std::thread::Builder::new()
                .name("watch-lines".to_owned())
                .spawn(move || lines(&argv, at, &tx, &stop, backoff))?;
        }

        // THE BELL: one message however often it is rung before it is heard.
        let rung = Arc::new(AtomicBool::new(false));
        let bell: Bell = {
            let (tx, rung) = (tx.clone(), Arc::clone(&rung));
            Arc::new(move || {
                if !rung.swap(true, Ordering::AcqRel) {
                    let _ = tx.send(Msg::Rung);
                }
            })
        };
        self.core.ring_on(Arc::clone(&bell));

        let mut dispatcher =
            Dispatcher::new(Arc::clone(&self.core), ears, keeper, tx.clone(), rung);
        dispatcher.paths = paths;
        dispatcher.journalist.clone_from(&self.journalist);
        dispatcher.stop = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("watch".to_owned())
            .spawn(move || dispatcher.run(&rx));
        let thread = match thread {
            Ok(t) => t,
            Err(e) => {
                self.core.ring_off(&bell);
                return Err(e);
            }
        };
        Ok(Some(Watching {
            core: self.core,
            bell,
            tx,
            stop,
            thread: Some(thread),
        }))
    }
}

/// The watches that make one `Source::Path` (or `Source::Tree`) heard. See
/// this module's header.
struct PathWatch {
    /// From the parent, narrowed to the path's name.
    parent: Option<Notice>,
    /// Inside it, while it is a directory — with the directory's identity
    /// (device, inode), so one deleted and made again is watched again.
    inside: Option<(Notice, (u64, u64))>,
    /// Whether the inside is the whole tree under it (`Source::Tree`).
    tree: bool,
}

impl PathWatch {
    /// The watches for one path — its whole tree when `tree` — and why it
    /// cannot be watched when it cannot.
    fn raise(path: &Path, tree: bool, tx: &Sender<Msg>, at: usize) -> (Self, Option<String>) {
        let mut watch = Self {
            parent: None,
            inside: None,
            tree,
        };
        let mut why = None;
        match (path.parent(), path.file_name().and_then(|n| n.to_str())) {
            (Some(parent), Some(name)) if parent.is_dir() => {
                match Notice::on_file(parent, name, Pace::Coalesced, ring(tx, at)) {
                    Ok(n) => watch.parent = Some(n),
                    Err(e) => why = Some(format!("{e:#}")),
                }
            }
            // NOT A DIRECTORY TO WATCH FROM: said, and not a watch that
            // raises nothing and reports nothing.
            (Some(parent), Some(_)) => {
                why = Some(format!(
                    "{} is not a directory, so {} cannot be watched from it",
                    parent.display(),
                    path.display()
                ));
            }
            // The root, or a name that is not text: from inside only.
            _ => {}
        }
        if let Err(e) = watch.follow_inside(path, tx, at) {
            why.get_or_insert(e);
        }
        if watch.parent.is_none() && watch.inside.is_none() && why.is_none() {
            why = Some(format!("{} cannot be watched", path.display()));
        }
        (watch, why)
    }

    /// Raises the watch inside when the path is a directory it is not
    /// watching yet — a new one, or a new directory by the same name — and
    /// drops it when the path is not a directory any more.
    fn follow_inside(&mut self, path: &Path, tx: &Sender<Msg>, at: usize) -> Result<(), String> {
        let now = directory_identity(path);
        match (&self.inside, now) {
            (Some((_, was)), Some(id)) if *was == id => Ok(()),
            (_, Some(id)) => {
                self.inside = None;
                let inside = if self.tree {
                    // EVERY FOLDER BELOW, the ones made later held by the
                    // notice's thread as their birth is read.
                    Notice::on_dirs(&[], &[path.to_path_buf()], Pace::Coalesced, ring(tx, at))
                } else {
                    Notice::watching(Some(path), ring(tx, at))
                }
                .map_err(|e| format!("{e:#}"))?;
                self.inside = Some((inside, id));
                Ok(())
            }
            (_, None) => {
                self.inside = None;
                Ok(())
            }
        }
    }
}

/// A directory's (device, inode), or `None` when the path is not one.
fn directory_identity(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_dir)
        .map(|m| (m.dev(), m.ino()))
}

/// What a notice rings: this ear's path.
fn ring(tx: &Sender<Msg>, at: usize) -> impl Fn() -> bool + Send + 'static {
    let tx = tx.clone();
    move || tx.send(Msg::Path(at)).is_ok()
}

/// Sleeps for `wait`, a slice at a time, and comes back early when told to
/// stop. `true` when it was told.
fn nap(wait: Duration, stop: &AtomicBool) -> bool {
    let until = Instant::now() + wait;
    while Instant::now() < until {
        if stop.load(Ordering::Relaxed) {
            return true;
        }
        std::thread::sleep((until - Instant::now()).min(Duration::from_millis(100)));
    }
    stop.load(Ordering::Relaxed)
}

/// The one follower of the journal, and the pass over its past.
struct Follower {
    journalist: Arc<dyn Journalist>,
    after: Option<Cursor>,
    /// The loosest priority of every journal match.
    loosest: Option<Priority>,
    /// The journal ears that want a past, by index, with their matches.
    backfilling: Vec<(usize, Match)>,
    tx: Sender<Msg>,
    stop: Arc<AtomicBool>,
    backoff: Duration,
    core: Arc<Core>,
}

impl Follower {
    fn run(mut self) {
        // Whether the journal was said down and not said back since.
        let mut down = false;
        // THE PAST FIRST, and only on a first start: with a cursor, what the
        // follower reads after it IS what was missed.
        if self.after.is_none()
            && !self.backfilling.is_empty()
            && let Some(why) = self.read_the_past()
            && !self.said_down(why, &mut down)
        {
            return;
        }
        let mut wait = self.backoff;
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return;
            }
            let args = ronoctua_journal::follow_arguments(
                self.after.as_ref(),
                self.loosest,
                ronoctua_journal::Backfill::Nothing,
            );
            let (took, why) = match self.journalist.open(&args) {
                Err(r) => (false, r),
                Ok(mut follow) => {
                    // FOLLOWED once it stays up, or once it speaks: a quiet
                    // journal is followed long before its next line.
                    let mut took = follow.settled(SETTLE);
                    if took && !self.said_up(&mut down) {
                        return;
                    }
                    let why = loop {
                        match follow.next_entry() {
                            Ok(entry) => {
                                if !took {
                                    took = true;
                                    if !self.said_up(&mut down) {
                                        return;
                                    }
                                }
                                self.after = Some(entry.cursor.clone());
                                if self.tx.send(Msg::Entry(entry)).is_err() {
                                    return;
                                }
                            }
                            Err(r) => break r,
                        }
                    };
                    tell_the_log(&*follow, why);
                    (took, why)
                }
            };
            if self.stop.load(Ordering::Relaxed) {
                return;
            }
            // A CURSOR IT WOULD NOT TAKE is forgotten, or the follower would
            // ask for it for ever and never start — and the next try is a
            // first start from now, at once and without a word: only if that
            // fails too is the journal down.
            if !took && why == Refusal::Unreadable && self.after.is_some() {
                self.after = None;
                if self.tx.send(Msg::Forget).is_err() {
                    return;
                }
                continue;
            }
            // ONE THAT TOOK AND ENDED is started again from its cursor, and
            // nothing was missed: not down. One that did not take is.
            if took {
                wait = self.backoff;
            } else if !self.said_down(why, &mut down) {
                return;
            }
            if nap(wait, &self.stop) {
                return;
            }
            if !took {
                wait = (wait * 2).min(MOST_BACKOFF);
            }
        }
    }

    /// Says the journal is down. `false` when nobody listens any more.
    fn said_down(&self, why: Refusal, down: &mut bool) -> bool {
        *down = true;
        self.tx.send(Msg::JournalDown(why)).is_ok()
    }

    /// Says the journal is back, if it had been said down. `false` when
    /// nobody listens any more.
    fn said_up(&self, down: &mut bool) -> bool {
        if !std::mem::take(down) {
            return true;
        }
        self.tx.send(Msg::JournalUp).is_ok()
    }

    /// Reads the past once, and hands each ear its share. `Some` with why it
    /// could not.
    fn read_the_past(&mut self) -> Option<Refusal> {
        let wanted: Vec<Match> = self.backfilling.iter().map(|(_, m)| m.clone()).collect();
        let window = ronoctua_journal::widest(wanted.iter().map(|m| m.backfill))?;
        let now_s = u64::try_from(self.core.now()).unwrap_or(0);
        // WHEN THIS BOOT BEGAN, and which boot it is — only when a `since`
        // and this boot are both asked for, and the pass reaches past one of
        // them.
        let (boot_began_s, boot) = if window.needs_the_boot() {
            let mut probe = match self.journalist.open(&ronoctua_journal::probe_arguments()) {
                Ok(p) => p,
                Err(r) => return Some(r),
            };
            match probe.next_entry() {
                // AN ENTRY WITH NO BOOT is taken as a boot nobody's: this
                // boot then hears nothing old, rather than every boot's.
                Ok(first) => (
                    Some(first.realtime_us / 1_000_000),
                    Some(first.fields.get(BOOT_ID).cloned().unwrap_or_default()),
                ),
                // A BOOT WITH NOTHING WRITTEN YET has nothing of its own to
                // hear: a boot id no entry has.
                Err(Refusal::Ended) => (None, Some(String::new())),
                Err(r) => {
                    tell_the_log(&*probe, r);
                    return Some(r);
                }
            }
        } else {
            (None, None)
        };
        // THE PASS READS AT THE FOLLOWER'S PRIORITY, not the backfilling
        // matches': the follower goes on from the last entry the pass read,
        // so a pass read at `err` would leave every quieter line written
        // after the boot's last error for the follower to hand to the ears
        // that asked for no past, as news. Each share still takes only what
        // its own match accepts.
        let args = ronoctua_journal::pass_arguments(self.loosest, window, boot_began_s, now_s);
        let mut pass = match self.journalist.open(&args) {
            Ok(p) => p,
            Err(r) => return Some(r),
        };
        let mut shares = Shares::new(wanted, now_s.saturating_mul(1_000_000), boot);
        let ended = loop {
            match pass.next_entry() {
                Ok(entry) => shares.offer(&entry),
                Err(r) => break r,
            }
        };
        let (taken, last) = shares.taken();
        if last.is_some() {
            self.after.clone_from(&last);
        }
        let shares = self
            .backfilling
            .iter()
            .map(|(at, _)| *at)
            .zip(taken)
            .collect();
        let _ = self.tx.send(Msg::Past { shares, last });
        (ended != Refusal::Ended).then(|| {
            tell_the_log(&*pass, ended);
            ended
        })
    }
}

/// One pass asked for one module — a journalctl that ends, read on a thread
/// of its own because the follower sits blocked in its next line and the
/// watch thread must never wait, and handed over in one message. See this
/// module's header.
struct Reader {
    module: &'static str,
    journalist: Arc<dyn Journalist>,
    /// Its journal sources: which one, and its match with a ring of the size
    /// asked for.
    ears: Vec<(usize, Match)>,
    after: Option<Cursor>,
    /// The line it stops before, unread ([`Pass::until`](crate::module::Pass::until)).
    until: Option<Cursor>,
    tx: Sender<Msg>,
    stop: Arc<AtomicBool>,
}

impl Reader {
    fn run(self) {
        // AT ITS OWN MATCHES' PRIORITY, not the follower's: nothing goes on
        // from where this one ends, so it carries no line nobody here wants.
        let priority = ronoctua_journal::loosest(self.ears.iter().map(|(_, m)| m));
        let args = ronoctua_journal::catch_up_arguments(self.after.as_ref(), priority);
        let Some(mut read) = self.read(&args) else {
            return;
        };
        // A CURSOR IT WOULD NOT TAKE stops only this one try, and this boot
        // is what a module with no cursor reads. One that gave a line took
        // its cursor, and what failed came after: reading this boot over it
        // would hand the module what it already holds.
        if self.after.is_some()
            && read.why == Some(Refusal::Unreadable)
            && read.shares.last().is_none()
        {
            tracing::info!(module = self.module, said = %read.said, "a pass was refused the place it was to start from, and reads this boot");
            let args = ronoctua_journal::catch_up_arguments(None, priority);
            let Some(again) = self.read(&args) else {
                return;
            };
            read = again;
        }
        let Read { shares, why, said } = read;
        if let Some(why) = why {
            tracing::warn!(module = self.module, why = why.id(), said = %said, "a pass could not read the journal to its end");
        }
        let lost = shares.lost().iter().sum();
        let (taken, last) = shares.taken();
        let shares = self.ears.iter().map(|(at, _)| *at).zip(taken).collect();
        let _ = self.tx.send(Msg::Passed {
            module: self.module,
            shares,
            lost,
            last,
            why,
        });
    }

    /// Reads one journalctl to its end. `None` when the thread was told to
    /// stop on the way.
    fn read(&self, args: &[String]) -> Option<Read> {
        let wanted = self.ears.iter().map(|(_, m)| m.clone()).collect();
        let mut shares = Shares::new(wanted, 0, None);
        let mut pass = match self.journalist.open(args) {
            Ok(p) => p,
            Err(r) => {
                return Some(Read {
                    shares,
                    why: Some(r),
                    said: String::new(),
                });
            }
        };
        let ended = loop {
            if self.stop.load(Ordering::Relaxed) {
                return None;
            }
            match pass.next_entry() {
                // WHERE IT WAS TO STOP is an end like the journal's.
                Ok(entry) if self.until.as_ref() == Some(&entry.cursor) => break Refusal::Ended,
                Ok(entry) => shares.offer(&entry),
                Err(r) => break r,
            }
        };
        // A FAILURE AFTER A LINE ends like the end of the journal, and it is
        // not: what comes after it was never read.
        let why = match ended {
            Refusal::Ended if pass.failed() => Some(Refusal::Unreadable),
            Refusal::Ended => None,
            refused => Some(refused),
        };
        Some(Read {
            shares,
            why,
            said: pass.said(),
        })
    }
}

/// What one journalctl gave a pass.
struct Read {
    /// What its matches took.
    shares: Shares,
    /// Why it stopped short of the end of the journal, when it did.
    why: Option<Refusal>,
    /// What it said as it stopped, for the log.
    said: String,
}

/// What journalctl said as the follower lost it, in its own words, for the
/// log — the brain's page shows the log.
fn tell_the_log(stream: &dyn Stream, why: Refusal) {
    let said = stream.said();
    tracing::warn!(why = why.id(), said = %said, "the journal is not being followed");
}

/// Follows one child's lines for as long as the thread lives, starting it
/// again with a backoff when it ends.
///
/// Its end is said when lines are missed for longer than a moment: when the
/// child did not take (it ended before it spoke or settled), or when it lived
/// less than [`LIVED`] — then it keeps dying, and the next try waits longer.
/// One that lived long and ended once is started again after the first wait,
/// and says nothing.
fn lines(argv: &[String], at: usize, tx: &Sender<Msg>, stop: &AtomicBool, first: Duration) {
    // BY STORE PATH, never by name: a unit's `PATH` is minimal, and a name
    // would fail far from its cause — refused here, with the reason.
    let Some(program) = argv.first().filter(|p| Path::new(p).is_absolute()) else {
        let _ = tx.send(Msg::Down(
            at,
            format!("{argv:?} does not start with a program's absolute path"),
        ));
        return;
    };
    let mut wait = first;
    let mut down = false;
    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let began = Instant::now();
        let spawned = std::process::Command::new(program)
            .args(&argv[1..])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();
        let (took, lost) = match spawned {
            Ok(mut child) => {
                let mut took = ronoctua_journal::settles(&mut child, SETTLE);
                if took && std::mem::take(&mut down) && tx.send(Msg::Up(at)).is_err() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
                if let Some(out) = child.stdout.take() {
                    for line in std::io::BufReader::new(out).lines() {
                        let Ok(line) = line else { break };
                        let mut gone = stop.load(Ordering::Relaxed);
                        if !took {
                            took = true;
                            gone |= std::mem::take(&mut down) && tx.send(Msg::Up(at)).is_err();
                        }
                        if gone || tx.send(Msg::Line(at, line)).is_err() {
                            let _ = child.kill();
                            let _ = child.wait();
                            return;
                        }
                    }
                }
                let lost = match child.wait() {
                    Ok(status) => format!("{program} ended: {status}"),
                    Err(e) => format!("{program}: {e}"),
                };
                (took, lost)
            }
            Err(e) => (false, format!("{program}: {e}")),
        };
        let lived = began.elapsed() >= LIVED;
        if lived {
            wait = first;
        }
        if !took || !lived {
            down = true;
            if tx.send(Msg::Down(at, lost)).is_err() {
                return;
            }
        } else {
            tracing::info!(what = %lost, "a watched child ended after a long life; starting it again");
        }
        if nap(wait, stop) {
            return;
        }
        wait = (wait * 2).min(MOST_BACKOFF);
    }
}

/// The one thread that hands what was heard to the modules.
struct Dispatcher {
    core: Arc<Core>,
    ears: Vec<Ear>,
    keeper: Keeper,
    /// For the watches raised later — a directory born after the start.
    tx: Sender<Msg>,
    /// The bell's: whether a ring is on its way. Cleared as it is heard.
    rung: Arc<AtomicBool>,
    /// The modules switched on, as last looked.
    on: BTreeSet<&'static str>,
    /// Why the journal is not followed, while it is not.
    journal_down: Option<Refusal>,
    /// The path watches, by ear.
    paths: BTreeMap<usize, PathWatch>,
    /// When each module was last woken — so a module that keeps asking for a
    /// moment already past is woken once a second at most, never in a spin.
    woke: BTreeMap<&'static str, Moment>,
    /// When the mind was last published from here.
    published: Option<Instant>,
    /// When it is to be published, when something waits for its turn.
    publish_due: Option<Instant>,
    /// Where a pass reads from; `None` on a machine with no journalctl.
    journalist: Option<Arc<dyn Journalist>>,
    /// What tells a pass on its way that the thread is ending.
    stop: Arc<AtomicBool>,
    /// The modules a pass is being read for, each with the lines the follower
    /// handed it meanwhile. Kept only while the pass lasts.
    passing: BTreeMap<&'static str, HashSet<Cursor>>,
    /// The lines the last pass that handed any handed each module, which the
    /// follower may still bring. As big as the module's rings, and it can
    /// stay: a cursor matches its own line and no other, whatever the clock
    /// did, so nothing here can silence a module.
    handed: BTreeMap<&'static str, HashSet<Cursor>>,
    /// Whether it has looked at who is on yet: everybody comes on at the
    /// first look, and only after it does coming on mean a person did it.
    looked: bool,
}

impl Dispatcher {
    fn new(
        core: Arc<Core>,
        ears: Vec<Ear>,
        keeper: Keeper,
        tx: Sender<Msg>,
        rung: Arc<AtomicBool>,
    ) -> Self {
        Self {
            core,
            ears,
            keeper,
            tx,
            rung,
            on: BTreeSet::new(),
            journal_down: None,
            paths: BTreeMap::new(),
            woke: BTreeMap::new(),
            published: None,
            publish_due: None,
            journalist: None,
            stop: Arc::new(AtomicBool::new(false)),
            passing: BTreeMap::new(),
            handed: BTreeMap::new(),
            looked: false,
        }
    }

    fn run(mut self, rx: &Receiver<Msg>) {
        // CATCH UP FIRST: what a module missed while the brain was off.
        self.reconcile();
        self.publish(Instant::now());
        loop {
            let first = match self.patience(Instant::now()) {
                None => match rx.recv() {
                    Ok(m) => Some(m),
                    Err(_) => return,
                },
                Some(d) => match rx.recv_timeout(d) {
                    Ok(m) => Some(m),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                },
            };
            // AND EVERYTHING ELSE WAITING, before one publication.
            let mut next = first;
            let mut handed = 0;
            while let Some(msg) = next {
                if matches!(msg, Msg::Stop) {
                    self.leave();
                    return;
                }
                self.hear(msg);
                handed += 1;
                next = if handed < DRAIN_MOST {
                    rx.try_recv().ok()
                } else {
                    None
                };
            }
            self.reconcile();
            let now = Instant::now();
            self.keep_cursor(now);
            self.core.save_journal_if_due(now);
            self.wake();
            self.publish(now);
        }
    }

    /// On the way out: the cursor, what it covers, and what is left to
    /// publish.
    fn leave(&mut self) {
        self.write_cursor();
        self.core.refresh();
    }

    /// The ids of the modules switched on, by one reading of the preferences.
    fn switched_on(&self) -> BTreeSet<&'static str> {
        let modules = self.core.modules();
        self.core.shared.with(|mind| {
            modules
                .all()
                .filter(|m| {
                    !modules.off(m.manifest().id) && mind.prefs.of_module(m.manifest()).enabled
                })
                .map(|m| m.manifest().id)
                .collect()
        })
    }

    /// Looks again at who is on: a watcher that came on is started — to
    /// catch up on what it missed while it was off, and read for it when a
    /// person switched it on — one that went off is told so, the passes asked
    /// for begin, and every module on is told what its sources are doing now.
    fn reconcile(&mut self) {
        let now_on = self.switched_on();
        let came: Vec<&'static str> = now_on.difference(&self.on).copied().collect();
        let went: Vec<&'static str> = self.on.difference(&now_on).copied().collect();
        self.on = now_on;
        let now = self.core.now();
        for id in went {
            if let Some(w) = self.core.modules().find(id).and_then(|m| m.watcher()) {
                w.stopped(now, &self.core.ctx(id, now, rehearsing_here()));
            }
        }
        for id in came {
            let mut owed = false;
            if let Some(w) = self.core.modules().find(id).and_then(|m| m.watcher()) {
                w.started(now, &self.core.ctx(id, now, rehearsing_here()));
                owed = w.owed();
            }
            // NOT AT THE FIRST LOOK: the brain itself is starting, and what
            // its modules missed is what the follower reads after its cursor —
            // save what a pass the brain stopped in was reading, which nothing
            // else will.
            if self.looked || owed {
                self.begin_pass(id, now);
            }
        }
        self.looked = true;
        for id in self.core.take_passes() {
            self.begin_pass(id, now);
        }
        self.settle();
    }

    /// Starts a pass for a module, if it wants one and none is on its way.
    fn begin_pass(&mut self, id: &'static str, now: Moment) {
        if self.passing.contains_key(id) {
            return;
        }
        let Some(w) = self.core.modules().find(id).and_then(|m| m.watcher()) else {
            return;
        };
        let ctx = self.core.ctx(id, now, rehearsing_here());
        let Some(pass) = w.pass(now, &ctx) else {
            return;
        };
        // EACH OF ITS MATCHES WITH A RING OF ITS OWN, which is what `Shares`
        // already keeps: `Reader` names no boot to it, so `ThisBoot` covers
        // all that was read, however many boots a cursor reaches across.
        let ears: Vec<(usize, Match)> = self
            .ears
            .iter()
            .filter(|ear| ear.module == id)
            .filter_map(|ear| match &ear.kind {
                Source::Journal(m) => {
                    let backfill = Backfill::ThisBoot { most: pass.most };
                    Some((
                        ear.source,
                        Match {
                            backfill,
                            ..m.clone()
                        },
                    ))
                }
                _ => None,
            })
            .collect();
        // EVERY PASS ASKED FOR IS ANSWERED, even one that cannot begin: a
        // module waiting for it would say it is still reading.
        let why = match &self.journalist {
            _ if ears.is_empty() => None,
            None => Some(Refusal::NoProgram),
            Some(journalist) => {
                let reader = Reader {
                    module: id,
                    journalist: Arc::clone(journalist),
                    ears,
                    after: pass.after,
                    until: pass.until,
                    tx: self.tx.clone(),
                    stop: Arc::clone(&self.stop),
                };
                let spawned = std::thread::Builder::new()
                    .name("journal-pass".to_owned())
                    .spawn(move || reader.run());
                if spawned.is_ok() {
                    self.passing.insert(id, HashSet::new());
                    return;
                }
                Some(Refusal::Unstartable)
            }
        };
        w.passed(
            Passed {
                shares: &[],
                lost: 0,
                last: None,
                why,
                ahead: false,
            },
            &ctx,
        );
    }

    /// Tells every module on what changed about its sources since it was
    /// last told: `Lost` for one that went down (or down for another reason),
    /// `Back` for one followed again.
    fn settle(&mut self) {
        let now = self.core.now();
        for at in 0..self.ears.len() {
            let ear = &self.ears[at];
            if !self.on.contains(ear.module) {
                continue;
            }
            let down = match ear.kind {
                Source::Journal(_) => self.journal_down.map(|r| r.id().to_owned()),
                _ => ear.down.clone(),
            };
            if down == ear.told {
                continue;
            }
            let (module, source) = (ear.module, ear.source);
            self.ears[at].told.clone_from(&down);
            match &down {
                Some(why) => self.tell(module, now, Heard::Lost { source, why }),
                None => self.tell(module, now, Heard::Back { source }),
            }
        }
    }

    /// Hands one thing heard to whoever listens to it.
    fn hear(&mut self, msg: Msg) {
        let now = self.core.now();
        match msg {
            Msg::Entry(entry) => {
                self.keeper.note(entry.cursor.clone());
                for ear in &self.ears {
                    if let Source::Journal(m) = &ear.kind
                        && self.on.contains(ear.module)
                        && m.accepts(&entry)
                    {
                        // A PASS GOT HERE FIRST: heard.
                        if self
                            .handed
                            .get(ear.module)
                            .is_some_and(|handed| handed.contains(&entry.cursor))
                        {
                            continue;
                        }
                        // THE FOLLOWER GOT HERE FIRST: the pass on its way
                        // will not hand this one.
                        if let Some(heard) = self.passing.get_mut(ear.module) {
                            heard.insert(entry.cursor.clone());
                        }
                        self.tell(
                            ear.module,
                            now,
                            Heard::Journal {
                                source: ear.source,
                                entry: &entry,
                            },
                        );
                    }
                }
            }
            Msg::Past { shares, last } => {
                if let Some(last) = last {
                    self.keeper.note(last);
                }
                for (at, entries) in shares {
                    let Some(ear) = self.ears.get(at) else {
                        continue;
                    };
                    if !self.on.contains(ear.module) {
                        continue;
                    }
                    for entry in &entries {
                        self.tell(
                            ear.module,
                            now,
                            Heard::Journal {
                                source: ear.source,
                                entry,
                            },
                        );
                    }
                }
            }
            Msg::Passed {
                module,
                mut shares,
                lost,
                last,
                why,
            } => {
                // WHAT IS LEFT is what the follower did not bring, and it
                // may still be on its way, even for a module that is off: it
                // may come back on first. One that handed nothing leaves the
                // last one's lines as they are.
                //
                // AND A PASS THAT READ EVERY LINE THE FOLLOWER BROUGHT went past
                // its place: what the follower brings up to where the pass
                // ended is skipped, never told, so the module is told so here,
                // or its place stays behind lines it already counted.
                let mut ahead = false;
                if let Some(heard) = self.passing.remove(module) {
                    let read: HashSet<&Cursor> = shares
                        .iter()
                        .flat_map(|(_, entries)| entries)
                        .map(|e| &e.cursor)
                        .collect();
                    ahead = !heard.is_empty() && heard.iter().all(|c| read.contains(c));
                    for (_, entries) in &mut shares {
                        entries.retain(|e| !heard.contains(&e.cursor));
                    }
                }
                let handed: HashSet<Cursor> = shares
                    .iter()
                    .flat_map(|(_, entries)| entries)
                    .map(|e| e.cursor.clone())
                    .collect();
                if !handed.is_empty() {
                    self.handed.insert(module, handed);
                }
                // AND THE CURSOR STAYS WHERE THE FOLLOWER LEFT IT.
                if let Some(w) = self.core.modules().find(module).and_then(|m| m.watcher()) {
                    w.passed(
                        Passed {
                            shares: &shares,
                            lost,
                            last: last.as_ref(),
                            why,
                            ahead,
                        },
                        &self.core.ctx(module, now, rehearsing_here()),
                    );
                }
            }
            Msg::JournalDown(why) => {
                self.journal_down = Some(why);
                self.settle();
            }
            Msg::JournalUp => {
                self.journal_down = None;
                self.settle();
            }
            Msg::Forget => self.keeper.forget(),
            Msg::Path(at) => {
                // THE WATCH FIRST, THEN THE WORD: what the module reads now,
                // plus what the watch hears from here on, is everything.
                self.rewatch(at);
                if let Some(ear) = self.ears.get(at)
                    && let Source::Path(path) | Source::Tree(path) = &ear.kind
                    && self.on.contains(ear.module)
                {
                    self.tell(
                        ear.module,
                        now,
                        Heard::Path {
                            source: ear.source,
                            path,
                        },
                    );
                }
            }
            Msg::Line(at, line) => {
                if let Some(ear) = self.ears.get(at)
                    && self.on.contains(ear.module)
                {
                    self.tell(
                        ear.module,
                        now,
                        Heard::Line {
                            source: ear.source,
                            line: &line,
                        },
                    );
                }
            }
            Msg::Down(at, why) => {
                tracing::warn!(why = %why, "a watched source is down");
                if let Some(ear) = self.ears.get_mut(at) {
                    ear.down = Some(why);
                }
                self.settle();
            }
            Msg::Up(at) => {
                if let Some(ear) = self.ears.get_mut(at) {
                    ear.down = None;
                }
                self.settle();
            }
            Msg::Rung => {
                // CLEARED BEFORE LOOKING, so a ring while it looks is heard.
                self.rung.store(false, Ordering::Release);
                self.reconcile();
            }
            Msg::Stop => {}
        }
    }

    /// Follows a path's directory from inside when it became one, and says
    /// whether it can be.
    fn rewatch(&mut self, at: usize) {
        let Some(Source::Path(path) | Source::Tree(path)) = self.ears.get(at).map(|e| &e.kind)
        else {
            return;
        };
        let Some(watch) = self.paths.get_mut(&at) else {
            return;
        };
        // A PATH WITH NO PARENT WATCH is down for good, and its reason stays.
        if watch.parent.is_none() {
            return;
        }
        let followed = watch.follow_inside(path, &self.tx, at);
        self.ears[at].down = followed.err();
        self.settle();
    }

    /// Hands one thing to one module.
    fn tell(&self, module: &'static str, now: Moment, heard: Heard<'_>) {
        let Some(w) = self.core.modules().find(module).and_then(|m| m.watcher()) else {
            return;
        };
        w.heard(heard, &self.core.ctx(module, now, rehearsing_here()));
    }

    /// When each watcher that is switched on next wants a wake, no earlier
    /// than a second after its last one.
    fn wanted(&self, now: Moment) -> Vec<(&'static str, Moment)> {
        self.core
            .modules()
            .all()
            .filter_map(|m| {
                let id = m.manifest().id;
                if !self.on.contains(id) {
                    return None;
                }
                let at = m.watcher()?.next_wake(now)?;
                Some((id, self.woke.get(id).map_or(at, |w| at.max(w + 1))))
            })
            .collect()
    }

    /// How long to wait for the next message: until the earliest wake (and
    /// never longer than [`WAKE_CHECK`] while one is armed), the cursor's
    /// turn, the notes' turn or the publication's — and `None`, no clock at
    /// all, when none of them is wanted.
    fn patience(&self, now: Instant) -> Option<Duration> {
        let moment = self.core.now();
        let wake = self
            .wanted(moment)
            .into_iter()
            .map(|(_, at)| at)
            .min()
            .map(|at| {
                Duration::from_secs(u64::try_from(at.saturating_sub(moment)).unwrap_or(0))
                    .min(WAKE_CHECK)
            });
        let deadline = [self.keeper.due(), self.publish_due, self.core.journal_due()]
            .into_iter()
            .flatten()
            .min()
            .map(|at| at.saturating_duration_since(now));
        match (wake, deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Wakes every module whose moment has come.
    fn wake(&mut self) {
        let now = self.core.now();
        for (id, at) in self.wanted(now) {
            if at <= now {
                self.woke.insert(id, now);
                self.tell(id, now, Heard::Wake);
            }
        }
    }

    /// Writes the cursor down when its turn has come.
    fn keep_cursor(&mut self, now: Instant) {
        if self.keeper.due().is_some_and(|at| at <= now) {
            self.write_cursor();
        }
    }

    /// Writes the cursor down, and first what it covers: each module's own
    /// list, then the notes, so a cursor never says a line was heard that a
    /// restart would not find. See this module's header.
    fn write_cursor(&mut self) {
        let now = self.core.now();
        for id in &self.on {
            if let Some(w) = self.core.modules().find(id).and_then(|m| m.watcher()) {
                w.keep(now, &self.core.ctx(id, now, rehearsing_here()));
            }
        }
        self.core.save_journal();
        if let Err(e) = self.keeper.flush() {
            tracing::warn!(error = %e, "the journal's cursor could not be written");
        }
    }

    /// Publishes what changed — at once when nothing was published for
    /// [`PUBLISH_EVERY`], else at the end of that window, once.
    fn publish(&mut self, now: Instant) {
        if !self.core.stale() {
            self.publish_due = None;
            return;
        }
        if let Some(at) = self.published
            && now < at + PUBLISH_EVERY
        {
            self.publish_due = Some(at + PUBLISH_EVERY);
            return;
        }
        self.core.refresh();
        self.published = Some(now);
        self.publish_due = None;
    }
}

/// The ids of the modules this core would watch for — what `ronoctua-brain`
/// logs at start.
#[must_use]
pub fn listeners(core: &Core) -> Vec<&'static str> {
    core.modules()
        .all()
        .filter(|m| m.watcher().is_some() && !core.modules().off(m.manifest().id))
        .map(|m| m.manifest().id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{Fake, Hand, manifest};
    use crate::mind::{Mind, Shared};
    use crate::module::{Acted, Action, Arg, Manifest, Module, Noted, Pass};
    use crate::prefs::{Prefs, Where};
    use crate::registry::Registry;
    use crate::wire::Knob;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-brain-watch-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn a_core(modules: Vec<Box<dyn Module>>) -> Arc<Core> {
        Arc::new(Core::new(
            Arc::new(Shared::of(Prefs::default())),
            Registry::of(modules).expect("the registry"),
            Where::default(),
            Box::new(Hand::at(1000)),
        ))
    }

    /// Waits until the mind says what the test wants, or gives up saying so.
    fn until(core: &Core, what: &str, wanted: impl Fn(&Mind) -> bool) -> Mind {
        let deadline = Instant::now() + Duration::from_secs(10);
        let (mut mind, mut seen) = core.shared.snapshot();
        while !wanted(&mind) {
            assert!(Instant::now() < deadline, "never: {what}");
            (mind, seen) = core.shared.wait_past(seen, Duration::from_millis(50));
        }
        mind
    }

    /// What a module noted, in order.
    fn noted(mind: &Mind, module: &str) -> Vec<String> {
        mind.journal
            .iter()
            .filter(|r| r.task == module)
            .map(|r| {
                r.args
                    .iter()
                    .find_map(|(_, a)| match a {
                        Arg::Text(t) => Some(t.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| r.says.clone())
            })
            .collect()
    }

    /// A module that writes down every message it hears, every loss and
    /// every return.
    fn scribe(id: &'static str, sources: Vec<Source>) -> Fake {
        Fake::watch(id).listens(sources, |heard, ctx| {
            let text = match heard {
                Heard::Journal { entry, .. } => entry.message.clone(),
                Heard::Path { path, .. } => format!("path {}", path.display()),
                Heard::Line { line, .. } => format!("line {line}"),
                Heard::Wake => "wake".to_owned(),
                Heard::Lost { why, .. } => format!("lost {why}"),
                Heard::Back { .. } => "back".to_owned(),
            };
            ctx.record(Noted::of("scribe-noted").with("what", Arg::Text(text)));
        })
    }

    /// An entry of THIS boot, `n` seconds after the epoch.
    fn entry(n: u64, message: &str, priority: u8) -> Entry {
        entry_of("now", n, message, priority)
    }

    fn entry_of(boot: &str, n: u64, message: &str, priority: u8) -> Entry {
        Entry {
            message: message.to_owned(),
            priority: Some(priority),
            cursor: Cursor(format!("s=t;i={n}")),
            realtime_us: n * 1_000_000,
            fields: BTreeMap::from([(BOOT_ID.to_owned(), boot.to_owned())]),
            ..Entry::default()
        }
    }

    /// A JOURNAL THAT ANSWERS ITS ARGUMENTS the way journalctl does —
    /// `--boot`, `--since`, `--priority`, `--after-cursor` and `--lines`,
    /// counted BEFORE anything the brain filters in process, and the last
    /// ten of the selection for a `--follow` that names no `--lines`. A
    /// fixture that ignored them would pass a pass that journalctl cuts.
    ///
    /// Every opening is written down. The first `refusing` openings give up
    /// at once, unreadable; the next pass, with `cutting` set, fails after
    /// that many lines; each `--follow` after them takes the next live
    /// channel, and with none left it cannot start.
    struct Journald {
        /// Everything written before the watch started, oldest first.
        past: Vec<Entry>,
        /// The boot the machine is in.
        boot: &'static str,
        /// Now, as journalctl reads it, in seconds.
        now_s: u64,
        live: Mutex<VecDeque<mpsc::Receiver<Entry>>>,
        refusing: AtomicUsize,
        cutting: AtomicUsize,
        opened: Mutex<Vec<Vec<String>>>,
    }

    impl Journald {
        fn with(past: Vec<Entry>, lives: usize) -> (Arc<Self>, Vec<mpsc::Sender<Entry>>) {
            let (senders, receivers): (Vec<_>, VecDeque<_>) =
                (0..lives).map(|_| mpsc::channel()).unzip();
            (
                Arc::new(Self {
                    past,
                    boot: "now",
                    now_s: 1000,
                    live: Mutex::new(receivers),
                    refusing: AtomicUsize::new(0),
                    cutting: AtomicUsize::new(0),
                    opened: Mutex::new(Vec::new()),
                }),
                senders,
            )
        }

        fn opened(&self) -> Vec<Vec<String>> {
            self.opened.lock().expect("opened").clone()
        }
    }

    /// What journalctl would select, by the arguments.
    fn selects(args: &[String], boot: &str, now_s: u64) -> impl Fn(&Entry) -> bool + use<> {
        let mut this_boot = false;
        let mut since_us = 0u64;
        let mut most_priority = ronoctua_journal::DEBUG;
        for a in args {
            if a == "--boot" {
                this_boot = true;
            } else if let Some(s) = a.strip_prefix("--since=-") {
                let seconds: u64 = s.trim_end_matches('s').parse().expect("since");
                since_us = now_s.saturating_sub(seconds) * 1_000_000;
            } else if let Some(s) = a.strip_prefix("--since=@") {
                since_us = s.parse::<u64>().expect("since") * 1_000_000;
            } else if let Some(p) = a.strip_prefix("--priority=0..") {
                most_priority = p.parse().expect("priority");
            }
        }
        let boot = boot.to_owned();
        move |e: &Entry| {
            (!this_boot || e.fields.get(BOOT_ID) == Some(&boot))
                && e.realtime_us >= since_us
                && e.priority.unwrap_or(ronoctua_journal::INFO) <= most_priority
        }
    }

    /// Its lines, then an end that says how it went.
    struct Scripted {
        lines: std::vec::IntoIter<Entry>,
        failed: bool,
    }

    impl Stream for Scripted {
        fn next_entry(&mut self) -> Result<Entry, Refusal> {
            self.lines.next().ok_or(Refusal::Ended)
        }
        fn failed(&self) -> bool {
            self.failed
        }
    }

    /// The past, then whatever the test sends that journalctl would pass.
    struct Live {
        past: std::vec::IntoIter<Entry>,
        live: mpsc::Receiver<Entry>,
        passes: Box<dyn Fn(&Entry) -> bool + Send>,
    }

    impl Stream for Live {
        fn next_entry(&mut self) -> Result<Entry, Refusal> {
            if let Some(e) = self.past.next() {
                return Ok(e);
            }
            loop {
                let e = self.live.recv().map_err(|_| Refusal::Ended)?;
                if (self.passes)(&e) {
                    return Ok(e);
                }
            }
        }
    }

    /// Gives up at once, as journalctl does on a journal it may not read.
    struct Blind;

    impl Stream for Blind {
        fn next_entry(&mut self) -> Result<Entry, Refusal> {
            Err(Refusal::Unreadable)
        }
        fn settled(&mut self, _within: Duration) -> bool {
            false
        }
    }

    impl Journalist for Journald {
        fn open(&self, args: &[String]) -> Result<Box<dyn Stream>, Refusal> {
            self.opened.lock().expect("opened").push(args.to_vec());
            if self
                .refusing
                .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
                .is_ok()
            {
                return Ok(Box::new(Blind));
            }
            let passes = selects(args, self.boot, self.now_s);
            let mut chosen: Vec<Entry> = self.past.iter().filter(|e| passes(e)).cloned().collect();
            if let Some(c) = args.iter().find_map(|a| a.strip_prefix("--after-cursor=")) {
                let at = chosen
                    .iter()
                    .position(|e| e.cursor.0 == c)
                    .map_or(0, |i| i + 1);
                chosen.drain(..at);
            }
            let follows = args.iter().any(|a| a == "--follow");
            let lines = args.iter().find_map(|a| a.strip_prefix("--lines="));
            let keep = match lines {
                Some("all") => chosen.len(),
                Some(n) => n.parse().expect("lines"),
                None if follows => 10,
                None => chosen.len(),
            };
            chosen.drain(..chosen.len().saturating_sub(keep));
            if !follows {
                let cut = self.cutting.swap(0, Ordering::AcqRel);
                if cut > 0 {
                    chosen.truncate(cut);
                }
                return Ok(Box::new(Scripted {
                    lines: chosen.into_iter(),
                    failed: cut > 0,
                }));
            }
            let live = self
                .live
                .lock()
                .expect("live")
                .pop_front()
                .ok_or(Refusal::Unstartable)?;
            Ok(Box::new(Live {
                past: chosen.into_iter(),
                live,
                passes: Box::new(passes),
            }))
        }
    }

    fn killed() -> Match {
        Match {
            max_priority: Some(ronoctua_journal::ERR),
            message_contains: Some("Killed".to_owned()),
            ..Match::default()
        }
    }

    #[test]
    fn a_watcher_hears_the_journal_lines_it_matches_and_the_cursor_is_kept() {
        let dir = scratch("journal");
        let core = a_core(vec![
            scribe("scribe", vec![Source::Journal(killed())]).boxed(),
        ]);
        let (journal, live) = Journald::with(vec![entry(1, "Killed before", 3)], 1);
        let watching = Watch::new(Arc::clone(&core))
            .reading(Some(journal.clone()))
            .keeping_cursor_in(Some(dir.clone()))
            .backing_off(Duration::from_millis(1))
            .start()
            .expect("start")
            .expect("somebody watches");
        live[0]
            .send(entry(2, "Out of memory: Killed process 42", 3))
            .expect("send");
        live[0]
            .send(entry(3, "a line nobody asked for", 3))
            .expect("send");
        live[0]
            .send(entry(4, "Killed, but only a warning", 4))
            .expect("send");
        live[0]
            .send(entry(5, "Out of memory: Killed process 43", 2))
            .expect("send");
        let mind = until(&core, "two lines heard", |m| noted(m, "scribe").len() >= 2);
        // THE MATCH, in process: the text AND the priority — and nothing of
        // the past, which nobody asked for.
        assert_eq!(
            noted(&mind, "scribe"),
            [
                "Out of memory: Killed process 42",
                "Out of memory: Killed process 43"
            ]
        );
        // A NOTE IS NOT A RUN: the schedule does not move.
        assert_eq!(core.last_run("scribe"), None);
        drop(watching);
        // THE FOLLOWER: no past on a first start with nothing asked, the
        // loosest priority asked for, and from now on.
        assert_eq!(
            journal.opened()[0],
            [
                "--follow",
                "--output=json",
                "--no-pager",
                "--priority=0..3",
                "--lines=0"
            ]
        );
        // AND WHERE IT WAS LEFT is on disk on the way out — the last entry
        // READ, matched or not (the warning is not read at `0..3`).
        assert_eq!(
            Cursor::read_from(&dir.join(CURSOR_FILE)),
            Some(Cursor("s=t;i=5".to_owned()))
        );

        // THE NEXT START goes on from there, and reads no past.
        let (journal, _live) = Journald::with(vec![entry(1, "Killed from the past", 3)], 1);
        let again = a_core(vec![
            scribe(
                "scribe",
                vec![Source::Journal(Match {
                    backfill: ronoctua_journal::Backfill::ThisBoot { most: 5 },
                    ..killed()
                })],
            )
            .boxed(),
        ]);
        let watching = Watch::new(Arc::clone(&again))
            .reading(Some(journal.clone()))
            .keeping_cursor_in(Some(dir.clone()))
            .start()
            .expect("start");
        let deadline = Instant::now() + Duration::from_secs(10);
        while journal.opened().is_empty() {
            assert!(Instant::now() < deadline, "the follower never opened");
            std::thread::sleep(Duration::from_millis(5));
        }
        let first = journal.opened()[0].clone();
        assert!(
            first.contains(&"--after-cursor=s=t;i=5".to_owned()),
            "{first:?}"
        );
        assert!(
            first.contains(&"--follow".to_owned()),
            "a pass ran with a cursor saved"
        );
        drop(watching);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_past_is_one_pass_and_each_module_takes_its_share() {
        // THE DEFECT THIS CATCHES: the pass cut with `--lines=2` BEFORE the
        // match. The boot's last two lines are noise, so a module asking for
        // its last two kills got none. And a `since` wider than the boot lost
        // the boot before it, because "this boot" was taken for the wider.
        let greedy = Source::Journal(Match {
            backfill: ronoctua_journal::Backfill::ThisBoot { most: 2 },
            ..killed()
        });
        let week = Source::Journal(Match {
            backfill: ronoctua_journal::Backfill::Since {
                seconds: 604_800,
                most: 10,
            },
            ..killed()
        });
        let core = a_core(vec![
            scribe("greedy", vec![greedy]).boxed(),
            scribe("week", vec![week]).boxed(),
            scribe("present", vec![Source::Journal(killed())]).boxed(),
        ]);
        let past = vec![
            entry_of("before", 1, "Killed zero", 3),
            entry(2, "Killed one", 3),
            entry(3, "Killed two", 3),
            entry(4, "not this", 3),
            entry(5, "Killed three", 3),
            entry(6, "not this", 3),
            entry(7, "not this either", 3),
        ];
        let (journal, live) = Journald::with(past, 1);
        let watching = Watch::new(Arc::clone(&core))
            .reading(Some(journal.clone()))
            .keeping_cursor_in(None)
            .start()
            .expect("start");
        live[0].send(entry(8, "Killed now", 3)).expect("send");
        let mind = until(&core, "the present heard by all three", |m| {
            ["greedy", "week", "present"]
                .iter()
                .all(|id| noted(m, id).last().is_some_and(|t| t == "Killed now"))
        });
        assert_eq!(
            noted(&mind, "greedy"),
            ["Killed two", "Killed three", "Killed now"]
        );
        assert_eq!(
            noted(&mind, "week"),
            [
                "Killed zero",
                "Killed one",
                "Killed two",
                "Killed three",
                "Killed now"
            ]
        );
        assert_eq!(noted(&mind, "present"), ["Killed now"]);
        drop(watching);
        let opened = journal.opened();
        // THE BOOT'S START FIRST, since both kinds were asked; THE PASS from
        // whichever began first, never cut, and ending (no `--follow`); and
        // the follower after the last line the pass READ.
        assert_eq!(opened[0], ronoctua_journal::probe_arguments());
        assert_eq!(opened[1][2..], ["--priority=0..3", "--since=@0"]);
        assert!(
            opened[2].contains(&"--after-cursor=s=t;i=7".to_owned()),
            "{:?}",
            opened[2]
        );
    }

    #[test]
    fn an_ear_that_asks_no_past_hears_none_when_another_asks_for_one() {
        // THE DEFECT THIS CATCHES: the pass read at the backfilling ear's
        // `err`, and the follower went on from the boot's last error at the
        // other ear's `notice` — so every notice written after that error was
        // handed to the ear that asked for no past, as if it were new.
        let past_ear = Source::Journal(Match {
            backfill: ronoctua_journal::Backfill::ThisBoot { most: 5 },
            ..killed()
        });
        let quiet_ear = Source::Journal(Match {
            max_priority: Some(5),
            message_contains: Some("Quiet".to_owned()),
            ..Match::default()
        });
        let core = a_core(vec![
            scribe("past", vec![past_ear]).boxed(),
            scribe("present", vec![quiet_ear]).boxed(),
        ]);
        let past = vec![
            entry(1, "Killed before", 3),
            entry(2, "Quiet before", 5),
            entry(3, "Quiet also before", 5),
        ];
        let (journal, live) = Journald::with(past, 1);
        let watching = Watch::new(Arc::clone(&core))
            .reading(Some(journal.clone()))
            .keeping_cursor_in(None)
            .start()
            .expect("start");
        live[0].send(entry(4, "Quiet now", 5)).expect("send");
        let mind = until(&core, "the present heard", |m| {
            noted(m, "present").last().is_some_and(|t| t == "Quiet now")
        });
        assert_eq!(noted(&mind, "past"), ["Killed before"]);
        assert_eq!(
            noted(&mind, "present"),
            ["Quiet now"],
            "the past was handed as news"
        );
        drop(watching);
        let opened = journal.opened();
        assert_eq!(opened[0][2..], ["--priority=0..5", "--boot"]);
        assert!(
            opened[1].contains(&"--after-cursor=s=t;i=3".to_owned()),
            "{:?}",
            opened[1]
        );
    }

    #[test]
    fn no_journal_is_said_and_a_module_switched_off_hears_nothing() {
        let core = a_core(vec![
            scribe("scribe", vec![Source::Journal(killed())]).boxed(),
            scribe("sleeper", vec![Source::Journal(killed())]).boxed(),
        ]);
        core.set("sleeper", &Knob::Enabled(false)).expect("off");
        let watching = Watch::new(Arc::clone(&core))
            .reading(None)
            .keeping_cursor_in(None)
            .start()
            .expect("start");
        let mind = until(&core, "the loss said", |m| !noted(m, "scribe").is_empty());
        assert_eq!(noted(&mind, "scribe"), ["lost no-program"]);
        drop(watching);
        assert!(
            noted(&core.shared.snapshot().0, "sleeper").is_empty(),
            "a module switched off heard"
        );
    }

    #[test]
    fn a_module_the_system_switched_off_is_never_listened_for() {
        // `RONOCTUA_BRAIN_OFF`: not even its sources are asked, so a journal
        // only it wanted is never followed and no thread is raised for it.
        let core = Arc::new(Core::new(
            Arc::new(Shared::of(Prefs::default())),
            Registry::of(vec![
                scribe("scribe", vec![Source::Journal(killed())]).boxed(),
            ])
            .expect("the registry")
            .switched_off(&["scribe"]),
            Where::default(),
            Box::new(Hand::at(1000)),
        ));
        assert!(listeners(&core).is_empty(), "it is said to listen");
        let (journal, _live) = Journald::with(Vec::new(), 1);
        let watching = Watch::new(Arc::clone(&core))
            .reading(Some(journal.clone()))
            .keeping_cursor_in(None)
            .start()
            .expect("start");
        assert!(watching.is_none(), "a thread was raised for it");
        assert!(journal.opened().is_empty(), "its journal was followed");
    }

    #[test]
    fn a_journal_that_comes_back_is_said_back_and_a_restart_that_took_is_not_a_loss() {
        let core = a_core(vec![
            scribe("scribe", vec![Source::Journal(killed())]).boxed(),
        ]);
        let (journal, live) = Journald::with(Vec::new(), 2);
        // THE FIRST TRY GIVES UP AT ONCE: down, and said so.
        journal.refusing.store(1, Ordering::Release);
        let watching = Watch::new(Arc::clone(&core))
            .reading(Some(journal.clone()))
            .keeping_cursor_in(None)
            .backing_off(Duration::from_millis(1))
            .start()
            .expect("start");
        // THE SECOND FOLLOWS, before a single line: back, and said so.
        until(&core, "back", |m| noted(m, "scribe").len() >= 2);
        live[0].send(entry(1, "Killed one", 3)).expect("send");
        until(&core, "the line", |m| noted(m, "scribe").len() >= 3);
        // A FOLLOWER THAT TOOK AND ENDED is started again from its cursor —
        // nothing was missed, so nothing is lost.
        let mut live = live.into_iter();
        drop(live.next());
        let _second = live.next();
        let deadline = Instant::now() + Duration::from_secs(10);
        while journal.opened().len() < 3 {
            assert!(Instant::now() < deadline, "the follower was not restarted");
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(100));
        drop(watching);
        assert_eq!(
            noted(&core.shared.snapshot().0, "scribe"),
            ["lost unreadable", "back", "Killed one"]
        );
        assert!(
            journal.opened()[2].contains(&"--after-cursor=s=t;i=1".to_owned()),
            "the restart did not go on from where it was"
        );
    }

    #[test]
    fn a_path_is_heard_when_something_changes_at_it_even_before_it_exists() {
        let dir = scratch("path");
        let later = dir.join("later");
        let core = a_core(vec![
            scribe(
                "scribe",
                vec![Source::Path(dir.clone()), Source::Path(later.clone())],
            )
            .boxed(),
        ]);
        let watching = Watch::new(Arc::clone(&core))
            .reading(None)
            .start()
            .expect("start");
        // THE KERNEL'S WATCHES ARE RAISED ON THE NOTICE THREADS, a moment after
        // the start: the file is written again until the change is heard, so
        // a write that landed before the watch cannot make this flaky — what
        // is proved is that a change IS heard, not how soon.
        let heard_both = |m: &Mind| {
            let n = noted(m, "scribe");
            n.iter().any(|t| t == &format!("path {}", later.display()))
                && n.iter().any(|t| t == &format!("path {}", dir.display()))
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        let mind = loop {
            std::fs::write(&later, "born").expect("write");
            let (mind, _) = core
                .shared
                .wait_past(core.shared.version(), Duration::from_millis(300));
            if heard_both(&mind) {
                break mind;
            }
            assert!(Instant::now() < deadline, "never: the birth heard");
        };
        assert!(!noted(&mind, "scribe").iter().any(|t| t.starts_with("lost")));
        drop(watching);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_born_after_the_start_is_heard_inside_and_again_when_made_again() {
        // THE DEFECT THIS CATCHES: a directory that did not exist at the
        // start was watched from its parent for its NAME only — its birth was
        // heard, and nothing that happened inside it ever was.
        let dir = scratch("born");
        let crun = dir.join("crun");
        let core = a_core(vec![
            scribe("scribe", vec![Source::Path(crun.clone())]).boxed(),
        ]);
        let watching = Watch::new(Arc::clone(&core))
            .reading(None)
            .start()
            .expect("start");
        let heard = || noted(&core.shared.snapshot().0, "scribe").len();
        // Waits until something `write` did is heard past `before`.
        let heard_after = |before: usize, write: &dyn Fn()| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                write();
                let _ = core
                    .shared
                    .wait_past(core.shared.version(), Duration::from_millis(300));
                if heard() > before {
                    return;
                }
                assert!(Instant::now() < deadline, "never heard");
            }
        };
        // QUIET: no tail of a burst still on its way — a notice's tail comes
        // up to a second late, and a publication half a second after that —
        // so what is heard next is what was done next.
        let quiet = || {
            let mut was = heard();
            loop {
                std::thread::sleep(Duration::from_millis(1600));
                let now = heard();
                if now == was {
                    return now;
                }
                was = now;
            }
        };
        // BORN: heard from the parent. Made again until heard, so a birth
        // before the parent's watch was raised cannot make this flaky.
        heard_after(0, &|| {
            let _ = std::fs::remove_dir(&crun);
            std::fs::create_dir(&crun).expect("mkdir");
        });
        // AND INSIDE: a container's directory appearing in it — which the
        // parent's watch, narrowed to the name `crun`, never hears.
        let settled = quiet();
        heard_after(settled, &|| {
            std::fs::write(crun.join("container"), "state").expect("write inside");
        });
        // MADE AGAIN: gone, back, and still heard inside — a new directory
        // by the same name is another inode, and the old watch died with the
        // old one.
        let settled = quiet();
        heard_after(settled, &|| {
            let _ = std::fs::remove_dir_all(&crun);
            std::fs::create_dir(&crun).expect("mkdir again");
        });
        let settled = quiet();
        heard_after(settled, &|| {
            std::fs::write(crun.join("other"), "state").expect("write inside again");
        });
        assert!(
            !noted(&core.shared.snapshot().0, "scribe")
                .iter()
                .any(|t| t.starts_with("lost")),
            "a path that is watched was said lost"
        );
        drop(watching);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tree_hears_inside_folders_made_after_the_start() {
        // THE DEFECT THIS CATCHES: a module that listed the folders of a
        // tree when it started never heard a file written in a folder made
        // later — the belongings' plain part, whose `software/` folder the
        // first declaration makes, and every later one writes inside.
        let dir = scratch("tree");
        let root = dir.join("person");
        std::fs::create_dir_all(&root).expect("mkdir");
        let core = a_core(vec![
            scribe("scribe", vec![Source::Tree(root.clone())]).boxed(),
        ]);
        let watching = Watch::new(Arc::clone(&core))
            .reading(None)
            .start()
            .expect("start");
        let heard = || noted(&core.shared.snapshot().0, "scribe").len();
        let heard_after = |before: usize, write: &dyn Fn()| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                write();
                let _ = core
                    .shared
                    .wait_past(core.shared.version(), Duration::from_millis(300));
                if heard() > before {
                    return;
                }
                assert!(Instant::now() < deadline, "never heard");
            }
        };
        let quiet = || {
            let mut was = heard();
            loop {
                std::thread::sleep(Duration::from_millis(1600));
                let now = heard();
                if now == was {
                    return now;
                }
                was = now;
            }
        };
        // TWO LEVELS MADE after the start, at once — heard at the root. Made
        // again until heard: a birth before the watch was raised is missed.
        let deep = root.join("software/icons");
        heard_after(0, &|| {
            let _ = std::fs::remove_dir_all(root.join("software"));
            std::fs::create_dir_all(&deep).expect("mkdir -p");
        });
        // AND A FILE WRITTEN IN EACH, later: neither folder was there when
        // the watch started.
        for (at, name) in [(root.join("software"), "declared.json"), (deep, "a.png")] {
            let settled = quiet();
            heard_after(settled, &|| {
                std::fs::write(at.join(name), "declared").expect("write inside");
            });
        }
        let said = noted(&core.shared.snapshot().0, "scribe");
        assert!(
            said.iter()
                .all(|t| t == &format!("path {}", root.display())),
            "a tree is heard as its own path: {said:?}"
        );
        drop(watching);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_childs_lines_are_heard_and_its_end_is_said() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = scratch("lines");
        let script = dir.join("speaks");
        // `printf` IS THE SHELL'S OWN: the script needs nothing on the PATH.
        std::fs::write(&script, "#!/bin/sh\nprintf 'one\\ntwo\\n'\nexit 3\n").expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let core = a_core(vec![
            scribe(
                "scribe",
                vec![
                    Source::Lines {
                        argv: vec![script.display().to_string()],
                    },
                    // BY NAME IS REFUSED, with the reason: a unit's PATH is
                    // minimal.
                    Source::Lines {
                        argv: vec!["sh".to_owned()],
                    },
                ],
            )
            .boxed(),
        ]);
        let watching = Watch::new(Arc::clone(&core))
            .reading(None)
            .backing_off(Duration::from_secs(60))
            .start()
            .expect("start");
        let mind = until(&core, "both lines and the end", |m| {
            let n = noted(m, "scribe");
            n.iter()
                .any(|t| t.starts_with("lost") && t.contains("exit status: 3"))
                && n.iter().any(|t| t.contains("absolute path"))
        });
        let n = noted(&mind, "scribe");
        let lines: Vec<&String> = n.iter().filter(|t| t.starts_with("line")).collect();
        assert_eq!(lines, ["line one", "line two"]);
        drop(watching);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wake_comes_when_asked_and_catch_up_comes_first() {
        let woken = Arc::new(AtomicUsize::new(0));
        let asked = Arc::clone(&woken);
        let core = a_core(vec![
            Fake::watch("sleeper")
                .listens(Vec::new(), |heard, ctx| {
                    if matches!(heard, Heard::Wake) {
                        ctx.record(Noted::of("sleeper-woke"));
                    }
                })
                .starts(|_, ctx| ctx.record(Noted::of("sleeper-started")))
                // ONCE: the moment is now until it has been woken.
                .wakes(move |now| (asked.load(Ordering::Relaxed) == 0).then_some(now))
                .boxed(),
        ]);
        // `listens` with no sources is still a watcher: a clock is a source.
        let watching = Watch::new(Arc::clone(&core))
            .reading(None)
            .start()
            .expect("start")
            .expect("a module that only wants a clock is a watcher");
        let mind = until(&core, "woken", |m| {
            m.journal.iter().any(|r| r.says == "sleeper-woke")
        });
        woken.store(1, Ordering::Relaxed);
        let says: Vec<&str> = mind.journal.iter().map(|r| r.says.as_str()).collect();
        assert_eq!(says[0], "sleeper-started", "catching up did not come first");
        drop(watching);
    }

    static ARM: Action = Action {
        id: "arm",
        command: "ringer-arm",
        label: "cmd-ringer-arm",
        row: false,
        asks: None,
        opens: None,
        optional: false,
        while_off: false,
    };

    #[test]
    fn a_wake_armed_from_outside_is_delivered_and_a_watcher_switched_on_later_is_started() {
        // THE DEFECT THIS CATCHES: the thread computed its wait only after a
        // message, and nothing outside it sent one. A wake armed by an action
        // once the thread had parked never rang, and a watcher switched on
        // later was never started nor told its journal was unreadable.
        let armed = Arc::new(Mutex::new(None::<Moment>));
        let (wants, arms, clears) = (Arc::clone(&armed), Arc::clone(&armed), Arc::clone(&armed));
        let ringer = Fake::new(Manifest {
            schedule: None,
            actions: std::slice::from_ref(&ARM),
            ..manifest("ringer")
        })
        .listens(Vec::new(), move |heard, ctx| {
            if matches!(heard, Heard::Wake) {
                *clears.lock().expect("armed") = None;
                ctx.record(Noted::of("ringer-rang"));
            }
        })
        .wakes(move |_| *wants.lock().expect("armed"))
        .acts(move |_, ctx| {
            *arms.lock().expect("armed") = Some(ctx.now());
            ctx.changed();
            Acted::Done
        });
        let late = scribe("late", vec![Source::Journal(killed())])
            .starts(|_, ctx| ctx.record(Noted::of("late-started")));
        let core = a_core(vec![ringer.boxed(), late.boxed()]);
        core.set("late", &Knob::Enabled(false)).expect("off");
        let watching = Watch::new(Arc::clone(&core))
            .reading(None)
            .start()
            .expect("start")
            .expect("somebody watches");
        // PARKED: nothing wanted, nothing pending — no clock at all.
        std::thread::sleep(Duration::from_millis(200));
        core.act("ringer", "arm", None, None).expect("armed");
        core.refresh();
        let mind = until(&core, "the wake armed from outside", |m| {
            m.journal.iter().any(|r| r.says == "ringer-rang")
        });
        assert!(
            noted(&mind, "late").is_empty(),
            "a module switched off was started"
        );
        // SWITCHED ON: started, then told what its journal is doing.
        core.set("late", &Knob::Enabled(true)).expect("on");
        let mind = until(&core, "the late one started and told", |m| {
            noted(m, "late").len() >= 2
        });
        assert_eq!(noted(&mind, "late"), ["late-started", "lost no-program"]);
        drop(watching);
    }

    #[test]
    fn a_burst_is_published_a_few_times_and_not_once_a_line() {
        // THE DEFECT THIS CATCHES: a whole mind to every screen for every
        // line a watcher noted — five hundred lines were five hundred minds.
        //
        // TWO BURSTS, because two things keep it down. Lines that arrive
        // together are handed out together and published once; lines that
        // arrive one by one, a few milliseconds apart, are each alone on the
        // channel — and only the half-second window keeps them from being a
        // mind apiece.
        let heard = Arc::new(AtomicUsize::new(0));
        let counts = Arc::clone(&heard);
        let core = a_core(vec![
            Fake::watch("busy")
                .listens(vec![Source::Journal(killed())], move |h, ctx| {
                    // EACH NOTE ITS OWN: the mind carries the log's tail, and
                    // a tail of identical notes is a mind that did not change.
                    if let Heard::Journal { entry, .. } = h {
                        counts.fetch_add(1, Ordering::Relaxed);
                        ctx.record(
                            Noted::of("busy-noted").with("what", Arg::Text(entry.message.clone())),
                        );
                        ctx.changed();
                    }
                })
                .boxed(),
        ]);
        let (journal, live) = Journald::with(Vec::new(), 1);
        let watching = Watch::new(Arc::clone(&core))
            .reading(Some(journal))
            .keeping_cursor_in(None)
            .start()
            .expect("start");
        // Counts the publications of one burst, `send` sending its lines.
        let publications = |send: &dyn Fn(u64), lines: usize| {
            // Whatever came before is published before this burst is counted.
            std::thread::sleep(PUBLISH_EVERY * 3);
            let (before, already) = (core.shared.version(), heard.load(Ordering::Relaxed));
            for n in 0..lines {
                send(n as u64);
            }
            let deadline = Instant::now() + Duration::from_secs(10);
            while heard.load(Ordering::Relaxed) < already + lines {
                assert!(Instant::now() < deadline, "the burst was not heard");
                std::thread::sleep(Duration::from_millis(5));
            }
            // AND ITS TAIL IS PUBLISHED, at the end of the window.
            std::thread::sleep(PUBLISH_EVERY * 3);
            assert!(!core.stale(), "the burst's tail was never published");
            core.shared.version() - before
        };
        let at_once = publications(
            &|n| {
                live[0]
                    .send(entry(n, &format!("Killed {n}"), 3))
                    .expect("send");
            },
            500,
        );
        assert!(
            (1..=4).contains(&at_once),
            "{at_once} publications for 500 lines at once"
        );
        // A LINE EVERY FEW MILLISECONDS for over a second: each is alone.
        let paced = publications(
            &|n| {
                live[0]
                    .send(entry(1000 + n, &format!("Killed {n}"), 3))
                    .expect("send");
                std::thread::sleep(Duration::from_millis(10));
            },
            120,
        );
        assert!(
            (1..=6).contains(&paced),
            "{paced} publications for 120 lines over a second"
        );
        drop(watching);
    }

    #[test]
    fn what_waits_on_the_channel_is_handed_out_before_anything_is_looked_at_again() {
        // THE DRAIN: five hundred lines waiting are handed out in one turn,
        // and every module's wake, the cursor and the publication are looked
        // at once for all of them — not five hundred times.
        let (heard, looked) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let (counts, looks) = (Arc::clone(&heard), Arc::clone(&looked));
        let core = a_core(vec![
            Fake::watch("busy")
                .listens(vec![Source::Journal(killed())], move |_, _| {
                    counts.fetch_add(1, Ordering::Relaxed);
                })
                .wakes(move |_| {
                    looks.fetch_add(1, Ordering::Relaxed);
                    None
                })
                .boxed(),
        ]);
        let (tx, rx) = mpsc::channel();
        let ears = vec![Ear {
            module: "busy",
            source: 0,
            kind: Source::Journal(killed()),
            down: None,
            told: None,
        }];
        let dispatcher = Dispatcher::new(
            Arc::clone(&core),
            ears,
            Keeper::at(None, CURSOR_FILE, CURSOR_EVERY),
            tx.clone(),
            Arc::new(AtomicBool::new(false)),
        );
        for n in 0..500 {
            tx.send(Msg::Entry(entry(n, "Killed", 3))).expect("send");
        }
        tx.send(Msg::Stop).expect("stop");
        dispatcher.run(&rx);
        assert_eq!(heard.load(Ordering::Relaxed), 500);
        let looked = looked.load(Ordering::Relaxed);
        assert!(
            looked <= 4,
            "the wakes were looked at {looked} times for one turn"
        );
    }

    #[test]
    fn no_clock_is_armed_when_nobody_wants_one() {
        let wants = Arc::new(Mutex::new(None::<Moment>));
        let asked = Arc::clone(&wants);
        let core = a_core(vec![
            Fake::watch("sleeper")
                .listens(Vec::new(), |_, _| {})
                .wakes(move |_| *asked.lock().expect("wants"))
                .boxed(),
        ]);
        let (tx, _rx) = mpsc::channel();
        let mut dispatcher = Dispatcher::new(
            Arc::clone(&core),
            Vec::new(),
            Keeper::at(None, CURSOR_FILE, CURSOR_EVERY),
            tx,
            Arc::new(AtomicBool::new(false)),
        );
        dispatcher.reconcile();
        let now = Instant::now;
        // EVENT, NOT CLOCK: nothing wanted, nothing pending — block for ever.
        assert_eq!(dispatcher.patience(now()), None);
        *wants.lock().expect("wants") = Some(1010);
        assert_eq!(dispatcher.patience(now()), Some(Duration::from_secs(10)));
        // A WAKE FAR AWAY is looked at again every so often — the wait's
        // clock stops while the machine sleeps, the wake's does not.
        *wants.lock().expect("wants") = Some(1000 + 3600);
        assert_eq!(dispatcher.patience(now()), Some(WAKE_CHECK));
        // A MOMENT ALREADY PAST, once woken, waits a second — never a spin.
        *wants.lock().expect("wants") = Some(900);
        assert_eq!(dispatcher.patience(now()), Some(Duration::ZERO));
        dispatcher.wake();
        assert_eq!(dispatcher.patience(now()), Some(Duration::from_secs(1)));
        // AND A CURSOR WAITING TO BE WRITTEN arms one too.
        *wants.lock().expect("wants") = None;
        dispatcher.keeper.note(Cursor("x".to_owned()));
        assert!(dispatcher.patience(now()).is_some());
        // Nobody watches at all: no thread.
        let nobody = a_core(vec![Fake::watch("mute").boxed()]);
        assert!(
            Watch::new(nobody)
                .reading(None)
                .start()
                .expect("start")
                .is_none()
        );
    }

    static READ: Action = Action {
        id: "read",
        command: "reader-read",
        label: "cmd-reader-read",
        row: false,
        asks: None,
        opens: None,
        optional: false,
        while_off: false,
    };

    /// A scribe that keeps its own place, as a module with a pass does: `read`
    /// asks for one from there, and each line it brings is noted, then how it
    /// ended, and the place moves only to a line it really read.
    fn reader(sources: Vec<Source>, from: Option<Cursor>, most: usize) -> Fake {
        reader_to(sources, from, most, None)
    }

    /// [`reader`], whose passes stop before `until`.
    fn reader_to(
        sources: Vec<Source>,
        from: Option<Cursor>,
        most: usize,
        until: Option<Cursor>,
    ) -> Fake {
        let place = Arc::new(Mutex::new(from));
        let moved = Arc::clone(&place);
        scribe("reader", sources)
            .declaring(Manifest {
                schedule: None,
                actions: std::slice::from_ref(&READ),
                ..manifest("reader")
            })
            .acts(|_, _| Acted::Pass)
            .catches_up(
                move |_, _| {
                    Some(Pass {
                        after: place.lock().expect("place").clone(),
                        most,
                        until: until.clone(),
                    })
                },
                move |passed, ctx| {
                    if let Some(last) = passed.last {
                        *moved.lock().expect("place") = Some(last.clone());
                    }
                    let note = |text: String| {
                        ctx.record(Noted::of("scribe-noted").with("what", Arg::Text(text)));
                    };
                    for (source, entries) in passed.shares {
                        for e in entries {
                            note(format!("read {source} {}", e.message));
                        }
                    }
                    note(format!(
                        "ended lost={} last={} why={}{}",
                        passed.lost,
                        passed.last.map_or("-", |c| c.0.as_str()),
                        passed.why.map_or("-", Refusal::id),
                        if passed.ahead { " ahead" } else { "" }
                    ));
                },
            )
    }

    /// A dispatcher nothing feeds, for a test that hands it its messages in
    /// the very order it is about, with the channel a pass answers on.
    fn by_hand(
        core: &Arc<Core>,
        module: &'static str,
        matches: Vec<Match>,
        journalist: Option<Arc<dyn Journalist>>,
    ) -> (Dispatcher, Receiver<Msg>) {
        let (tx, rx) = mpsc::channel();
        let ears = matches
            .into_iter()
            .enumerate()
            .map(|(source, m)| Ear {
                module,
                source,
                kind: Source::Journal(m),
                down: None,
                told: None,
            })
            .collect();
        let mut dispatcher = Dispatcher::new(
            Arc::clone(core),
            ears,
            Keeper::at(None, CURSOR_FILE, CURSOR_EVERY),
            tx,
            Arc::new(AtomicBool::new(false)),
        );
        dispatcher.journalist = journalist;
        (dispatcher, rx)
    }

    /// The message a pass ends in.
    fn the_pass(rx: &Receiver<Msg>) -> Msg {
        loop {
            match rx.recv_timeout(Duration::from_secs(10)) {
                Ok(msg @ Msg::Passed { .. }) => return msg,
                Ok(_) => {}
                Err(e) => panic!("the pass never ended: {e}"),
            }
        }
    }

    /// What it noted so far, with no thread to publish it.
    fn noted_now(core: &Core, module: &str) -> Vec<String> {
        core.refresh();
        noted(&core.shared.snapshot().0, module)
    }

    /// A journal that cannot even be started.
    struct NoStart;

    impl Journalist for NoStart {
        fn open(&self, _args: &[String]) -> Result<Box<dyn Stream>, Refusal> {
            Err(Refusal::Unstartable)
        }
    }

    #[test]
    fn a_pass_reads_after_the_module_and_leaves_the_shared_cursor_alone() {
        // THE DEFECT THIS CATCHES: the pass written down as the follower's
        // position. The shared cursor is everybody's, and a pass over what
        // one module missed would send it back, for all of them.
        let dir = scratch("pass");
        Cursor("s=t;i=4".to_owned())
            .write_to(&dir, CURSOR_FILE)
            .expect("the cursor");
        let quiet = Match {
            max_priority: Some(ronoctua_journal::INFO),
            message_contains: Some("Quiet".to_owned()),
            ..Match::default()
        };
        let core = a_core(vec![
            reader(
                vec![Source::Journal(killed())],
                Some(Cursor("s=t;i=1".to_owned())),
                10,
            )
            .boxed(),
            scribe("other", vec![Source::Journal(quiet)]).boxed(),
        ]);
        core.set("reader", &Knob::Enabled(false)).expect("off");
        let past = vec![
            entry(1, "Killed first", 3),
            entry(2, "Killed second", 3),
            entry(3, "Killed third", 3),
            entry(4, "a quieter line", 6),
        ];
        let (journal, _live) = Journald::with(past, 1);
        let watching = Watch::new(Arc::clone(&core))
            .reading(Some(journal.clone()))
            .keeping_cursor_in(Some(dir.clone()))
            .start()
            .expect("start");
        core.act("reader", "read", None, None).expect("asked");
        let mind = until(&core, "the pass handed over", |m| {
            noted(m, "reader")
                .last()
                .is_some_and(|t| t.starts_with("ended"))
        });
        assert_eq!(
            noted(&mind, "reader"),
            [
                "read 0 Killed second",
                "read 0 Killed third",
                "ended lost=0 last=s=t;i=3 why=-"
            ]
        );
        assert!(noted(&mind, "other").is_empty(), "the pass reached another");
        drop(watching);
        // AT ITS OWN PRIORITY, FROM ITS OWN PLACE, and it ends. Only passes
        // are counted: the follower opens on a thread of its own, and nothing
        // here waited for it.
        let opened = journal.opened();
        let passes: Vec<_> = opened
            .iter()
            .filter(|args| !args.iter().any(|a| a == "--follow"))
            .collect();
        assert_eq!(passes.len(), 1, "{opened:?}");
        assert_eq!(
            passes[0][2..],
            ["--priority=0..3", "--after-cursor=s=t;i=1"]
        );
        // AND THE CURSOR IS WHERE THE FOLLOWER LEFT IT.
        assert_eq!(
            Cursor::read_from(&dir.join(CURSOR_FILE)),
            Some(Cursor("s=t;i=4".to_owned()))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn each_source_keeps_its_own_ring_and_what_they_lost_is_said() {
        // A WARNING NEVER PUSHES AN ERROR OUT: two bands, two rings, and each
        // line in one of them.
        let errors = Match {
            max_priority: Some(ronoctua_journal::ERR),
            ..Match::default()
        };
        let warnings = Match {
            max_priority: Some(ronoctua_journal::WARNING),
            min_priority: Some(ronoctua_journal::WARNING),
            ..Match::default()
        };
        let core = a_core(vec![
            reader(
                vec![
                    Source::Journal(errors.clone()),
                    Source::Journal(warnings.clone()),
                ],
                None,
                2,
            )
            .boxed(),
        ]);
        core.set("reader", &Knob::Enabled(false)).expect("off");
        let past = vec![
            entry(1, "an old error", 3),
            entry(2, "an error", 3),
            entry(3, "a warning", 4),
            entry(4, "a newer warning", 4),
            entry(5, "the newest warning", 4),
            entry(6, "a line nobody here wants", 6),
        ];
        let (journal, _live) = Journald::with(past, 0);
        let (mut d, rx) = by_hand(
            &core,
            "reader",
            vec![errors, warnings],
            Some(journal.clone()),
        );
        d.reconcile();
        core.act("reader", "read", None, None).expect("asked");
        d.reconcile();
        d.hear(the_pass(&rx));
        assert_eq!(
            noted_now(&core, "reader"),
            [
                "read 0 an old error",
                "read 0 an error",
                "read 1 a newer warning",
                "read 1 the newest warning",
                "ended lost=1 last=s=t;i=5 why=-"
            ]
        );
        // AT THE LOOSEST OF ITS OWN: the follower's priority is nobody's here.
        assert_eq!(journal.opened()[0][2..], ["--priority=0..4", "--boot"]);
    }

    /// FAILS IF: a pass the brain stopped in the middle of is lost — the
    /// follower's cursor covers only what the follower read, and the stretch
    /// the pass was reading nothing else reads — or is read again past the
    /// line the follower took over at, which the module heard already.
    #[test]
    fn a_pass_owed_as_the_brain_starts_is_read_up_to_where_the_follower_took_over() {
        let past = vec![
            entry(1, "Killed first", 3),
            entry(2, "Killed second", 3),
            entry(3, "Killed heard before the stop", 3),
            entry(4, "Killed after it", 3),
        ];
        let took_over = Some(Cursor("s=t;i=3".to_owned()));
        for owed in [true, false] {
            let core = a_core(vec![
                reader_to(vec![Source::Journal(killed())], None, 10, took_over.clone())
                    .owes(move || owed)
                    .boxed(),
            ]);
            let (journal, _) = Journald::with(past.clone(), 0);
            let (mut d, rx) = by_hand(&core, "reader", vec![killed()], Some(journal.clone()));
            // THE FIRST LOOK: the brain is starting.
            d.reconcile();
            if !owed {
                assert!(journal.opened().is_empty(), "the brain's start made a pass");
                continue;
            }
            d.hear(the_pass(&rx));
            assert_eq!(
                noted_now(&core, "reader"),
                [
                    "read 0 Killed first",
                    "read 0 Killed second",
                    "ended lost=0 last=s=t;i=2 why=-"
                ]
            );
        }
    }

    #[test]
    fn a_module_switched_on_is_read_for_and_a_brain_that_starts_is_not() {
        // A BRAIN THAT STARTS: everybody comes on, nobody is read for — what
        // its modules missed is what the follower reads after its cursor.
        let core = a_core(vec![
            reader(vec![Source::Journal(killed())], None, 10)
                .starts(|_, ctx| ctx.record(Noted::of("reader-started")))
                .stops(|_, ctx| ctx.record(Noted::of("reader-stopped")))
                .boxed(),
        ]);
        let past = vec![entry(1, "Killed while it was off", 3)];
        let (journal, live) = Journald::with(past, 1);
        let watching = Watch::new(Arc::clone(&core))
            .reading(Some(journal.clone()))
            .keeping_cursor_in(None)
            .start()
            .expect("start");
        live[0].send(entry(2, "Killed now", 3)).expect("send");
        let mind = until(&core, "the line", |m| noted(m, "reader").len() >= 2);
        assert_eq!(noted(&mind, "reader"), ["reader-started", "Killed now"]);
        assert_eq!(journal.opened().len(), 1, "the brain's start made a pass");
        // SWITCHED OFF, and seen off: an off and an on inside one look of the
        // thread are nothing at all, and nothing was missed.
        core.set("reader", &Knob::Enabled(false)).expect("off");
        until(&core, "stopped", |m| noted(m, "reader").len() >= 3);
        // AND ON AGAIN BY A PERSON: started, and read for.
        core.set("reader", &Knob::Enabled(true)).expect("on");
        let mind = until(&core, "the pass", |m| {
            noted(m, "reader")
                .last()
                .is_some_and(|t| t.starts_with("ended"))
        });
        assert_eq!(
            noted(&mind, "reader")[2..],
            [
                "reader-stopped",
                "reader-started",
                "read 0 Killed while it was off",
                "ended lost=0 last=s=t;i=1 why=-"
            ]
        );
        drop(watching);
    }

    #[test]
    fn what_a_pass_and_the_follower_both_read_is_heard_once() {
        let line = |n: u64, text: &str| entry(n, text, 3);
        let past = vec![
            line(1, "Killed first"),
            line(2, "Killed second"),
            line(3, "Killed third"),
            line(4, "Killed fourth"),
        ];
        let start = |past: Vec<Entry>| {
            let core = a_core(vec![
                reader(vec![Source::Journal(killed())], None, 10).boxed(),
            ]);
            core.set("reader", &Knob::Enabled(false)).expect("off");
            let (journal, _) = Journald::with(past, 0);
            let (mut d, rx) = by_hand(&core, "reader", vec![killed()], Some(journal));
            d.reconcile();
            core.set("reader", &Knob::Enabled(true)).expect("on");
            d.reconcile();
            (core, d, rx)
        };

        // THE FOLLOWER GOT TO THE THIRD LINE FIRST: heard live, and the pass
        // hands the others. Heard out of order, and once — and the pass, which
        // read the third too, is ahead of the follower: what it brings up to
        // the fourth is skipped, so only the pass can move the place there.
        let (core, mut d, rx) = start(past.clone());
        d.hear(Msg::Entry(line(3, "Killed third")));
        d.hear(the_pass(&rx));
        d.hear(Msg::Entry(line(4, "Killed fourth")));
        assert_eq!(
            noted_now(&core, "reader"),
            [
                "Killed third",
                "read 0 Killed first",
                "read 0 Killed second",
                "read 0 Killed fourth",
                "ended lost=0 last=s=t;i=4 why=- ahead"
            ]
        );

        // THE PASS GOT TO THE FOURTH FIRST: what the follower still brings
        // up to there is not heard, and the first line past it is.
        let (core, mut d, rx) = start(past);
        d.hear(the_pass(&rx));
        d.hear(Msg::Entry(line(3, "Killed third")));
        d.hear(Msg::Entry(line(4, "Killed fourth")));
        d.hear(Msg::Entry(line(5, "Killed fifth")));
        assert_eq!(
            noted_now(&core, "reader"),
            [
                "read 0 Killed first",
                "read 0 Killed second",
                "read 0 Killed third",
                "read 0 Killed fourth",
                "ended lost=0 last=s=t;i=4 why=-",
                "Killed fifth"
            ]
        );
    }

    #[test]
    fn a_line_is_known_by_its_cursor_so_a_clock_set_back_loses_none() {
        // THE DEFECT THIS CATCHES: the pass and the follower told apart by
        // when a line was written. With a clock an hour ahead at boot and set
        // right later, the pass's newest stamp lies an hour past the present:
        // every live line would be dropped until then, unheard, and a live
        // line would take from the pass all it stamps after itself.
        let ahead = entry(3600, "Killed ahead", 3);
        let set_right = entry(10, "Killed set right", 3);
        let core = a_core(vec![
            reader(vec![Source::Journal(killed())], None, 10).boxed(),
        ]);
        core.set("reader", &Knob::Enabled(false)).expect("off");
        let (journal, _) = Journald::with(vec![ahead, set_right.clone()], 0);
        let (mut d, rx) = by_hand(&core, "reader", vec![killed()], Some(journal));
        d.reconcile();
        core.set("reader", &Knob::Enabled(true)).expect("on");
        d.reconcile();
        // WRITTEN WHILE THE PASS READS, stamped before the line it read first
        // — and past where the pass ended, so the pass is not ahead of it.
        d.hear(Msg::Entry(entry(20, "Killed live", 3)));
        d.hear(the_pass(&rx));
        // AND STILL THE ONE LINE BOTH READ is heard once.
        d.hear(Msg::Entry(set_right));
        d.hear(Msg::Entry(entry(30, "Killed after", 3)));
        assert_eq!(
            noted_now(&core, "reader"),
            [
                "Killed live",
                "read 0 Killed ahead",
                "read 0 Killed set right",
                "ended lost=0 last=s=t;i=10 why=-",
                "Killed after"
            ]
        );
    }

    #[test]
    fn a_line_a_pass_read_is_not_heard_again_when_the_module_comes_back_on() {
        let past = vec![entry(1, "Killed first", 3), entry(2, "Killed second", 3)];
        let core = a_core(vec![
            reader(vec![Source::Journal(killed())], None, 10).boxed(),
        ]);
        core.set("reader", &Knob::Enabled(false)).expect("off");
        let (journal, _) = Journald::with(past, 0);
        let (mut d, rx) = by_hand(&core, "reader", vec![killed()], Some(journal.clone()));
        d.reconcile();
        // READ BY HAND while it is off, and the follower still behind.
        core.act("reader", "read", None, None).expect("asked");
        d.reconcile();
        d.hear(the_pass(&rx));
        let read = noted_now(&core, "reader");
        // BACK ON: its place is where the pass ended, so the next pass reads
        // nothing, and what the follower brings late was read already.
        core.set("reader", &Knob::Enabled(true)).expect("on");
        d.reconcile();
        d.hear(the_pass(&rx));
        d.hear(Msg::Entry(entry(2, "Killed second", 3)));
        // A CLOCK SET BACK WHILE IT WAS OFF: a new line, stamped before the
        // one the pass ended on, is a new line.
        d.hear(Msg::Entry(Entry {
            cursor: Cursor("s=t;i=3".to_owned()),
            ..entry(1, "Killed under an older clock", 3)
        }));
        assert_eq!(
            noted_now(&core, "reader")[read.len()..],
            ["ended lost=0 last=- why=-", "Killed under an older clock"],
            "a line was heard twice, or a new one not at all"
        );
        assert_eq!(
            journal.opened()[1][2..],
            ["--priority=0..3", "--after-cursor=s=t;i=2"]
        );
    }

    #[test]
    fn a_press_while_a_pass_is_on_its_way_is_that_pass() {
        let core = a_core(vec![
            reader(vec![Source::Journal(killed())], None, 10).boxed(),
        ]);
        core.set("reader", &Knob::Enabled(false)).expect("off");
        let (journal, _) = Journald::with(vec![entry(1, "Killed", 3)], 0);
        let (mut d, rx) = by_hand(&core, "reader", vec![killed()], Some(journal.clone()));
        d.reconcile();
        core.act("reader", "read", None, None).expect("asked");
        d.reconcile();
        core.act("reader", "read", None, None).expect("asked");
        d.reconcile();
        d.hear(the_pass(&rx));
        assert_eq!(journal.opened().len(), 1, "two passes for one");
        assert_eq!(
            noted_now(&core, "reader"),
            ["read 0 Killed", "ended lost=0 last=s=t;i=1 why=-"]
        );
        // AND THE NEXT ONE, once this one ended, is a pass again.
        core.act("reader", "read", None, None).expect("asked");
        d.reconcile();
        d.hear(the_pass(&rx));
        assert_eq!(journal.opened().len(), 2);
    }

    #[test]
    fn a_cursor_refused_falls_back_to_this_boot_and_an_empty_pass_is_not_a_failed_one() {
        let past = vec![
            entry_of("before", 1, "Killed before", 3),
            entry(2, "Killed this boot", 3),
        ];
        let from = || Some(Cursor("not a cursor".to_owned()));
        let press = |journal: Arc<Journald>, from: Option<Cursor>| {
            let core = a_core(vec![
                reader(vec![Source::Journal(killed())], from, 10).boxed(),
            ]);
            core.set("reader", &Knob::Enabled(false)).expect("off");
            let (mut d, rx) = by_hand(&core, "reader", vec![killed()], Some(journal));
            d.reconcile();
            core.act("reader", "read", None, None).expect("asked");
            d.reconcile();
            d.hear(the_pass(&rx));
            noted_now(&core, "reader")
        };
        // THE CURSOR WOULD NOT BE TAKEN: this boot, in one more try.
        let (journal, _) = Journald::with(past.clone(), 0);
        journal.refusing.store(1, Ordering::Release);
        assert_eq!(
            press(journal.clone(), from()),
            ["read 0 Killed this boot", "ended lost=0 last=s=t;i=2 why=-"]
        );
        let opened = journal.opened();
        assert_eq!(opened[0][3], "--after-cursor=not a cursor");
        assert_eq!(opened[1][3], "--boot");
        // NOT EVEN THAT: a failed pass, which moves no place and says why. A
        // journal with no cursor to blame gets one try.
        let (journal, _) = Journald::with(past, 0);
        journal.refusing.store(2, Ordering::Release);
        assert_eq!(
            press(journal.clone(), from()),
            ["ended lost=0 last=- why=unreadable"]
        );
        assert_eq!(journal.opened().len(), 2);
        let (journal, _) = Journald::with(Vec::new(), 0);
        journal.refusing.store(1, Ordering::Release);
        assert_eq!(
            press(journal.clone(), None),
            ["ended lost=0 last=- why=unreadable"]
        );
        assert_eq!(journal.opened().len(), 1);
        // AN EMPTY ONE has no `last` either, and no reason: nothing was
        // written, which is an answer.
        let (journal, _) = Journald::with(Vec::new(), 0);
        assert_eq!(press(journal, from()), ["ended lost=0 last=- why=-"]);
    }

    #[test]
    fn a_pass_that_fails_after_a_line_says_so_and_the_next_goes_on_from_there() {
        // THE DEFECT THIS CATCHES: journalctl failing partway read as the end
        // of the journal, with what came after it never read and the module
        // given nothing to act on.
        let past = vec![
            entry(1, "Killed first", 3),
            entry(2, "Killed second", 3),
            entry(3, "Killed third", 3),
        ];
        let core = a_core(vec![
            reader(
                vec![Source::Journal(killed())],
                Some(Cursor("s=t;i=0".to_owned())),
                10,
            )
            .boxed(),
        ]);
        core.set("reader", &Knob::Enabled(false)).expect("off");
        let (journal, _) = Journald::with(past, 0);
        journal.cutting.store(2, Ordering::Release);
        let (mut d, rx) = by_hand(&core, "reader", vec![killed()], Some(journal.clone()));
        d.reconcile();
        core.act("reader", "read", None, None).expect("asked");
        d.reconcile();
        d.hear(the_pass(&rx));
        // WHAT IT READ STANDS, and how far it got.
        assert_eq!(
            noted_now(&core, "reader"),
            [
                "read 0 Killed first",
                "read 0 Killed second",
                "ended lost=0 last=s=t;i=2 why=unreadable"
            ]
        );
        // ITS CURSOR WAS TAKEN: one try, and never this boot over what it has.
        assert_eq!(journal.opened().len(), 1);
        core.act("reader", "read", None, None).expect("asked");
        d.reconcile();
        d.hear(the_pass(&rx));
        assert_eq!(
            noted_now(&core, "reader")[3..],
            ["read 0 Killed third", "ended lost=0 last=s=t;i=3 why=-"]
        );
        assert_eq!(journal.opened()[1][3], "--after-cursor=s=t;i=2");
    }

    /// What a pass told the log, read on this thread so the log can be read.
    fn logged_by(journal: Arc<Journald>, after: Option<Cursor>) -> Vec<crate::tail::Line> {
        use tracing_subscriber::layer::SubscriberExt as _;
        let backfill = Backfill::ThisBoot { most: 10 };
        let (tx, _rx) = mpsc::channel();
        let reader = Reader {
            module: "reader",
            journalist: journal,
            ears: vec![(
                0,
                Match {
                    backfill,
                    ..killed()
                },
            )],
            after,
            until: None,
            tx,
            stop: Arc::new(AtomicBool::new(false)),
        };
        let (_, before) = crate::tail::global().after(0);
        let subscriber = tracing_subscriber::registry().with(crate::tail::Copier);
        tracing::subscriber::with_default(subscriber, || reader.run());
        let (lines, _) = crate::tail::global().after(before);
        lines
            .into_iter()
            .filter(|l| l.text.contains("pass"))
            .collect()
    }

    #[test]
    fn a_pass_tells_the_log_about_itself_and_never_about_the_follower() {
        // THE DEFECT THIS CATCHES: a pass that failed wrote that the journal
        // is not being followed, with the follower running, and a refused
        // cursor wrote it before a second try that worked.
        let past = || vec![entry(1, "Killed", 3)];
        let refused = || Some(Cursor("not a cursor".to_owned()));
        // A RETRY THAT WORKS is no failure.
        let (journal, _) = Journald::with(past(), 0);
        journal.refusing.store(1, Ordering::Release);
        let lines = logged_by(journal, refused());
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0].level, crate::tail::Level::Info, "{lines:?}");

        // ONE CUT SHORT is, and so is one with nowhere left to read.
        for (refusing, cutting, after) in [(0, 1, None), (2, 0, refused())] {
            let (journal, _) = Journald::with(past(), 0);
            journal.refusing.store(refusing, Ordering::Release);
            journal.cutting.store(cutting, Ordering::Release);
            let lines = logged_by(journal, after);
            let warned = lines.last().expect("a line");
            assert_eq!(warned.level, crate::tail::Level::Warn, "{lines:?}");
            assert!(warned.text.contains("\"reader\""), "{lines:?}");
            assert!(warned.text.contains("unreadable"), "{lines:?}");
            assert!(
                lines.iter().all(|l| !l.text.contains("not being followed")),
                "{lines:?}"
            );
        }
    }

    #[test]
    fn a_pass_that_cannot_begin_is_answered_all_the_same() {
        // A MODULE WAITING FOR ITS ANSWER would say it is reading for ever.
        let core = a_core(vec![
            reader(vec![Source::Journal(killed())], None, 10).boxed(),
        ]);
        core.set("reader", &Knob::Enabled(false)).expect("off");
        let (mut d, _rx) = by_hand(&core, "reader", vec![killed()], None);
        d.reconcile();
        core.act("reader", "read", None, None).expect("asked");
        d.reconcile();
        assert_eq!(
            noted_now(&core, "reader"),
            ["ended lost=0 last=- why=no-program"]
        );
        assert!(
            d.passing.is_empty(),
            "a pass that never began is on its way"
        );

        let core = a_core(vec![
            reader(vec![Source::Journal(killed())], None, 10).boxed(),
        ]);
        core.set("reader", &Knob::Enabled(false)).expect("off");
        let (mut d, rx) = by_hand(&core, "reader", vec![killed()], Some(Arc::new(NoStart)));
        d.reconcile();
        core.act("reader", "read", None, None).expect("asked");
        d.reconcile();
        d.hear(the_pass(&rx));
        assert_eq!(
            noted_now(&core, "reader"),
            ["ended lost=0 last=- why=unstartable"]
        );

        // A MODULE WITH NO JOURNAL SOURCE has nothing to read, which is no
        // failure.
        let core = a_core(vec![reader(Vec::new(), None, 10).boxed()]);
        core.set("reader", &Knob::Enabled(false)).expect("off");
        let (journal, _) = Journald::with(vec![entry(1, "Killed", 3)], 0);
        let (mut d, _rx) = by_hand(&core, "reader", Vec::new(), Some(journal.clone()));
        d.reconcile();
        core.act("reader", "read", None, None).expect("asked");
        d.reconcile();
        assert_eq!(noted_now(&core, "reader"), ["ended lost=0 last=- why=-"]);
        assert!(journal.opened().is_empty(), "nobody wants a journal here");
    }

    #[test]
    fn going_off_is_told_once_and_a_brain_that_stops_tells_nobody() {
        let core = a_core(vec![
            Fake::watch("sleeper")
                .listens(Vec::new(), |_, _| {})
                .stops(|_, ctx| ctx.record(Noted::of("sleeper-stopped")))
                .boxed(),
        ]);
        let (mut d, _rx) = by_hand(&core, "sleeper", Vec::new(), None);
        let stops = || {
            core.refresh();
            let mind = core.shared.snapshot().0;
            mind.journal
                .iter()
                .filter(|r| r.says == "sleeper-stopped")
                .count()
        };
        d.reconcile();
        d.leave();
        assert_eq!(stops(), 0, "the brain's own end told it to stop");
        core.set("sleeper", &Knob::Enabled(false)).expect("off");
        d.reconcile();
        d.reconcile();
        assert_eq!(stops(), 1);
    }

    #[test]
    fn what_a_module_keeps_is_written_before_the_cursor_that_covers_it() {
        // THE DEFECT THIS CATCHES: the cursor on disk ahead of a module's
        // list. A restart starts after the cursor, so the lines between the
        // two were heard and are nowhere — and a real stop runs no `leave`.
        let dir = scratch("keep");
        let file = dir.join(CURSOR_FILE);
        let saw = Arc::new(Mutex::new(Vec::<Option<Cursor>>::new()));
        let (seen, looked) = (Arc::clone(&saw), file.clone());
        let core = a_core(vec![
            scribe("scribe", vec![Source::Journal(killed())])
                .keeps(move |_, _| {
                    seen.lock().expect("saw").push(Cursor::read_from(&looked));
                })
                .boxed(),
            scribe("sleeper", vec![Source::Journal(killed())])
                .keeps(|_, _| panic!("a module switched off was asked to keep"))
                .boxed(),
        ]);
        core.set("sleeper", &Knob::Enabled(false)).expect("off");
        let (mut d, _rx) = by_hand(&core, "scribe", vec![killed()], None);
        d.keeper = Keeper::at(Some(dir.clone()), CURSOR_FILE, Duration::ZERO);
        d.reconcile();
        // NOTHING HEARD: nothing to cover, nobody asked.
        d.keep_cursor(Instant::now());
        assert!(saw.lock().expect("saw").is_empty(), "asked for nothing");

        d.hear(Msg::Entry(entry(1, "Killed", 3)));
        d.keep_cursor(Instant::now());
        // AND ON THE WAY OUT TOO.
        d.hear(Msg::Entry(entry(2, "Killed again", 3)));
        d.leave();
        let cursor = |n: u64| Some(Cursor(format!("s=t;i={n}")));
        assert_eq!(
            *saw.lock().expect("saw"),
            [None, cursor(1)],
            "the module was asked after the cursor moved"
        );
        assert_eq!(Cursor::read_from(&file), cursor(2));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
