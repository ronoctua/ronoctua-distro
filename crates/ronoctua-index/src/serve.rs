//! The unit (feature `write`): reconcile, then follow the kernel.
//!
//! # One thread, no clock at rest
//!
//! The unit waits in `poll` on its two inotify descriptors with no deadline
//! while nothing is pending. A change arms the flush: it lands after [`QUIET`]
//! of silence, or [`HOLD`] after the first change of a burst that never goes
//! quiet, or at once past [`crate::crawl::BATCH`] paths — and the deadline is
//! gone again. A home nobody touches costs nothing.
//!
//! # What it watches, on two descriptors
//!
//! On the FOLDERS' descriptor, every folder it walked, one watch each
//! (`fs.inotify.max_user_watches` is raised by `nix/modules/index.nix`; past
//! it, the rest is caught by the next reconcile, and the log says so once).
//!
//! On the TRIGGERS' descriptor, the folders that hold:
//! - `index.json` (the person's settings): a change walks again;
//! - `/run/current-system` and the person's profile link: a switch or a
//!   `nix profile` change relists the manuals and the packages, when their
//!   signature moved;
//! - the video catalogue and its write-ahead log: new videos, and a deleted
//!   one taken out.
//!
//! Two descriptors because the kernel keeps ONE mask per folder per
//! descriptor: the profile link's folder is the home, which is also the first
//! folder walked, and on one descriptor a trigger's flags became the walked
//! home's (`IN_MASK_ADD` ORs them). The catalogue's write-ahead log needs
//! `MODIFY` — it is written and never closed — and a walked folder must never
//! have it: a file held open and written in the home would be a batch every
//! second. A trigger folder whose trigger has not appeared yet is watched
//! through its parent, and every time a trigger fires they are armed again,
//! so a folder that appears later is watched from then on.
//!
//! `IN_Q_OVERFLOW` means events were lost, and which ones is unknowable: on
//! the folders' descriptor every root is walked again (the stamps make that
//! cheap for what did not change); on the triggers', every trigger is taken
//! as fired.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::fd::{AsFd, BorrowedFd, OwnedFd};
use rustix::fs::inotify::{self, ReadFlags, WatchFlags};
use rustix::io::Errno;

use crate::crawl::{BATCH, Crawler, Limits, Rules, Walked};
use crate::videos::Videos;
use crate::write::Writer;
use crate::{Kind, Refusal, Settings, Why, man, packages, settings};

/// Silence after the last change before the batch is applied.
pub const QUIET: Duration = Duration::from_secs(1);

/// The longest a batch waits for silence that is not coming.
pub const HOLD: Duration = Duration::from_secs(10);

/// What a walked folder is watched for. `MODIFY` is absent: it fires on every
/// `write`, and `CLOSE_WRITE` is when a writer let go.
const FOLDER: WatchFlags = WatchFlags::CREATE
    .union(WatchFlags::DELETE)
    .union(WatchFlags::MOVED_FROM)
    .union(WatchFlags::MOVED_TO)
    .union(WatchFlags::CLOSE_WRITE)
    .union(WatchFlags::ATTRIB)
    .union(WatchFlags::DELETE_SELF)
    .union(WatchFlags::MOVE_SELF)
    .union(WatchFlags::ONLYDIR)
    .union(WatchFlags::DONT_FOLLOW)
    .union(WatchFlags::EXCL_UNLINK)
    .union(WatchFlags::MASK_ADD);

/// A root, which is FOLLOWED: a root may be a link to the folder it names
/// (`~/Dropbox` on another disk), and with `DONT_FOLLOW` the watch would be
/// asked of the link, which is not a folder.
const ROOT: WatchFlags = FOLDER.difference(WatchFlags::DONT_FOLLOW);

/// How `folder`, walked from `root`, is watched.
fn walked(folder: &Path, root: &Path) -> WatchFlags {
    if folder == root { ROOT } else { FOLDER }
}

/// What a folder holding a trigger is watched for, on the triggers' own
/// descriptor: a file written and closed, created, replaced by a rename,
/// removed. `MODIFY` is not asked: it fires on every `write` to every file in
/// the folder, and the one trigger that needs it asks it alone.
const TRIGGER: WatchFlags = WatchFlags::CREATE
    .union(WatchFlags::MOVED_TO)
    .union(WatchFlags::CLOSE_WRITE)
    .union(WatchFlags::DELETE)
    .union(WatchFlags::ONLYDIR)
    .union(WatchFlags::MASK_ADD);

/// The catalogue's folder: its write-ahead log is written and never closed,
/// so `MODIFY` too — on the triggers' descriptor, where no walked folder's
/// watch can take it.
const CATALOGUE: WatchFlags = TRIGGER.union(WatchFlags::MODIFY);

/// Which sources the unit indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sources {
    /// The files under the roots.
    pub files: bool,
    /// The manual pages.
    pub manuals: bool,
    /// The packages one can install.
    pub packages: bool,
    /// The video catalogue.
    pub videos: bool,
}

impl Sources {
    /// Every source.
    pub const ALL: Self = Self {
        files: true,
        manuals: true,
        packages: true,
        videos: true,
    };

    /// From `files,manuals,packages,videos`; an unknown word is refused.
    ///
    /// # Errors
    ///
    /// The word that is not a source.
    pub fn parse(list: &str) -> Result<Self, String> {
        let mut out = Self {
            files: false,
            manuals: false,
            packages: false,
            videos: false,
        };
        for word in list.split(',').map(str::trim).filter(|w| !w.is_empty()) {
            match word {
                "files" => out.files = true,
                "manuals" => out.manuals = true,
                "packages" => out.packages = true,
                "videos" => out.videos = true,
                other => return Err(other.to_owned()),
            }
        }
        Ok(out)
    }
}

/// How the unit was started.
pub struct Config {
    /// The index's folder.
    pub dir: PathBuf,
    /// `assets/cache.nix`'s numbers.
    pub limits: Limits,
    /// The writer's arena, in bytes.
    pub memory_bytes: usize,
    /// Roots given on the command line, over the settings' (checks, tests).
    pub roots: Option<Vec<PathBuf>>,
    /// Which sources.
    pub sources: Sources,
    /// The home: the root when the settings name none.
    pub home: Option<PathBuf>,
    /// Where `index.json` is (`None`: the defaults, unwatched).
    pub settings_dir: Option<PathBuf>,
    /// Where manual folders may be (the ones that exist are read).
    pub man_candidates: Vec<PathBuf>,
    /// The Nix index file (`RONOCTUA_NIX_INDEX`).
    pub packages: Option<PathBuf>,
    /// The video catalogue.
    pub videos: Option<Box<dyn Videos>>,
    /// Paths whose change relists the manuals and the packages.
    pub system_triggers: Vec<PathBuf>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("dir", &self.dir)
            .field("limits", &self.limits)
            .field("sources", &self.sources)
            .finish_non_exhaustive()
    }
}

/// What one pass over every source did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pass {
    /// The files.
    pub files: Walked,
    /// Manual pages added (0: unchanged).
    pub manuals: u64,
    /// Packages added (0: unchanged).
    pub packages: u64,
    /// Videos added or replaced.
    pub videos: u64,
}

/// A change waiting for the flush.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Change {
    /// A file (or a folder's own entry): recheck it.
    File(PathBuf),
    /// A folder: recheck it and everything under it.
    Tree(PathBuf),
}

/// What a trigger path means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trigger {
    Settings,
    System,
    Videos,
}

/// Flags a batch carries beside its paths.
#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    rewalk: bool,
    force: bool,
    settings: bool,
    system: bool,
    videos: bool,
}

impl Flags {
    const fn any(self) -> bool {
        self.rewalk || self.settings || self.system || self.videos
    }
}

/// One watch: its folder, and the root it was walked from (`None`: a folder
/// watched only for a trigger).
#[derive(Debug, Clone)]
struct Watch {
    path: PathBuf,
    root: Option<PathBuf>,
}

/// The watches, both ways.
#[derive(Debug, Default)]
struct Watches {
    by_wd: HashMap<i32, Watch>,
    by_path: BTreeMap<PathBuf, i32>,
    refused: bool,
}

impl Watches {
    fn add(
        &mut self,
        fd: Option<BorrowedFd<'_>>,
        path: &Path,
        root: Option<&Path>,
        flags: WatchFlags,
    ) {
        let Some(fd) = fd else {
            return;
        };
        match inotify::add_watch(fd, path, flags) {
            Ok(wd) => {
                // A folder moved inside the tree keeps its watch: the same
                // descriptor comes back for its new path.
                let kept_root = self.by_wd.get(&wd).and_then(|w| w.root.clone());
                if let Some(old) = self.by_wd.get(&wd)
                    && old.path != path
                {
                    self.by_path.remove(&old.path);
                }
                let root = root.map(Path::to_path_buf).or(kept_root);
                self.by_wd.insert(
                    wd,
                    Watch {
                        path: path.to_path_buf(),
                        root,
                    },
                );
                self.by_path.insert(path.to_path_buf(), wd);
            }
            Err(Errno::NOSPC) => {
                if !self.refused {
                    self.refused = true;
                    tracing::warn!(
                        "inotify's watches ran out (fs.inotify.max_user_watches); folders past this are caught by the next reconcile"
                    );
                }
            }
            Err(_) => {}
        }
    }

    /// Removes the watches of `path` and every folder under it.
    fn drop_under(&mut self, fd: Option<BorrowedFd<'_>>, path: &Path) {
        let under: Vec<(PathBuf, i32)> = self
            .by_path
            .range(path.to_path_buf()..)
            .take_while(|(p, _)| p.starts_with(path))
            .map(|(p, wd)| (p.clone(), *wd))
            .collect();
        for (p, wd) in under {
            if self.by_wd.get(&wd).is_some_and(|w| w.root.is_some()) {
                if let Some(fd) = fd {
                    let _ = inotify::remove_watch(fd, wd);
                }
                self.by_path.remove(&p);
                self.by_wd.remove(&wd);
            }
        }
    }

    /// Removes the folder watches whose root is not one of `roots`.
    fn keep_roots(&mut self, fd: Option<BorrowedFd<'_>>, roots: &[PathBuf]) {
        let stale: Vec<i32> = self
            .by_wd
            .iter()
            .filter(|(_, w)| w.root.as_ref().is_some_and(|r| !roots.contains(r)))
            .map(|(wd, _)| *wd)
            .collect();
        for wd in stale {
            if let Some(w) = self.by_wd.remove(&wd) {
                self.by_path.remove(&w.path);
                if let Some(fd) = fd {
                    let _ = inotify::remove_watch(fd, wd);
                }
            }
        }
    }

    /// The kernel dropped a watch (`IN_IGNORED`).
    fn forget(&mut self, wd: i32) {
        if let Some(w) = self.by_wd.remove(&wd)
            && self.by_path.get(&w.path) == Some(&wd)
        {
            self.by_path.remove(&w.path);
        }
    }
}

/// Opens the index for writing: one of another format is wiped and built
/// again (it is a cache).
///
/// # Errors
///
/// [`Why::Busy`] (another unit holds it), [`Why::Unreadable`].
pub fn open_writer(dir: &Path, memory_bytes: usize) -> Result<Writer, Refusal> {
    match Writer::open(dir, memory_bytes) {
        Err(Refusal {
            why: Why::OtherFormat,
        }) => {
            tracing::info!(dir = %dir.display(), "the index is of another format; building it again");
            Writer::wipe(dir).map_err(|_| Refusal {
                why: Why::Unreadable,
            })?;
            Writer::open(dir, memory_bytes)
        }
        other => other,
    }
}

/// The unit's state.
pub struct Unit {
    config: Config,
    crawler: Crawler,
    settings: Option<Settings>,
    /// The walked folders' watches (the folders' descriptor).
    watches: Watches,
    /// The trigger folders' watches (the triggers' descriptor).
    trigger_watches: Watches,
    triggers: HashMap<PathBuf, Trigger>,
    /// When `serve` began, seconds since the epoch — said in `status.json`.
    since_s: i64,
    /// Whether `serve` began with an empty index.
    scratch: bool,
    /// When the last commit landed.
    committed_s: Option<i64>,
}

impl std::fmt::Debug for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Unit")
            .field("config", &self.config)
            .field("watches", &self.watches.by_wd.len())
            .field("trigger_watches", &self.trigger_watches.by_wd.len())
            .finish_non_exhaustive()
    }
}

/// The walk's rules: the limits, the settings, and never the index's own
/// folder.
fn rules_of(config: &Config, settings: Option<&Settings>) -> Rules {
    Rules::new(&config.limits, settings.unwrap_or(&Settings::default())).without(&config.dir)
}

impl Unit {
    /// Opens the index and reads the settings.
    ///
    /// # Errors
    ///
    /// As [`open_writer`].
    pub fn new(config: Config) -> Result<Self, Refusal> {
        let settings = config
            .settings_dir
            .clone()
            .map_or_else(|| Some(Settings::default()), |dir| read_settings(&dir));
        // The writer first: it makes the folder the rules keep out.
        let writer = open_writer(&config.dir, config.memory_bytes)?;
        let rules = rules_of(&config, settings.as_ref());
        Ok(Self {
            crawler: Crawler::new(writer, rules, config.limits.ceiling_bytes),
            config,
            settings,
            watches: Watches::default(),
            trigger_watches: Watches::default(),
            triggers: HashMap::new(),
            since_s: 0,
            scratch: false,
            committed_s: None,
        })
    }

    /// The writer (the one-shot build finishes it).
    #[must_use]
    pub fn into_writer(self) -> Writer {
        self.crawler.writer
    }

    /// What holding the ceiling has cost since the unit started.
    #[must_use]
    pub const fn held(&self) -> crate::crawl::Held {
        self.crawler.held()
    }

    /// The roots the files come from.
    fn roots(&self) -> Option<Vec<PathBuf>> {
        if let Some(roots) = &self.config.roots {
            return Some(roots.clone());
        }
        let settings = self.settings.as_ref()?;
        let home = self.config.home.as_deref()?;
        Some(settings.roots_or(home))
    }

    /// Walks every root (reconcile) and deletes what is gone.
    fn reconcile(&mut self, fd: Option<BorrowedFd<'_>>, force: bool) -> Result<Walked, Refusal> {
        if !self.config.sources.files {
            return Ok(Walked::default());
        }
        let Some(roots) = self.roots() else {
            tracing::error!(
                "index.json does not parse (or there is no home): the files keep what the last walk indexed"
            );
            return Ok(Walked::default());
        };
        let started = Instant::now();
        self.watches.keep_roots(fd, &roots);
        let mut seen = HashSet::new();
        let mut total = Walked::default();
        for root in &roots {
            let watches = &mut self.watches;
            let walked = self.crawler.walk(root, &mut seen, force, &mut |folder| {
                watches.add(fd, folder, Some(root), walked(folder, root));
            })?;
            total.entries += walked.entries;
            total.indexed += walked.indexed;
        }
        total.deleted = self.crawler.prune(&roots, &seen)?;
        let contents = self.crawler.rules.contents;
        self.crawler.writer.state_mut().contents = Some(contents);
        tracing::info!(
            entries = total.entries,
            indexed = total.indexed,
            deleted = total.deleted,
            watches = self.watches.by_wd.len(),
            seconds = started.elapsed().as_secs(),
            "the files are reconciled"
        );
        Ok(total)
    }

    /// Relists the manual pages when their folders resolve somewhere new.
    fn manuals(&mut self) -> Result<u64, Refusal> {
        if !self.config.sources.manuals {
            return Ok(0);
        }
        let folders: Vec<PathBuf> = self
            .config
            .man_candidates
            .iter()
            .filter(|p| p.is_dir())
            .cloned()
            .collect();
        let signature = man::signature(&folders);
        if self.crawler.writer.state().manuals.as_deref() == Some(signature.as_str()) {
            return Ok(0);
        }
        self.crawler.writer.delete_kind(Kind::Man);
        let mut added = 0;
        for (name, section, path) in man::pages(&folders) {
            if let Some(document) =
                man::document(&name, &section, &path, self.crawler.rules.content_bytes)
            {
                self.crawler.writer.add(&document)?;
                added += 1;
            }
        }
        self.crawler.writer.state_mut().manuals = Some(signature);
        tracing::info!(pages = added, "the manual pages are relisted");
        Ok(added)
    }

    /// Relists the packages when the Nix index file changed.
    fn packages(&mut self) -> Result<u64, Refusal> {
        if !self.config.sources.packages {
            return Ok(0);
        }
        let Some(path) = self.config.packages.clone() else {
            tracing::info!("{} is not set: no packages are indexed", packages::VARIABLE);
            return Ok(0);
        };
        let Some(signature) = packages::signature(&path) else {
            tracing::warn!(path = %path.display(), "the Nix index is not there");
            return Ok(0);
        };
        if self.crawler.writer.state().packages.as_deref() == Some(signature.as_str()) {
            return Ok(0);
        }
        let Some(documents) = packages::documents(&path, self.crawler.rules.content_bytes) else {
            return Ok(0);
        };
        self.crawler.writer.delete_kind(Kind::Package);
        for document in &documents {
            self.crawler.writer.add(document)?;
        }
        self.crawler.writer.state_mut().packages = Some(signature);
        tracing::info!(packages = documents.len(), "the packages are relisted");
        Ok(documents.len() as u64)
    }

    /// A change of the catalogue: the videos changed since the generation
    /// taken are taken; then — a DELETED video is never answered again (the
    /// catalogue keeps its tombstone and `all_since` leaves it out) — what
    /// the index holds is counted against what the catalogue shows, and more
    /// in the index means one was deleted: the videos are taken again whole.
    ///
    /// The count is one `COUNT` per batch; the whole take happens only after
    /// the person deleted something. (A video added between the two
    /// questions can hide a deletion for one batch; the addition is itself a
    /// change of the catalogue, and the next batch sees both.)
    ///
    /// # Errors
    ///
    /// The writer's.
    pub(crate) fn follow_videos(&mut self) -> Result<(), Refusal> {
        if !self.config.sources.videos {
            return Ok(());
        }
        let since = self.crawler.writer.state().videos.unwrap_or(0);
        self.videos(false)?;
        let Some(source) = self.config.videos.as_mut() else {
            return Ok(());
        };
        // WHAT THE CATALOGUE TOOK OUT since the last generation taken —
        // deleted in the player, unavailable on YouTube — leaves the index by
        // its id; the count below stays as the net for what this misses.
        match source.gone_since(since) {
            Ok(gone) => {
                for id in gone {
                    self.crawler
                        .writer
                        .delete(&crate::videos::Found::index_id_of(&id))?;
                }
            }
            Err(why) => tracing::debug!(why, "the video catalogue did not say what left it"),
        }
        let Some(source) = self.config.videos.as_mut() else {
            return Ok(());
        };
        let shown = match source.shown() {
            Ok(n) => n,
            Err(why) => {
                tracing::debug!(why, "the video catalogue did not say how many it shows");
                return Ok(());
            }
        };
        // What the index holds is what its last commit holds.
        if self.crawler.writer.pending() > 0 {
            self.crawler.commit()?;
        }
        let held = self.crawler.writer.count_kind(Kind::Video);
        if held > shown {
            tracing::info!(
                held,
                shown,
                "the catalogue shows fewer videos than the index holds; taking them again whole"
            );
            self.videos(true)?;
            self.crawler.commit()?;
        }
        Ok(())
    }

    /// Takes the videos changed since the last generation (`whole`: all of
    /// them, the old ones deleted first).
    fn videos(&mut self, whole: bool) -> Result<u64, Refusal> {
        if !self.config.sources.videos {
            return Ok(0);
        }
        let since = if whole {
            0
        } else {
            self.crawler.writer.state().videos.unwrap_or(0)
        };
        let Some(source) = self.config.videos.as_mut() else {
            return Ok(0);
        };
        let (found, generation) = match source.changed_since(since) {
            Ok(answer) => answer,
            Err(why) => {
                tracing::debug!(why, "the video catalogue did not answer");
                return Ok(0);
            }
        };
        if whole {
            self.crawler.writer.delete_kind(Kind::Video);
        }
        let mut taken = 0;
        for video in &found {
            if video.hidden {
                self.crawler.writer.delete(&video.index_id())?;
            } else {
                self.crawler
                    .writer
                    .replace(&video.document(self.crawler.rules.content_bytes))?;
                taken += 1;
            }
        }
        self.crawler.writer.state_mut().videos = Some(generation);
        Ok(taken)
    }

    /// Says what the unit is doing in `status.json`, beside the index — the
    /// brain's mark and page read it (`crate::status`). A failed write is
    /// logged and nothing else: the index is the unit's work, the status a
    /// courtesy.
    fn tell(&self, phase: crate::Phase, scratch: bool, since_s: i64, committed_s: Option<i64>) {
        let status = crate::Status {
            pid: std::process::id(),
            phase,
            scratch,
            since_s,
            documents: self.crawler.writer.documents(),
            committed_s,
        };
        if let Err(e) = crate::status::write(&self.config.dir, &status) {
            tracing::warn!(error = %e, "the index's status could not be written");
        }
    }

    /// Every source once, then a commit.
    ///
    /// # Errors
    ///
    /// The writer's.
    pub fn pass(&mut self, fd: Option<BorrowedFd<'_>>) -> Result<Pass, Refusal> {
        // A walk that read contents and one that did not leave different
        // documents behind the same stamps: a change of the setting reads
        // every file again.
        let force = self
            .crawler
            .writer
            .state()
            .contents
            .is_some_and(|was| was != self.crawler.rules.contents);
        let pass = Pass {
            files: self.reconcile(fd, force)?,
            manuals: self.manuals()?,
            packages: self.packages()?,
            videos: self.videos(true)?,
        };
        self.crawler.commit()?;
        Ok(pass)
    }

    /// Watches, on the triggers' descriptor, the settings, the system's and
    /// profile's links and the video catalogue — as they are NOW: called again
    /// whenever a trigger fires, so a folder that appeared since (the
    /// settings' folder, made when the person first saves them; the
    /// catalogue's) is watched from then on, and a profile link that names
    /// another link has that one watched.
    ///
    /// Nothing is created: the unit's home is read-only, and a folder the
    /// person has not made yet is watched through its parent.
    fn watch_triggers(&mut self, fd: BorrowedFd<'_>) {
        let mut wanted: Vec<(PathBuf, Trigger)> = Vec::new();
        if let Some(dir) = &self.config.settings_dir {
            wanted.push((dir.join(settings::FILE), Trigger::Settings));
        }
        if self.config.sources.manuals || self.config.sources.packages {
            for link in &self.config.system_triggers {
                wanted.push((link.clone(), Trigger::System));
                // A profile link names the link that changes (`profile` in
                // its profiles folder): that one is watched too. Never a
                // target in the store — a watch there would wake on every
                // build the machine makes.
                if let Ok(target) = std::fs::read_link(link) {
                    let target = link.parent().map_or(target.clone(), |p| p.join(&target));
                    if !target.starts_with("/nix/store") {
                        wanted.push((target, Trigger::System));
                    }
                }
            }
        }
        if self.config.sources.videos
            && let Some(file) = self.config.videos.as_ref().and_then(|v| v.watched())
        {
            let mut wal = file.clone().into_os_string();
            wal.push("-wal");
            wanted.push((PathBuf::from(wal), Trigger::Videos));
            wanted.push((file, Trigger::Videos));
        }
        for (path, trigger) in wanted {
            let flags = if trigger == Trigger::Videos {
                CATALOGUE
            } else {
                TRIGGER
            };
            if let Some(dir) = path.parent() {
                if !dir.is_dir() {
                    // Not there yet (no catalogue yet): its parent is watched
                    // for the folder, which is then a trigger itself.
                    if let Some(up) = dir.parent() {
                        self.trigger_watches.add(Some(fd), up, None, TRIGGER);
                        self.triggers.insert(dir.to_path_buf(), trigger);
                    }
                } else {
                    self.trigger_watches.add(Some(fd), dir, None, flags);
                }
            }
            self.triggers.insert(path, trigger);
        }
    }

    /// One event of the triggers' descriptor into the batch's flags.
    fn heard_trigger(
        &mut self,
        wd: i32,
        mask: ReadFlags,
        name: Option<&OsString>,
        flags: &mut Flags,
    ) {
        if mask.contains(ReadFlags::QUEUE_OVERFLOW) {
            // Which trigger was lost is unknowable: all of them fired. Each
            // is cheap when nothing moved (a signature, a count).
            flags.settings = true;
            flags.system = true;
            flags.videos = true;
            return;
        }
        if mask.contains(ReadFlags::IGNORED) {
            self.trigger_watches.forget(wd);
            return;
        }
        let Some(watch) = self.trigger_watches.by_wd.get(&wd) else {
            return;
        };
        let full = name.map_or_else(|| watch.path.clone(), |n| watch.path.join(n));
        match self.triggers.get(&full) {
            Some(Trigger::Settings) => flags.settings = true,
            Some(Trigger::System) => flags.system = true,
            Some(Trigger::Videos) => flags.videos = true,
            None => {}
        }
    }

    /// One event of the folders' descriptor into the batch.
    fn heard_folder(
        &mut self,
        wd: i32,
        mask: ReadFlags,
        name: Option<&OsString>,
        pending: &mut HashMap<PathBuf, Change>,
        flags: &mut Flags,
    ) {
        if mask.contains(ReadFlags::QUEUE_OVERFLOW) {
            flags.rewalk = true;
            return;
        }
        if mask.contains(ReadFlags::IGNORED) {
            self.watches.forget(wd);
            return;
        }
        let Some(watch) = self.watches.by_wd.get(&wd) else {
            return;
        };
        let full = name.map_or_else(|| watch.path.clone(), |n| watch.path.join(n));
        let Some(root) = watch.root.clone() else {
            return;
        };
        // A path the rules leave out is not news: a shell's history written
        // at every prompt in a walked home would be a commit a second.
        if self.crawler.rules.skips_under(&root, &full) {
            return;
        }
        if name.is_none() {
            if mask.intersects(ReadFlags::DELETE_SELF | ReadFlags::MOVE_SELF) {
                pending.insert(full, Change::Tree(root));
            }
            return;
        }
        if mask.contains(ReadFlags::ISDIR) {
            if mask.intersects(
                ReadFlags::CREATE | ReadFlags::MOVED_TO | ReadFlags::DELETE | ReadFlags::MOVED_FROM,
            ) {
                pending.insert(full, Change::Tree(root));
            }
        } else {
            pending.entry(full).or_insert(Change::File(root));
        }
    }

    /// Re-reads `index.json`; a change walks again.
    fn reread_settings(&mut self, flags: &mut Flags) {
        let Some(dir) = self.config.settings_dir.clone() else {
            return;
        };
        let Some(new) = read_settings(&dir) else {
            return;
        };
        if self.settings.as_ref() == Some(&new) {
            return;
        }
        let was = self.crawler.rules.contents;
        self.crawler.rules = rules_of(&self.config, Some(&new));
        flags.rewalk = true;
        flags.force |= was != new.contents;
        self.settings = Some(new);
    }

    /// Applies a batch, then commits.
    fn apply(
        &mut self,
        fd: BorrowedFd<'_>,
        triggers: BorrowedFd<'_>,
        pending: HashMap<PathBuf, Change>,
        mut flags: Flags,
    ) -> Result<(), Refusal> {
        if flags.settings || flags.system || flags.videos {
            // A trigger's folder may have just appeared, or a profile link now
            // names another link: armed again as they are now, BEFORE what
            // they hold is read — what is written into a folder from now on
            // is heard, and what was written before is read below.
            self.watch_triggers(triggers);
        }
        if flags.settings {
            self.reread_settings(&mut flags);
        }
        if flags.rewalk {
            self.reconcile(Some(fd), flags.force)?;
        } else {
            for (path, change) in pending {
                match change {
                    Change::File(root) => self.crawler.recheck(&root, &path)?,
                    Change::Tree(root) => {
                        self.watches.drop_under(Some(fd), &path);
                        let watches = &mut self.watches;
                        self.crawler.recheck_tree(&root, &path, &mut |folder| {
                            watches.add(Some(fd), folder, Some(&root), walked(folder, &root));
                        })?;
                    }
                }
            }
        }
        if flags.system {
            self.manuals()?;
            self.packages()?;
        }
        if flags.videos {
            self.follow_videos()?;
        }
        // Nothing changed (every path was as the index had it): no commit, so
        // no door reloads for nothing.
        if self.crawler.writer.pending() == 0 {
            return Ok(());
        }
        self.crawler.commit()?;
        self.committed_s = Some(crate::now_s());
        self.tell(
            crate::Phase::Following,
            self.scratch,
            self.since_s,
            self.committed_s,
        );
        Ok(())
    }

    /// Runs for ever: a pass over every source, then the kernel's events.
    ///
    /// # Errors
    ///
    /// The writer's, or inotify that cannot be opened.
    pub fn serve(&mut self) -> Result<(), Refusal> {
        let open = || {
            inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK).map_err(
                |e| {
                    tracing::error!(error = %e, "inotify could not be opened");
                    Refusal { why: Why::Failed }
                },
            )
        };
        let fd: OwnedFd = open()?;
        let triggers: OwnedFd = open()?;
        self.watch_triggers(triggers.as_fd());
        // FROM SCRATCH when the index holds nothing as the unit starts: a
        // first walk of a home, or one of another format wiped — what the
        // brain marks while it lasts.
        self.since_s = crate::now_s();
        self.scratch = self.crawler.writer.documents() == 0;
        self.tell(crate::Phase::Walking, self.scratch, self.since_s, None);
        let pass = self.pass(Some(fd.as_fd()))?;
        self.committed_s = Some(crate::now_s());
        self.tell(
            crate::Phase::Following,
            self.scratch,
            self.since_s,
            self.committed_s,
        );
        tracing::info!(
            ?pass,
            documents = self.crawler.writer.documents(),
            "the index is up to date; following changes"
        );

        let mut buffer = vec![MaybeUninit::<u8>::uninit(); 64 * 1024];
        let mut pending: HashMap<PathBuf, Change> = HashMap::new();
        let mut flags = Flags::default();
        let mut first: Option<Instant> = None;
        let mut last = Instant::now();
        loop {
            let deadline = first.map(|f| (last + QUIET).min(f + HOLD));
            let timeout = deadline.map(|d| {
                let left = d.saturating_duration_since(Instant::now());
                Timespec {
                    tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
                    tv_nsec: i64::from(left.subsec_nanos()),
                }
            });
            let mut fds = [
                PollFd::new(&fd, PollFlags::IN),
                PollFd::new(&triggers, PollFlags::IN),
            ];
            match rustix::event::poll(&mut fds, timeout.as_ref()) {
                Ok(_) | Err(Errno::INTR) => {}
                Err(e) => {
                    tracing::error!(error = %e, "poll failed");
                    return Err(Refusal { why: Why::Failed });
                }
            }
            let folder_events = drain(fd.as_fd(), &mut buffer);
            let trigger_events = drain(triggers.as_fd(), &mut buffer);
            for (wd, mask, name) in &folder_events {
                self.heard_folder(*wd, *mask, name.as_ref(), &mut pending, &mut flags);
            }
            for (wd, mask, name) in &trigger_events {
                self.heard_trigger(*wd, *mask, name.as_ref(), &mut flags);
            }
            if (!folder_events.is_empty() || !trigger_events.is_empty())
                && (!pending.is_empty() || flags.any())
            {
                let now = Instant::now();
                last = now;
                first.get_or_insert(now);
            }
            let Some(started) = first else {
                continue;
            };
            let now = Instant::now();
            let due = now >= last + QUIET || now >= started + HOLD || pending.len() >= BATCH;
            if due {
                let batch = std::mem::take(&mut pending);
                let these = std::mem::take(&mut flags);
                first = None;
                tracing::debug!(paths = batch.len(), "applying a batch");
                self.apply(fd.as_fd(), triggers.as_fd(), batch, these)?;
            }
        }
    }
}

/// Every event waiting on one descriptor (it is non-blocking: an empty queue
/// ends the read).
fn drain(
    fd: BorrowedFd<'_>,
    buffer: &mut [MaybeUninit<u8>],
) -> Vec<(i32, ReadFlags, Option<OsString>)> {
    let mut events = Vec::new();
    let mut reader = inotify::Reader::new(fd, buffer);
    loop {
        match reader.next() {
            Ok(event) => events.push((
                event.wd(),
                event.events(),
                event
                    .file_name()
                    .map(|n| std::ffi::OsStr::from_bytes(n.to_bytes()).to_os_string()),
            )),
            Err(Errno::INTR) => {}
            Err(_) => break,
        }
    }
    events
}

/// `index.json`, or `None` when it does not parse (a writer's rule: what the
/// person wrote is never replaced by a default — the files wait). The log
/// says why: a skip that is neither a name nor a path is refused by name.
fn read_settings(dir: &Path) -> Option<Settings> {
    let store: ronoctua_store::Store<Settings> =
        ronoctua_store::Store::at(Some(dir.to_path_buf()), settings::FILE);
    let parsed = store.parsed();
    if parsed.is_none() {
        let path = dir.join(settings::FILE);
        let why = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).err())
            .map_or_else(String::new, |e| e.to_string());
        tracing::error!(path = %path.display(), why, "index.json does not parse");
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_are_named_and_an_unknown_one_is_refused() {
        assert_eq!(
            Sources::parse("files,manuals,packages,videos"),
            Ok(Sources::ALL)
        );
        let files = Sources::parse("files").unwrap();
        assert!(files.files && !files.manuals && !files.packages && !files.videos);
        assert_eq!(Sources::parse("files,movies"), Err("movies".to_owned()));
    }

    #[test]
    fn a_tree_of_watches_is_dropped_whole_and_nothing_beside_it() {
        let mut w = Watches::default();
        for (wd, p) in [
            (1, "/r/a"),
            (2, "/r/a/b"),
            (3, "/r/a/b/c"),
            (4, "/r/a-x"),
            (5, "/r/ab"),
        ] {
            w.by_wd.insert(
                wd,
                Watch {
                    path: PathBuf::from(p),
                    root: Some(PathBuf::from("/r")),
                },
            );
            w.by_path.insert(PathBuf::from(p), wd);
        }
        w.drop_under(None, Path::new("/r/a"));
        let left: Vec<&str> = w.by_path.keys().map(|p| p.to_str().unwrap()).collect();
        assert_eq!(left, ["/r/a-x", "/r/ab"]);
        assert_eq!(w.by_wd.len(), 2);
        w.forget(4);
        assert_eq!(w.by_path.len(), 1);
    }
}
