//! THE RANKING — the video catalogue's videos, best first, as a page of the
//! player (`LEADER p k`, a row of "Listen and watch"): what the person's
//! channels published, scored by their own rules, one click from the queue.
//!
//! # What it reads, and how
//!
//! The video catalogue (`ronoctua_video_metrics`), opened READ-ONLY
//! (`Catalog::open_ro`) by a thread of this door's own — the player's daemon
//! never sees it: a page of the ranking is a question this door asks the
//! file, as channels by theme are (`crate::player_catalog`). The thread
//! WATCHES `<catalogue>-wal`, where every writer's commit lands, and asks
//! SQLite's `data_version` whether another connection committed before it
//! reads the page again: a scan that adds videos, or the video catalogue's
//! door hiding one, is on this page with no clock (`Catalog` docs). A door
//! with no loop — a photograph's, a test's — reads in place, at once.
//!
//! The order and the page are SQL's (`Catalog::ranked`, `count`): the
//! score is the channel's, the video's static points and its recency,
//! computed at read time — a list read tomorrow is tomorrow's.
//!
//! # A row, a tile
//!
//! Each video is its thumbnail (the catalogue's file on disk, decoded off the
//! loop by `crate::player_posters`), its title, its channel and its score —
//! a row each, the picture at its head (`player_posters::strip`), or a grid
//! of thumbnails (`player_posters::tiled`); `L` and the glyph on the title
//! turn one into the other, and `<prior>`/`<next>`/`<home>`/`<end>` walk the
//! pages. `RET` — the second click — puts the video at the queue's end,
//! TITLED (`Thing::Named`) and ONCE: one the queue has already is the
//! player's refusal, which the view asks about, "no" first
//! (`player::overruled`, `Told::Asks`). `P` plays it now, `a` adds it.
//!
//! # More of one video
//!
//! The `⋯` at the end of a row — `e` on the keyboard — opens the video's own
//! page (`PageId::RankedVideo`): play now, add, watch it in the Chrome kiosk
//! (the footer's `kiosk-open`), download it (the transfers' queue), hide it
//! from the ranking, or delete it from the catalogue for good — after a
//! question, "no" first. Hiding and deleting are the two writes a reader of
//! the catalogue may make (`Catalog::hide`, `Catalog::delete`), carried out
//! off the loop ([`carry`]) and said when they are done.
//!
//! # Without the catalogue
//!
//! A machine with no catalogue yet — nothing ever added — says so in a
//! sentence that teaches where channels are added; a catalogue that cannot
//! be read says why. Never an empty page that reads as "nothing ranks".

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use ronoctua_i18n::{Arg, key};
use ronoctua_player::wire::{Ask, State, Thing};
use ronoctua_tokens as tokens;
use ronoctua_video_metrics::read::{Catalog, Filter, Page, RankedVideo, Video};
use ronoctua_video_metrics::{VideoId, Why};

use crate::player::{Errand, PageId, Press, Target, View};
use crate::player_posters::{Shape, Tile};
use crate::player_rows::Builder;

/// How many videos a page of the ranking holds — the video catalogue's own
/// page, whose grid's pictures the renderer's frame is known to keep
/// (`video_metrics::PICTURES_MOST`).
pub const PER_PAGE: u32 = crate::video_metrics::PER_PAGE;

/// The command `⋯` is, and `e`: the video's own page.
pub const MORE: &str = "player-ranking-more";

/// The command the title's glyph is, and `L`: the other layout.
pub const LAYOUT: &str = "player-ranking-layout";

/// How the ranking is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// A row each video: its thumbnail at the head, its title, its channel
    /// and its score beside it.
    #[default]
    List,
    /// A grid of thumbnails, each with its title, channel and score under it.
    Grid,
}

impl Layout {
    /// The other one.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::List => Self::Grid,
            Self::Grid => Self::List,
        }
    }

    /// Its name's key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::List => key::PLAYER_CATALOG_LAYOUT_LIST,
            Self::Grid => key::PLAYER_RANKING_LAYOUT_GRID,
        }
    }

    /// Its glyph — the title's control says which layout it is now.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::List => tokens::icon::CATALOG_LAYOUT_LIST,
            Self::Grid => tokens::icon::CATALOG_LAYOUT_POSTERS,
        }
    }
}

/// The ranking's own keys (`Press::Ranking`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingKey {
    /// The page before (`<prior>`).
    Earlier,
    /// The page after (`<next>`).
    Later,
    /// The first page (`<home>`).
    First,
    /// The last page (`<end>`).
    Last,
    /// The other layout (`L`).
    Layout,
    /// The video under the cursor's own page (`e`, the `⋯`).
    More,
}

impl RankingKey {
    /// The key of command `id`, when it is one of the ranking's.
    #[must_use]
    pub fn of_command(id: &str) -> Option<Self> {
        Some(match id {
            "player-ranking-earlier" => Self::Earlier,
            "player-ranking-later" => Self::Later,
            "player-ranking-first" => Self::First,
            "player-ranking-last" => Self::Last,
            LAYOUT => Self::Layout,
            MORE => Self::More,
            _ => return None,
        })
    }
}

/// What a row of a video's own page does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankedAct {
    /// Played now, right after what plays; the queue stays.
    PlayNow,
    /// At the queue's end, once.
    Add,
    /// Watched in the Chrome kiosk.
    Kiosk,
    /// Downloaded, by the transfers' queue.
    Download,
    /// Hidden from the ranking.
    Hide,
    /// Deleted from the catalogue for good, after a question.
    Delete,
}

impl RankedAct {
    /// The six, in the page's order.
    pub const ALL: [Self; 6] = [
        Self::PlayNow,
        Self::Add,
        Self::Kiosk,
        Self::Download,
        Self::Hide,
        Self::Delete,
    ];

    /// Its row's label.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::PlayNow => key::PLAYER_RANKING_PLAY_NOW,
            Self::Add => key::PLAYER_RANKING_ADD,
            Self::Kiosk => key::PLAYER_RANKING_KIOSK,
            Self::Download => key::PLAYER_RANKING_DOWNLOAD,
            Self::Hide => key::PLAYER_RANKING_HIDE,
            Self::Delete => key::PLAYER_RANKING_DELETE,
        }
    }

    /// Its row's spot.
    #[must_use]
    pub const fn spot(self) -> &'static str {
        match self {
            Self::PlayNow => "ranked-play-now",
            Self::Add => "ranked-add",
            Self::Kiosk => "ranked-kiosk",
            Self::Download => "ranked-download",
            Self::Hide => "ranked-hide",
            Self::Delete => "ranked-delete",
        }
    }
}

/// What the reading thread is asked.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Want {
    /// A page of the ranking, from 0.
    Page(u32),
    /// One video, whole.
    Video(VideoId),
}

/// A page of the ranking, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// Which page, from 0.
    pub page: u32,
    /// Its videos, best first.
    pub rows: Vec<RankedVideo>,
    /// How many videos rank, on every page.
    pub total: u64,
}

/// What a reading came to.
#[derive(Debug, Clone)]
enum Got {
    /// A page.
    Page(Listed),
    /// One video, or none by that id.
    Video(Option<Box<Video>>),
}

/// The catalogue, as the reading thread holds it: opened once, again after
/// a reading failed, and `data_version` when the asks were last read.
struct Reader {
    path: PathBuf,
    catalog: Option<Catalog>,
    version: Option<u64>,
    /// What was last asked of each kind — read again when another
    /// connection commits.
    page: Option<u32>,
    video: Option<VideoId>,
}

impl Reader {
    const fn new(path: PathBuf) -> Self {
        Self {
            path,
            catalog: None,
            version: None,
            page: None,
            video: None,
        }
    }

    /// The asks of a turn — the last of each kind — and, on a stir, what
    /// was asked before of each kind not asked this turn, when another
    /// connection committed since: a commit that lands with an ask of the
    /// other kind is read too, or the page stayed as it was until the next.
    fn handle(&mut self, asked: Vec<Want>, stirred: bool) -> Vec<(Want, Result<Got, Why>)> {
        let mut wants: Vec<Want> = Vec::new();
        for want in asked {
            match &want {
                Want::Page(n) => self.page = Some(*n),
                Want::Video(id) => self.video = Some(id.clone()),
            }
            wants.retain(|w| std::mem::discriminant(w) != std::mem::discriminant(&want));
            wants.push(want);
        }
        if stirred && self.moved() {
            if !wants.iter().any(|w| matches!(w, Want::Page(_))) {
                wants.extend(self.page.map(Want::Page));
            }
            if !wants.iter().any(|w| matches!(w, Want::Video(_))) {
                wants.extend(self.video.clone().map(Want::Video));
            }
        }
        wants
            .into_iter()
            .map(|w| {
                let got = self.read(&w);
                (w, got)
            })
            .collect()
    }

    /// Whether another connection committed since the last reading — or
    /// the catalogue is not open: reading again is how a door hears that it
    /// opened, or why not.
    fn moved(&mut self) -> bool {
        match self.catalog.as_ref().map(Catalog::data_version) {
            Some(Ok(v)) => self.version != Some(v),
            _ => true,
        }
    }

    /// One reading.
    fn read(&mut self, want: &Want) -> Result<Got, Why> {
        if self.catalog.is_none() {
            self.catalog = Some(Catalog::open_ro(&self.path).map_err(|r| r.why)?);
        }
        let Some(c) = self.catalog.as_ref() else {
            return Err(Why::NotThere);
        };
        self.version = c.data_version().ok();
        let got = match want {
            Want::Page(n) => {
                let filter = Filter::default();
                c.ranked(
                    &filter,
                    Page {
                        number: *n,
                        per_page: PER_PAGE,
                    },
                )
                .and_then(|rows| {
                    c.count(&filter).map(|total| {
                        Got::Page(Listed {
                            page: *n,
                            rows,
                            total,
                        })
                    })
                })
            }
            Want::Video(id) => c.video(id).map(|v| Got::Video(v.map(Box::new))),
        };
        // A READING THAT FAILED is read again from a catalogue opened anew:
        // the file may have been replaced under the connection.
        got.map_err(|r| {
            self.catalog = None;
            r.why
        })
    }
}

/// What the thread and the door share.
#[derive(Default)]
struct Shared {
    asked: Vec<Want>,
    stirred: bool,
    answered: Vec<(Want, Result<Got, Why>)>,
    ended: bool,
}

/// The thread that reads the catalogue for this door, and the watch on its
/// commits. Dropped with the view, and the thread ends.
struct Worker {
    shared: Arc<(Mutex<Shared>, Condvar)>,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Worker")
    }
}

impl Worker {
    fn start(path: PathBuf, wake: crate::player_subtitles::Wake) -> Option<Self> {
        let shared: Arc<(Mutex<Shared>, Condvar)> = Arc::default();
        let theirs = shared.clone();
        let read = path.clone();
        let born = std::thread::Builder::new()
            .name("player-ranking".to_owned())
            .spawn(move || {
                let mut reader = Reader::new(read);
                let (lock, bell) = &*theirs;
                loop {
                    let (asked, stirred) = {
                        let mut s = lock.lock().unwrap_or_else(PoisonError::into_inner);
                        loop {
                            if s.ended {
                                return;
                            }
                            if !s.asked.is_empty() || s.stirred {
                                break (
                                    std::mem::take(&mut s.asked),
                                    std::mem::take(&mut s.stirred),
                                );
                            }
                            s = bell.wait(s).unwrap_or_else(PoisonError::into_inner);
                        }
                    };
                    let got = reader.handle(asked, stirred);
                    if got.is_empty() {
                        continue;
                    }
                    lock.lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .answered
                        .extend(got);
                    wake.call();
                }
            });
        if let Err(e) = born {
            tracing::warn!(error = %e, "could not start the thread that reads the ranking");
            return None;
        }
        let watch = watched(&path, shared.clone());
        Some(Self {
            shared,
            _watch: watch,
        })
    }

    fn ask(&self, want: Want) {
        let (lock, bell) = &*self.shared;
        lock.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .asked
            .push(want);
        bell.notify_all();
    }

    fn take(&self) -> Vec<(Want, Result<Got, Why>)> {
        std::mem::take(
            &mut self
                .shared
                .0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .answered,
        )
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let (lock, bell) = &*self.shared;
        lock.lock().unwrap_or_else(PoisonError::into_inner).ended = true;
        bell.notify_all();
    }
}

/// The watch on the catalogue's commits: `<catalogue>-wal`, in its folder —
/// made first, as the video catalogue's door makes it: a watch on a folder
/// that does not exist never fires, and the first scan's commit is what a
/// page that says "nothing yet" waits for.
fn watched(
    catalogue: &Path,
    shared: Arc<(Mutex<Shared>, Condvar)>,
) -> Option<ronoctua_files::notice::Notice> {
    let dir = catalogue.parent()?;
    let name = format!("{}-wal", catalogue.file_name()?.to_string_lossy());
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(error = %e, "no folder for the video catalogue; the ranking does not follow it");
        return None;
    }
    ronoctua_files::notice::Notice::on_file(
        dir,
        &name,
        ronoctua_files::notice::Pace::Coalesced,
        move || {
            let (lock, bell) = &*shared;
            let mut s = lock.lock().unwrap_or_else(PoisonError::into_inner);
            s.stirred = true;
            let going = !s.ended;
            drop(s);
            bell.notify_all();
            going
        },
    )
    .map_err(|e| tracing::warn!(error = %e, "the video catalogue's commits are not watched"))
    .ok()
}

/// One video read whole — `None` when the catalogue has none by its id —
/// or why it was not read.
type Whole = Result<Option<Box<Video>>, Why>;

/// The ranking as this view holds it.
#[derive(Debug, Default)]
pub struct Ranking {
    /// The catalogue: this machine's, or a photograph's; `None` with no home
    /// to keep one in.
    path: Option<PathBuf>,
    /// Which page, from 0.
    pub(crate) page: u32,
    /// How it is drawn.
    pub(crate) layout: Layout,
    /// The video whose own page is open.
    pub(crate) chosen: Option<VideoId>,
    /// The last page read, or why it was not.
    listed: Option<Result<Listed, Why>>,
    /// The page `listed` answers — read, or REFUSED: a refusal is an answer
    /// too, held until the catalogue moves (the thread's watch reads it
    /// again) or another page is asked. Asked again on every settle, a
    /// machine with no catalogue read, woke and repainted without end.
    answered: Option<u32>,
    /// The page held was changed by a write of this door's, and is read
    /// again at the next look (the video read whole is dropped instead).
    stale: bool,
    /// The last video read, or why it was not.
    video: Option<(VideoId, Whole)>,
    /// What was asked and not answered yet — asked once.
    asked_page: Option<u32>,
    asked_video: Option<VideoId>,
    /// The reading thread, once a door with a loop asked.
    worker: Option<Worker>,
    /// The thumbnails of what is on screen, decoded off the loop.
    pub(crate) posters: crate::player_posters::Posters,
    /// How the video catalogue is opened, as the leader teaches it — read
    /// once, for the sentence a machine with no catalogue says.
    opens_with: Option<String>,
    /// What the rows were handed and not read: a test's.
    given: bool,
    /// The spot the cursor goes back to at the next settle — the row that
    /// opened a video's page this door closed when the video went.
    follow_owed: Option<String>,
}

impl Ranking {
    /// This machine's catalogue.
    #[must_use]
    pub fn of_this_machine() -> Self {
        Self::at(ronoctua_video_metrics::default_path())
    }

    /// The catalogue at `path` — a photograph's, a test's.
    #[must_use]
    pub fn at(path: Option<PathBuf>) -> Self {
        Self {
            path,
            posters: crate::player_posters::Posters::at(None),
            ..Self::default()
        }
    }

    /// A page HANDED to the view, read from nowhere — a test's.
    #[cfg(test)]
    pub fn given(&mut self, rows: Vec<RankedVideo>, total: u64) {
        self.given = true;
        self.answered = Some(self.page);
        self.listed = Some(Ok(Listed {
            page: self.page,
            rows,
            total,
        }));
    }

    /// How many pages the ranking takes, at least one.
    fn pages(&self) -> u32 {
        match &self.listed {
            Some(Ok(l)) => u32::try_from(l.total.div_ceil(u64::from(PER_PAGE)))
                .unwrap_or(u32::MAX)
                .max(1),
            _ => 1,
        }
    }

    /// The page read, when it is the one asked.
    fn listed(&self) -> Option<&Result<Listed, Why>> {
        self.listed.as_ref()
    }

    /// A video of the page, or the one read whole — whichever holds it.
    fn row_of(&self, id: &VideoId) -> Option<&RankedVideo> {
        let whole = self
            .video
            .as_ref()
            .filter(|(v, _)| v == id)
            .and_then(|(_, got)| got.as_ref().ok()?.as_ref())
            .map(|v| &v.row);
        whole.or_else(|| match &self.listed {
            Some(Ok(l)) => l.rows.iter().find(|r| &r.id == id),
            _ => None,
        })
    }

    /// Everything shown is read again at the next look: a write of this
    /// door's was done. What is held stays on screen until the new reading
    /// comes — the page does not blink to "reading".
    fn forget(&mut self) {
        self.asked_page = None;
        self.asked_video = None;
        self.video = None;
        self.stale = true;
    }

    /// Whether page `n` is held, read or refused, and not stale.
    fn holds(&self, n: u32) -> bool {
        !self.stale && self.answered == Some(n)
    }

    /// An answer kept: a page past the last — the ranking shrank under it —
    /// goes back to the last one, and is asked for.
    fn keep(&mut self, want: Want, got: Result<Got, Why>) {
        match (want, got) {
            (Want::Page(n), Ok(Got::Page(l))) => {
                if self.asked_page == Some(n) {
                    self.asked_page = None;
                }
                let last = u32::try_from(l.total.div_ceil(u64::from(PER_PAGE)))
                    .unwrap_or(u32::MAX)
                    .saturating_sub(1);
                let shrank = l.rows.is_empty() && n > last;
                self.listed = Some(Ok(l));
                self.answered = Some(n);
                self.stale = false;
                if shrank && self.page == n {
                    self.page = last;
                }
            }
            (Want::Page(n), Err(why)) => {
                if self.asked_page == Some(n) {
                    self.asked_page = None;
                }
                self.listed = Some(Err(why));
                self.answered = Some(n);
                self.stale = false;
            }
            (Want::Video(id), got) => {
                if self.asked_video.as_ref() == Some(&id) {
                    self.asked_video = None;
                }
                let got = match got {
                    Ok(Got::Video(v)) => Ok(v),
                    Ok(Got::Page(_)) => return,
                    Err(why) => Err(why),
                };
                self.video = Some((id, got));
            }
            (Want::Page(_), Ok(Got::Video(_))) => {}
        }
    }

    /// `want` asked — of the thread, or read in place with no loop.
    fn ask(&mut self, want: Want, wake: Option<&crate::player_subtitles::Wake>) {
        let Some(path) = self.path.clone() else {
            let got = Err(Why::NotThere);
            self.keep(want, got);
            return;
        };
        match &want {
            Want::Page(n) => self.asked_page = Some(*n),
            Want::Video(id) => self.asked_video = Some(id.clone()),
        }
        match wake {
            Some(wake) => {
                if self.worker.is_none() {
                    self.worker = Worker::start(path, wake.clone());
                }
                match &self.worker {
                    Some(w) => w.ask(want),
                    None => self.keep(want, Err(Why::Unreadable)),
                }
            }
            None => {
                let mut reader = Reader::new(path);
                let mut got = reader.handle(vec![want], false);
                if let Some((want, got)) = got.pop() {
                    self.keep(want, got);
                }
            }
        }
    }
}

/// A WRITE OR AN ERRAND for a video, carried out off the loop
/// (`player::carry_out`, [`carry`]) and said when it is done
/// (`Told::Ranking`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Hidden from the ranking: it stays catalogued.
    Hide {
        /// The catalogue.
        catalogue: PathBuf,
        /// The video.
        id: VideoId,
        /// Its title, for the sentence.
        title: String,
    },
    /// Deleted for good: its tombstone keeps it from coming back.
    Delete {
        /// The catalogue.
        catalogue: PathBuf,
        /// The video.
        id: VideoId,
        /// Its title, for the sentence.
        title: String,
    },
    /// Downloaded by the transfers' queue, at its best, into the person's
    /// downloads.
    Download {
        /// Its address.
        url: String,
        /// Its title, for the sentence.
        title: String,
    },
}

/// What an [`Act`] came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Hidden, or why not.
    Hidden(VideoId, String, Result<(), Why>),
    /// Deleted, or why not.
    Deleted(VideoId, String, Result<(), Why>),
    /// Handed to the transfers' queue — `taken` — and how the queue is
    /// opened, as the leader teaches it.
    Downloading {
        /// Its title.
        title: String,
        /// The queue took it.
        taken: bool,
        /// `LEADER t t`, in the leader's words.
        keys: Option<String>,
    },
}

impl Act {
    /// What it comes to when it is done — a photograph's, which does
    /// nothing.
    #[must_use]
    pub fn done(&self) -> Outcome {
        match self {
            Self::Hide { id, title, .. } => Outcome::Hidden(id.clone(), title.clone(), Ok(())),
            Self::Delete { id, title, .. } => Outcome::Deleted(id.clone(), title.clone(), Ok(())),
            Self::Download { title, .. } => Outcome::Downloading {
                title: title.clone(),
                taken: true,
                keys: None,
            },
        }
    }
}

/// `act`, carried out — on a thread of its own: a write waits for a scan
/// that holds the catalogue (`busy_timeout`), and the transfers' socket for
/// its daemon.
#[must_use]
pub fn carry(act: Act) -> Outcome {
    match act {
        Act::Hide {
            catalogue,
            id,
            title,
        } => {
            let done = Catalog::open_ro(&catalogue)
                .and_then(|c| c.hide(&id))
                .map_err(|r| r.why);
            Outcome::Hidden(id, title, done)
        }
        Act::Delete {
            catalogue,
            id,
            title,
        } => {
            let done = Catalog::open_ro(&catalogue)
                .and_then(|c| c.delete(&id))
                .map_err(|r| r.why);
            Outcome::Deleted(id, title, done)
        }
        Act::Download { url, title } => {
            use ronoctua_transfers::where_to::{Wish, landing, work_for};
            let work = work_for(
                &url,
                landing(),
                Wish::media(ronoctua_transfers::Want::Video),
            );
            let taken =
                match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
                    Ok(()) => true,
                    Err(e) => {
                        tracing::warn!(error = %e, "the transfer queue did not take the download");
                        false
                    }
                };
            // THE KEYS THE SENTENCE TEACHES, from the leader's own trees:
            // this may be a window, a process of its own.
            let keys = crate::leader::default_of()
                .ok()
                .and_then(|(trees, _)| crate::leader::taught_keys(&trees, "transfers-open", None))
                .map(|k| format!("{} {k}", crate::leader()));
            Outcome::Downloading { title, taken, keys }
        }
    }
}

/// What a door says of `outcome`.
#[must_use]
pub fn said(outcome: &Outcome, texts: &ronoctua_i18n::Texts) -> String {
    let refused = |title: &str, why: Why| {
        texts.t_with(
            key::PLAYER_RANKING_NOT_CHANGED,
            &[
                ("title", title),
                ("why", &texts.t(crate::video_metrics_words::why_key(why))),
            ],
        )
    };
    match outcome {
        Outcome::Hidden(_, title, Ok(())) => {
            texts.t_with(key::PLAYER_RANKING_HIDDEN, &[("title", title)])
        }
        Outcome::Deleted(_, title, Ok(())) => {
            texts.t_with(key::PLAYER_RANKING_DELETED, &[("title", title)])
        }
        Outcome::Hidden(_, title, Err(why)) | Outcome::Deleted(_, title, Err(why)) => {
            refused(title, *why)
        }
        Outcome::Downloading {
            title,
            taken: true,
            keys: Some(keys),
        } => texts.t_with(
            key::PLAYER_RANKING_DOWNLOADING,
            &[("title", title), ("keys", keys)],
        ),
        Outcome::Downloading {
            title,
            taken: true,
            keys: None,
        } => texts.t_with(key::PLAYER_RANKING_DOWNLOADING_PLAIN, &[("title", title)]),
        Outcome::Downloading {
            title,
            taken: false,
            ..
        } => texts.t_with(key::PLAYER_RANKING_NOT_DOWNLOADED, &[("title", title)]),
    }
}

/// What putting `v` in the queue asks — at its end, ONCE, or now — titled
/// from the start, and the sentence it says.
#[must_use]
pub fn put(v: &RankedVideo, play: bool, texts: &ronoctua_i18n::Texts) -> Errand {
    Errand::TellSaying(
        Ask::Put {
            what: vec![Thing::Named {
                location: v.url.clone(),
                title: v.title.clone(),
            }],
            after: None,
            into: None,
            play,
            replace: false,
            // AT THE QUEUE'S END, ONCE: one already there is refused, and the
            // view asks whether to add it again (`player::overruled`). Played
            // now, it plays.
            once: !play,
        },
        if play {
            texts.t_with(key::PLAYER_PLAYING_NOW, &[("name", &v.title)])
        } else {
            texts.t_with(key::PLAYER_RANKING_ADDED, &[("title", &v.title)])
        },
    )
}

/// A row's spot: the video's id — the cursor keeps it when the ranking
/// reorders under it.
#[must_use]
pub fn spot_of(id: &VideoId) -> String {
    format!("ranked-{}", id.0)
}

/// What is said under a video's title, each line in its parts
/// (`player_posters::Tile::under`). In a ROW: its channel and its score,
/// then when it was published and how long it runs — each line's parts on
/// one line where they fit, else a line each. In a TILE of the grid, a box
/// too narrow for both on a line: its score first — what the ranking is by,
/// never cut — and its channel under it.
fn under(
    v: &RankedVideo,
    layout: Layout,
    now_s: i64,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Vec<String>> {
    let points = crate::video_metrics_words::points(v.score, texts);
    if layout == Layout::Grid {
        return vec![vec![points], vec![v.channel.clone()]];
    }
    let when: Vec<String> = v
        .published_at_s
        .map(|p| crate::video_metrics_words::age(p, now_s, texts))
        .into_iter()
        .chain(v.duration_s.map(crate::video_metrics_words::length))
        .collect();
    let mut lines = vec![vec![v.channel.clone(), points]];
    if !when.is_empty() {
        lines.push(when);
    }
    lines
}

/// Now, in seconds since the epoch — an age is counted from it.
fn now_s() -> i64 {
    crate::video_metrics::now_s()
}

impl View {
    /// The catalogue the ranking reads is `path` — a photograph's, a
    /// test's.
    pub fn ranking_at(&mut self, path: PathBuf) {
        self.ranking = Ranking::at(Some(path));
    }

    /// WHAT THE RANKING'S PAGES READ, on each settle: the answers that came
    /// back — the cursor kept on its video, wherever the ranking moved it —
    /// the page or the video on screen asked when it is not the one held,
    /// and the thumbnails of what is shown decoded.
    pub(crate) fn ranking_look(
        &mut self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let page = self.page();
        if !matches!(page, PageId::Ranking | PageId::RankedVideo) {
            return;
        }
        if self.ranking.opens_with.is_none() {
            self.ranking.opens_with = Some(
                crate::leader::default_of()
                    .ok()
                    .and_then(|(trees, _)| {
                        crate::leader::taught_keys(&trees, "video-metrics-open", None)
                    })
                    .map(|k| format!("{} {k}", crate::leader()))
                    .unwrap_or_default(),
            );
        }
        let came = self
            .ranking
            .worker
            .as_ref()
            .map_or_else(Vec::new, Worker::take);
        if let Some(spot) = self.ranking.follow_owed.take() {
            self.ranking_follow(&spot, state, columns, texts);
            self.top_mut().moved = true;
        }
        if !came.is_empty() {
            let here = self.ranking_here(state, columns, texts);
            for (want, got) in came {
                self.ranking.keep(want, got);
            }
            if let Some(here) = here {
                self.ranking_follow(&here, state, columns, texts);
            }
        }
        if self.ranking.given {
            return;
        }
        let wake = self.wake.clone();
        let n = self.ranking.page;
        // A PAGE ANSWERED IS HELD, refused too: it is read again when the
        // catalogue moves — the thread's watch — or another page is asked.
        let held = self.ranking.holds(n);
        if page == PageId::Ranking && !held && self.ranking.asked_page != Some(n) {
            self.ranking.ask(Want::Page(n), wake.as_ref());
        }
        if page == PageId::RankedVideo
            && let Some(id) = self.ranking.chosen.clone()
        {
            let held = self.ranking.video.as_ref().is_some_and(|(v, _)| *v == id);
            if !held && self.ranking.asked_video.as_ref() != Some(&id) {
                self.ranking.ask(Want::Video(id), wake.as_ref());
            }
        }
        // THE THUMBNAILS OF WHAT IS SHOWN — the page's, or the video's own.
        let files: Vec<PathBuf> = match page {
            PageId::Ranking => match &self.ranking.listed {
                Some(Ok(l)) => l.rows.iter().filter_map(|r| r.thumb_file.clone()).collect(),
                _ => Vec::new(),
            },
            _ => self
                .ranking
                .chosen
                .as_ref()
                .and_then(|id| self.ranking.row_of(id))
                .and_then(|r| r.thumb_file.clone())
                .into_iter()
                .collect(),
        };
        self.ranking.posters.look_files(&files, wake.as_ref());
    }

    /// WHERE THE CURSOR IS, by the video's id: on a row of the list its
    /// spot, and on a shelf of the grid the spot of the cell it is on — the
    /// shelf's is its first video's, and a ranking that reordered under it
    /// put another video in that cell.
    fn ranking_here(
        &self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<String> {
        let rows = self.rows(state, columns, texts);
        let row = rows.get(self.top().cursor)?;
        let grid: Vec<&crate::player_rows::Cell> =
            row.cells.iter().filter(|c| !c.control).collect();
        let spot = if grid.is_empty() {
            row.spot.clone()
        } else {
            grid.get(self.top().cell.min(grid.len() - 1))?.spot.clone()
        };
        Some(spot).filter(|s| !s.is_empty())
    }

    /// The cursor on `spot` — a row's, or a cell's of a shelf, cursor and
    /// cell both — wherever the rows put it now ([`Self::ranking_here`]).
    fn ranking_follow(
        &mut self,
        spot: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let rows = self.rows(state, columns, texts);
        let cell = rows.iter().enumerate().find_map(|(n, r)| {
            r.cells
                .iter()
                .filter(|c| !c.control)
                .position(|c| c.spot == spot)
                .map(|k| (n, k))
        });
        match cell {
            Some((n, k)) if !rows.iter().any(|r| r.spot == spot) => {
                let top = self.top_mut();
                top.cursor = n;
                top.cell = k;
            }
            _ => self.follow(spot, state, columns, texts),
        }
    }

    /// The ranking's title, its layout's glyph beside it.
    pub(crate) fn ranking_title(&self, b: &mut Builder<'_>) {
        b.heading_controls_said(
            b.t(key::PLAYER_PAGE_RANKING),
            &[(self.ranking.layout.glyph(), Target::RankingLayout, LAYOUT)],
        );
    }

    /// Why there is no ranking to show, in a sentence: no catalogue yet —
    /// and where channels are added — or why it cannot be read.
    fn ranking_refused(&self, why: Why, b: &mut Builder<'_>) {
        let said = match why {
            Why::NotThere => b.texts.t_with(
                key::PLAYER_RANKING_NONE,
                &[(
                    "keys",
                    self.ranking.opens_with.as_deref().unwrap_or_default(),
                )],
            ),
            other => b.t(crate::video_metrics_words::why_key(other)),
        };
        crate::player_find::note_said(&said, b);
    }

    /// The ranking's rows, under its title: the page, the layout, and the
    /// videos of the page — a row each or a grid.
    pub(crate) fn ranking_rows(&self, b: &mut Builder<'_>) {
        let listed = match self.ranking.listed() {
            None => {
                b.note(key::PLAYER_RANKING_READING);
                return;
            }
            Some(Err(why)) => {
                self.ranking_refused(*why, b);
                return;
            }
            Some(Ok(l)) => l,
        };
        if listed.total == 0 {
            b.note(key::PLAYER_RANKING_EMPTY);
            return;
        }
        let n = |x: u64| Arg::Number(i64::try_from(x).unwrap_or(i64::MAX));
        b.pair(
            b.t(key::PLAYER_CATALOG_PAGE),
            b.texts.t_args(
                key::PLAYER_CATALOG_PAGE_OF,
                &[
                    ("page", Arg::Number(i64::from(listed.page) + 1)),
                    ("pages", Arg::Number(i64::from(self.ranking.pages()))),
                ],
            ),
            Target::RankingPage,
            "ranking-page",
        );
        b.pair(
            b.t(key::PLAYER_CATALOG_LAYOUT),
            b.t(self.ranking.layout.key()),
            Target::RankingLayout,
            "ranking-layout",
        );
        let from = u64::from(listed.page) * u64::from(PER_PAGE) + 1;
        let to = from + u64::try_from(listed.rows.len()).unwrap_or(0);
        b.heading_said(b.texts.t_args(
            key::PLAYER_CATALOG_SHOWN,
            &[
                ("from", n(from)),
                ("to", n(to.saturating_sub(1).max(from))),
                ("total", n(listed.total)),
            ],
        ));
        let now = now_s();
        let tiles: Vec<Tile> = listed
            .rows
            .iter()
            .map(|v| Tile {
                picture: v
                    .thumb_file
                    .as_deref()
                    .and_then(|f| self.ranking.posters.of_file(f)),
                glyph: tokens::icon::PLAYER_LANE_PICTURE,
                title: v.title.clone(),
                under: under(v, self.ranking.layout, now, b.texts),
                target: Target::Ranked(v.id.clone()),
                spot: spot_of(&v.id),
                more: Some((Target::RankedMore(v.id.clone()), MORE)),
            })
            .collect();
        let shape = Shape {
            cells: tokens::space::VIDEOMETRICS_THUMBCELLS,
            lines: tokens::space::VIDEOMETRICS_THUMBLINES,
            titles: tokens::space::VIDEOMETRICS_TITLELINES,
        };
        match self.ranking.layout {
            Layout::List => crate::player_posters::strip(&tiles, shape, b),
            Layout::Grid => crate::player_posters::tiled(&tiles, shape, b),
        }
    }

    /// A video's own page: its thumbnail, what it is, and what can be done
    /// with it.
    pub(crate) fn ranked_video_rows(&self, b: &mut Builder<'_>) {
        let Some(id) = self.ranking.chosen.as_ref() else {
            b.note(key::PLAYER_PAGE_GONE);
            return;
        };
        let whole = self.ranking.video.as_ref().filter(|(v, _)| v == id);
        if let Some((_, Err(why))) = whole {
            self.ranking_refused(*why, b);
            return;
        }
        if let Some((_, Ok(None))) = whole {
            b.note(crate::video_metrics_words::why_key(Why::NotFound));
            return;
        }
        let Some(v) = self.ranking.row_of(id) else {
            b.note(key::PLAYER_RANKING_READING);
            return;
        };
        if let Some((key, size)) = v
            .thumb_file
            .as_deref()
            .and_then(|f| self.ranking.posters.of_file(f))
        {
            b.cover(&key, size);
        }
        let fact = |b: &mut Builder<'_>, label: &str, value: String, spot: &str| {
            b.pair_whole(b.t(label), value, Target::Nothing, spot);
        };
        fact(
            b,
            key::PLAYER_RANKING_TITLE,
            v.title.clone(),
            "ranked-title",
        );
        fact(
            b,
            key::PLAYER_RANKING_CHANNEL,
            v.channel.clone(),
            "ranked-channel",
        );
        fact(
            b,
            key::PLAYER_RANKING_SCORE,
            crate::video_metrics_words::points(v.score, b.texts),
            "ranked-score",
        );
        if let Some(s) = v.duration_s {
            fact(
                b,
                key::PLAYER_RANKING_LENGTH,
                crate::video_metrics_words::length(s),
                "ranked-length",
            );
        }
        if let Some(p) = v.published_at_s {
            fact(
                b,
                key::PLAYER_RANKING_PUBLISHED,
                crate::video_metrics_words::age(p, now_s(), b.texts),
                "ranked-published",
            );
        }
        b.heading(key::PLAYER_RANKING_ACTIONS);
        for act in RankedAct::ALL {
            b.pair(
                b.t(act.key()),
                String::new(),
                Target::RankedDo(act),
                act.spot(),
            );
        }
    }

    /// A video's own page opened, over the ranking.
    fn open_ranked(
        &mut self,
        id: &VideoId,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let from = self.spot_here(state, columns, texts);
        self.ranking.chosen = Some(id.clone());
        self.ranking.video = None;
        self.push(PageId::RankedVideo, from);
        self.settle_in(state, columns, texts);
        // THE CURSOR ON WHAT IS DONE MOST — playing it — and not on the facts
        // above it, which `RET` does nothing on.
        self.follow(RankedAct::PlayNow.spot(), state, columns, texts);
    }

    /// The ranking's own keys.
    pub(crate) fn ranking_pressed(
        &mut self,
        key: RankingKey,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        if self.page() != PageId::Ranking {
            return Errand::Nothing;
        }
        let last = self.ranking.pages().saturating_sub(1);
        let r = &mut self.ranking;
        match key {
            RankingKey::Earlier => r.page = r.page.saturating_sub(1),
            RankingKey::Later => r.page = (r.page + 1).min(last),
            RankingKey::First => r.page = 0,
            RankingKey::Last => r.page = last,
            RankingKey::Layout => r.layout = r.layout.next(),
            RankingKey::More => {
                let target = self
                    .rows(state, columns, texts)
                    .get(self.cursor())
                    .map(|r| r.target.clone());
                if let Some(Target::Ranked(id)) = target {
                    self.open_ranked(&id, state, columns, texts);
                }
                return Errand::Nothing;
            }
        }
        // A NEW PAGE STARTS AT ITS TOP: the cursor's spot is on another page.
        if matches!(
            key,
            RankingKey::Earlier | RankingKey::Later | RankingKey::First | RankingKey::Last
        ) {
            self.follow("ranking-page", state, columns, texts);
        }
        self.settle_in(state, columns, texts);
        Errand::Nothing
    }

    /// What a press does on a row of the ranking's pages — `None` for a
    /// target that is none of theirs.
    pub(crate) fn ranking_turned(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        let errand = match (target, press) {
            (Target::Ranked(id), Press::Enter | Press::Add | Press::PlayNow) => {
                let v = self.ranking.row_of(id)?;
                put(v, press == Press::PlayNow, texts)
            }
            (Target::RankedMore(id), Press::Enter) => {
                let id = id.clone();
                self.open_ranked(&id, state, columns, texts);
                Errand::Nothing
            }
            (Target::RankingLayout, Press::Enter | Press::Less | Press::More) => {
                return Some(self.ranking_pressed(RankingKey::Layout, state, columns, texts));
            }
            (Target::RankingPage, Press::Less) => {
                return Some(self.ranking_pressed(RankingKey::Earlier, state, columns, texts));
            }
            // `RET` ON THE PAGE goes on, and from the last back to the first.
            (Target::RankingPage, Press::Enter | Press::More) => {
                let key = if press == Press::Enter && self.ranking.page + 1 >= self.ranking.pages()
                {
                    RankingKey::First
                } else {
                    RankingKey::Later
                };
                return Some(self.ranking_pressed(key, state, columns, texts));
            }
            (Target::RankedDo(act), Press::Enter) => self.ranked_do(*act, state, columns, texts),
            _ => return None,
        };
        Some(errand)
    }

    /// A row of a video's own page, pressed.
    fn ranked_do(
        &mut self,
        act: RankedAct,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(id) = self.ranking.chosen.clone() else {
            return Errand::Nothing;
        };
        let Some(v) = self.ranking.row_of(&id).cloned() else {
            return Errand::Said(texts.t(crate::video_metrics_words::why_key(Why::NotFound)));
        };
        let catalogue = self.ranking.path.clone();
        match act {
            RankedAct::PlayNow | RankedAct::Add => put(&v, act == RankedAct::PlayNow, texts),
            RankedAct::Kiosk => Errand::Kiosk(v.url),
            RankedAct::Download => Errand::Ranking(Act::Download {
                url: v.url,
                title: v.title,
            }),
            RankedAct::Hide | RankedAct::Delete => {
                let Some(catalogue) = catalogue else {
                    return Errand::Said(
                        texts.t(crate::video_metrics_words::why_key(Why::NotThere)),
                    );
                };
                if act == RankedAct::Hide {
                    return Errand::Ranking(Act::Hide {
                        catalogue,
                        id,
                        title: v.title,
                    });
                }
                let _ = (state, columns);
                // FOR GOOD, after a question — "no" first: a deleted video
                // never comes back.
                let sentence = texts.t_with(key::ASK_PLAYER_RANKING_DELETE, &[("title", &v.title)]);
                self.ask_about(
                    sentence,
                    Errand::Ranking(Act::Delete {
                        catalogue,
                        id,
                        title: v.title,
                    }),
                )
            }
        }
    }

    /// What a hide or a delete came to, for the view: done, the video's own
    /// page closes — it is no longer in the ranking — the cursor goes back
    /// to the row that opened it at the next settle, and the ranking is read
    /// again. `true` when the rows changed.
    pub(crate) fn ranking_told(&mut self, outcome: &Outcome) -> bool {
        let gone = match outcome {
            Outcome::Hidden(id, _, Ok(())) | Outcome::Deleted(id, _, Ok(())) => id,
            _ => return false,
        };
        self.ranking.forget();
        if self.page() == PageId::RankedVideo
            && self.ranking.chosen.as_ref() == Some(gone)
            && self.levels.len() > 1
            && let Some(left) = self.levels.pop()
        {
            self.ranking.follow_owed = left.opened_from;
        }
        true
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::player::fixture;
    use ronoctua_video_metrics::ChannelId;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a language")
    }

    /// A video of the ranking, `n` best.
    pub(crate) fn video(n: u32) -> RankedVideo {
        RankedVideo {
            id: VideoId(format!("video{n:06}")),
            url: format!("https://www.youtube.com/watch?v=video{n:06}"),
            title: format!("A video worth watching, number {n}"),
            channel: format!("Channel {}", n % 3),
            channel_id: ChannelId(format!("UCchannel{}", n % 3)),
            score: 9000 - i64::from(n) * 100,
            thumb_file: None,
            duration_s: Some(600 + n),
            published_at_s: Some(1_700_000_000),
            is_short: false,
            hidden: false,
        }
    }

    /// A view on the ranking, handed `count` videos.
    fn on_the_ranking(count: u32) -> View {
        let mut v = View::blank(None);
        v.ranking = Ranking::at(None);
        v.ranking
            .given((0..count).map(video).collect(), u64::from(count));
        v.push(PageId::Ranking, None);
        v
    }

    /// Every word the rows draw, a line each.
    fn said_on(rows: &[crate::player_rows::Row]) -> String {
        rows.iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn row_at(rows: &[crate::player_rows::Row], spot: &str) -> usize {
        rows.iter()
            .position(|r| r.spot == spot)
            .unwrap_or_else(|| panic!("no row {spot}"))
    }

    /// FAILS IF: `RET` on a video stops putting it at the queue's end,
    /// TITLED and ONCE, saying "Added: …" — or `P` stops playing it now, or
    /// the refusal of one queued already stops being the person's question,
    /// "no" first, whose "yes" adds it again.
    #[test]
    fn a_video_goes_in_the_queue_titled_and_once() {
        let t = texts("en-GB");
        let s = fixture::idle();
        let mut v = on_the_ranking(3);
        let rows = v.rows(Some(&s), 100, &t);
        let at = row_at(&rows, &spot_of(&video(1).id));
        v.follow(&spot_of(&video(1).id), Some(&s), 100, &t);
        assert_eq!(v.cursor(), at);
        let Errand::TellSaying(ask, said) = v.key(Press::Enter, Some(&s), 100, &t) else {
            panic!("RET did not tell the player");
        };
        let Ask::Put {
            what, play, once, ..
        } = &ask
        else {
            panic!("{ask:?}");
        };
        assert_eq!(
            what,
            &[Thing::Named {
                location: video(1).url,
                title: video(1).title,
            }]
        );
        assert!(!play && *once, "queued at the end, once");
        assert_eq!(
            said,
            t.t_with(key::PLAYER_RANKING_ADDED, &[("title", &video(1).title)])
        );
        let Errand::TellSaying(Ask::Put { play, once, .. }, _) =
            v.key(Press::PlayNow, Some(&s), 100, &t)
        else {
            panic!("P did not play it");
        };
        assert!(play && !once, "played now");
        // THE QUEUE HAS IT: the player's refusal is the person's to overrule.
        let trouble = ronoctua_player::wire::Trouble::AlreadyQueued {
            entry: ronoctua_player::queue::EntryId(4),
            name: ronoctua_player::queue::Name::Titled(video(1).title),
        };
        let again = crate::player::overruled(&ask, &trouble).expect("an ask that overrules it");
        assert!(v.told(
            &crate::player::Told::Asks {
                again: Box::new(again.clone()),
                trouble,
            },
            &t
        ));
        v.settle_in(Some(&s), 100, &t);
        let rows = v.rows(Some(&s), 100, &t);
        assert!(
            rows.get(v.cursor()).is_some_and(|r| r.spot == "no"),
            "the question was not born on its no"
        );
        v.key(Press::Down, Some(&s), 100, &t);
        assert!(
            matches!(
                v.key(Press::Enter, Some(&s), 100, &t),
                Errand::TellSaying(a, _) if a == again
            ),
            "yes did not add it again"
        );
    }

    /// FAILS IF: the `⋯` of a row — or `e` — stops opening the video's own
    /// page, or that page stops offering the six things it does, or delete
    /// stops being asked "no" first, or hide asks — or `Esc` stops going back
    /// to the video's row.
    #[test]
    fn the_more_control_opens_the_video_s_page() {
        let t = texts("en-GB");
        let s = fixture::idle();
        let mut v = on_the_ranking(4);
        let rows = v.rows(Some(&s), 100, &t);
        let spot = spot_of(&video(2).id);
        let at = row_at(&rows, &spot);
        let more = rows[at]
            .cells
            .iter()
            .find(|c| c.control)
            .expect("a ⋯ at the end of the row");
        assert_eq!(more.about, Some(MORE));
        assert!(v.clicked_at(at, more.from, &rows), "the ⋯ did not act");
        v.key(Press::Enter, Some(&s), 100, &t);
        assert_eq!(v.page(), PageId::RankedVideo);
        assert_eq!(v.ranking.chosen, Some(video(2).id));
        let rows = v.rows(Some(&s), 100, &t);
        assert_eq!(
            rows.get(v.cursor()).map(|r| r.spot.as_str()),
            Some(RankedAct::PlayNow.spot()),
            "the page did not open on playing it"
        );
        for act in RankedAct::ALL {
            row_at(&rows, act.spot());
        }
        // HIDE ACTS AT ONCE; DELETE ASKS.
        v.follow(RankedAct::Hide.spot(), Some(&s), 100, &t);
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 100, &t),
            Errand::Said(_)
        ));
        let mut v = on_the_ranking(4);
        v.ranking.path = Some(PathBuf::from("/nowhere/catalog.sqlite"));
        v.follow(&spot, Some(&s), 100, &t);
        assert_eq!(
            v.key(Press::Ranking(RankingKey::More), Some(&s), 100, &t),
            Errand::Nothing
        );
        assert_eq!(v.page(), PageId::RankedVideo, "e did not open it");
        v.follow(RankedAct::Hide.spot(), Some(&s), 100, &t);
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 100, &t),
            Errand::Ranking(Act::Hide { .. })
        ));
        v.follow(RankedAct::Delete.spot(), Some(&s), 100, &t);
        assert_eq!(v.key(Press::Enter, Some(&s), 100, &t), Errand::Nothing);
        let rows = v.rows(Some(&s), 100, &t);
        assert_eq!(rows.get(v.cursor()).map(|r| r.spot.as_str()), Some("no"));
        v.key(Press::Down, Some(&s), 100, &t);
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 100, &t),
            Errand::Ranking(Act::Delete { .. })
        ));
        // DELETED: the page goes, back to the ranking.
        assert!(v.ranking_told(&Outcome::Deleted(video(2).id, video(2).title, Ok(()))));
        assert_eq!(v.page(), PageId::Ranking);
        // AND THE KIOSK AND THE DOWNLOAD are errands with the video's address.
        let mut v = on_the_ranking(4);
        v.follow(&spot, Some(&s), 100, &t);
        v.key(Press::Ranking(RankingKey::More), Some(&s), 100, &t);
        v.follow(RankedAct::Kiosk.spot(), Some(&s), 100, &t);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 100, &t),
            Errand::Kiosk(video(2).url)
        );
        v.follow(RankedAct::Download.spot(), Some(&s), 100, &t);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 100, &t),
            Errand::Ranking(Act::Download {
                url: video(2).url,
                title: video(2).title,
            })
        );
        assert_eq!(v.key(Press::Escape, Some(&s), 100, &t), Errand::Nothing);
        assert_eq!(v.page(), PageId::Ranking);
        assert_eq!(v.spot_here(Some(&s), 100, &t), Some(spot));
    }

    /// FAILS IF: the pages stop at their ends, a new page does not start at
    /// its top, or `L` stops turning the list into the grid and back — or
    /// the grid's `⋯` stops being taken where it is drawn, on a line of its
    /// own and nowhere else of the tile.
    #[test]
    fn the_pages_and_the_layouts() {
        let t = texts("pt-BR");
        let s = fixture::idle();
        let mut v = on_the_ranking(3);
        v.ranking.given((0..PER_PAGE).map(video).collect(), 60);
        assert_eq!(v.ranking.pages(), 3);
        v.key(Press::Ranking(RankingKey::Earlier), Some(&s), 100, &t);
        assert_eq!(v.ranking.page, 0);
        v.key(Press::Ranking(RankingKey::Last), Some(&s), 100, &t);
        assert_eq!(v.ranking.page, 2);
        v.key(Press::Ranking(RankingKey::Later), Some(&s), 100, &t);
        assert_eq!(v.ranking.page, 2, "past the last");
        v.key(Press::Ranking(RankingKey::First), Some(&s), 100, &t);
        assert_eq!(v.ranking.page, 0);
        v.key(Press::Ranking(RankingKey::Layout), Some(&s), 100, &t);
        assert_eq!(v.ranking.layout, Layout::Grid);
        let rows = v.rows(Some(&s), 100, &t);
        let shelf = row_at(&rows, &format!("shelf-{}", spot_of(&video(0).id)));
        // THE ⋯ LINE is a line of its own, under the titles: a click on the
        // picture takes the video, and only the ⋯ opens its page.
        let more_line = (shelf..rows.len())
            .find(|&n| rows[n].cells.iter().any(|c| c.control))
            .expect("a line with the ⋯");
        assert!(rows[more_line].heading, "the ⋯'s line is the tile's row");
        assert!(rows[shelf].cells.iter().all(|c| !c.control));
        assert!(
            rows[more_line]
                .cells
                .iter()
                .all(|c| c.about == Some(MORE) && c.width == tokens::space::PLAYER_QUEUEGLYPH)
        );
        // AND IT LIGHTS ONLY ITS `⋯`, as a click takes only it: the lines
        // of what is said under the titles light nothing.
        use crate::player_rows::{Lighting, lighting};
        let mores: Vec<(u16, u16)> = rows[more_line]
            .cells
            .iter()
            .map(|c| (c.from, c.width))
            .collect();
        assert_eq!(lighting(&rows, more_line), Lighting::Cell(mores));
        assert_eq!(lighting(&rows, more_line + 1), Lighting::Cell(Vec::new()));
        v.key(Press::Ranking(RankingKey::Layout), Some(&s), 100, &t);
        assert_eq!(v.ranking.layout, Layout::List);
    }

    /// FAILS IF: the list's `⋯` is taken — by a click, or said under the
    /// pointer — on a line of the video's row it is not drawn on, or the
    /// row's lines stop lighting from its words on, each line the same.
    #[test]
    fn the_list_s_more_is_taken_on_its_own_line() {
        let t = texts("en-GB");
        let s = fixture::idle();
        let mut v = on_the_ranking(3);
        let rows = v.rows(Some(&s), 100, &t);
        let at = row_at(&rows, &spot_of(&video(1).id));
        let more = rows[at]
            .cells
            .iter()
            .find(|c| c.control)
            .expect("a ⋯")
            .clone();
        let lines = crate::player_rows::Row::lines_of(&rows, at);
        assert!(lines > 2, "a video's row is its picture's lines: {lines}");
        assert_eq!(
            crate::player_rows::control_under(&rows, at, more.from),
            Some(MORE)
        );
        let words = rows[at].fields[0].at;
        for l in at..at + lines {
            assert_eq!(
                crate::player_rows::lighting(&rows, l),
                crate::player_rows::Lighting::From(words),
                "line {l} of the video's row"
            );
            if l == at {
                continue;
            }
            assert_eq!(
                crate::player_rows::control_under(&rows, l, more.from),
                None,
                "the ⋯ said on line {l}, under it"
            );
            // A CLICK THERE IS A CLICK ON THE ROW: the first chooses it, the
            // second queues it — and the video's page never opens.
            v.follow(&spot_of(&video(0).id), Some(&s), 100, &t);
            assert!(!v.clicked_at(l, more.from, &rows), "line {l} acted at once");
            assert!(v.clicked_at(l, more.from, &rows), "the second click");
            assert!(
                matches!(
                    v.key(Press::Enter, Some(&s), 100, &t),
                    Errand::TellSaying(Ask::Put { .. }, _)
                ),
                "line {l}"
            );
            assert_eq!(v.page(), PageId::Ranking, "line {l} opened the page");
        }
    }

    /// FAILS IF: on the grid, a ranking that reordered under the cursor — a
    /// scan put a video on top — leaves the cursor on the cell its video was
    /// in, another's now: `RET` queued a video the person never chose.
    #[test]
    fn the_grid_s_cursor_keeps_its_video() {
        let t = texts("en-GB");
        let s = fixture::idle();
        let mut v = on_the_ranking(8);
        v.ranking.layout = Layout::Grid;
        let rows = v.rows(Some(&s), 100, &t);
        let shelf = row_at(&rows, &format!("shelf-{}", spot_of(&video(0).id)));
        assert!(rows[shelf].cells.len() >= 3, "a shelf of three at least");
        v.top_mut().cursor = shelf;
        v.top_mut().cell = 2;
        assert_eq!(
            v.ranking_here(Some(&s), 100, &t),
            Some(spot_of(&video(2).id))
        );
        // A NEW VIDEO TOPS THE RANKING, as the door's thread reads it.
        let worker = Worker {
            shared: Arc::default(),
            _watch: None,
        };
        let mut now: Vec<RankedVideo> = vec![video(99)];
        now.extend((0..8).map(video));
        worker
            .shared
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .answered
            .push((
                Want::Page(0),
                Ok(Got::Page(Listed {
                    page: 0,
                    rows: now,
                    total: 9,
                })),
            ));
        v.ranking.worker = Some(worker);
        v.settle_in(Some(&s), 100, &t);
        assert_eq!(
            v.ranking_here(Some(&s), 100, &t),
            Some(spot_of(&video(2).id)),
            "the cursor stayed on the cell, and not on its video"
        );
        let rows = v.rows(Some(&s), 100, &t);
        assert_eq!(rows[v.cursor()].target, Target::Ranked(video(2).id));
    }

    /// FAILS IF: a page the catalogue REFUSED — a machine with no catalogue
    /// yet — is asked again on every settle: a door with a loop read, woke
    /// and repainted without end. Or the first commit there, the scan the
    /// page waits for, is not read when it lands.
    #[test]
    fn a_refused_page_is_held_until_the_catalogue_moves() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-ranking-held-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("video-metrics").join("catalog.sqlite");
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let tx = Mutex::new(tx);
        let t = texts("en-GB");
        let mut v = View::blank(None);
        v.wake = Some(crate::player_subtitles::Wake::new(move || {
            let _ = tx.lock().unwrap_or_else(PoisonError::into_inner).send(());
        }));
        v.ranking_at(path.clone());
        v.key(Press::Go(PageId::Ranking), None, 100, &t);
        let wait = std::time::Duration::from_secs(20);
        rx.recv_timeout(wait).expect("the refusal came back");
        for _ in 0..4 {
            v.settle_in(None, 100, &t);
        }
        assert!(
            matches!(v.ranking.listed, Some(Err(Why::NotThere))),
            "{:?}",
            v.ranking.listed
        );
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(500))
                .is_err(),
            "read again, with nothing changed"
        );
        // THE FIRST SCAN COMMITS, and the page reads it.
        committed(&path);
        rx.recv_timeout(wait).expect("the commit was not heard");
        v.settle_in(None, 100, &t);
        assert!(
            matches!(v.ranking.listed, Some(Ok(ref l)) if l.total == 0),
            "{:?}",
            v.ranking.listed
        );
        drop(v);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a hide or a delete of this door's leaves the page as it
    /// was read — the video still listed — until some other commit.
    #[test]
    fn a_write_of_this_door_reads_the_page_again() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-ranking-again-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("video-metrics").join("catalog.sqlite");
        committed(&path);
        let t = texts("en-GB");
        let mut v = View::blank(None);
        v.ranking_at(path);
        v.key(Press::Go(PageId::Ranking), None, 100, &t);
        assert!(v.ranking.holds(0));
        assert!(v.ranking_told(&Outcome::Hidden(video(1).id, video(1).title, Ok(()))));
        assert!(!v.ranking.holds(0), "the page is held as it was");
        v.settle_in(None, 100, &t);
        assert!(v.ranking.holds(0), "read again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a machine with no catalogue draws an empty page instead of
    /// the sentence that says where channels are added — in both languages —
    /// or a catalogue that cannot be read is not said, or an empty one is
    /// shown as a page of nothing.
    #[test]
    fn without_the_catalogue_a_sentence() {
        let s = fixture::idle();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let mut v = View::blank(None);
            v.ranking = Ranking::at(None);
            v.key(Press::Go(PageId::Ranking), Some(&s), 100, &t);
            let said = said_on(&v.rows(Some(&s), 100, &t));
            let none = t.t_with(
                key::PLAYER_RANKING_NONE,
                &[("keys", v.ranking.opens_with.as_deref().unwrap_or_default())],
            );
            let first = crate::chrome::note_lines(&none, 40)[0].clone();
            assert!(said.contains(first.trim()), "{language}: {said}");
            v.ranking.listed = Some(Err(Why::Unreadable));
            let said = said_on(&v.rows(Some(&s), 100, &t));
            assert!(
                said.contains(
                    crate::chrome::note_lines(
                        &t.t(crate::video_metrics_words::why_key(Why::Unreadable)),
                        40
                    )[0]
                    .trim()
                ),
                "{language}: {said}"
            );
            v.ranking.given(Vec::new(), 0);
            let said = said_on(&v.rows(Some(&s), 100, &t));
            assert!(
                said.contains(
                    crate::chrome::note_lines(&t.t(key::PLAYER_RANKING_EMPTY), 40)[0].trim()
                ),
                "{language}: {said}"
            );
        }
    }

    /// A catalogue made by the video catalogue's own door — the one writer
    /// a door has (`video-metrics-readers-are-read-only`) — and a write of it,
    /// a commit of another connection than the reader's.
    fn committed(path: &Path) {
        let weights = ronoctua_video_metrics::Weights::default();
        let mut door = crate::video_metrics_feed::Direct::at(path.to_path_buf());
        let said = door.want(crate::video_metrics_feed::Want::Edit(
            crate::video_metrics::Edit::Weights(weights),
        ));
        assert!(
            said.iter().any(|a| matches!(
                a,
                crate::video_metrics_feed::Arrived::Done(crate::video_metrics::Done::Edited(
                    _,
                    Ok(())
                ))
            )),
            "the catalogue was not written"
        );
    }

    /// FAILS IF: the reading of a page stops coming with its count, a
    /// catalogue that is not there stops being said as such, or a stir with
    /// nothing committed by another connection is read again.
    #[test]
    fn the_reader_reads_a_page_with_its_count() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-ranking-reader-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut absent = Reader::new(dir.join("none").join("catalog.sqlite"));
        let got = absent.handle(vec![Want::Page(0)], false);
        assert!(
            matches!(got.as_slice(), [(_, Err(Why::NotThere))]),
            "{got:?}"
        );
        let path = dir.join("video-metrics").join("catalog.sqlite");
        committed(&path);
        let mut r = Reader::new(path.clone());
        let got = r.handle(vec![Want::Page(1), Want::Page(0)], false);
        assert_eq!(got.len(), 1, "two asks of a page read twice");
        assert!(
            matches!(&got[0], (Want::Page(0), Ok(Got::Page(l))) if l.total == 0 && l.rows.is_empty()),
            "{got:?}"
        );
        assert!(r.handle(Vec::new(), true).is_empty(), "nothing moved");
        // ANOTHER CONNECTION COMMITS.
        committed(&path);
        assert_eq!(r.handle(Vec::new(), true).len(), 1, "a commit was missed");
        // AND ONE THAT LANDS WITH AN ASK OF THE OTHER KIND: the page is read
        // too, and not left as it was until the next commit.
        committed(&path);
        let got = r.handle(vec![Want::Video(video(1).id)], true);
        assert!(
            got.iter().any(|(w, _)| *w == Want::Page(0))
                && got.iter().any(|(w, _)| *w == Want::Video(video(1).id)),
            "{got:?}"
        );
        let mut v = View::blank(None);
        v.ranking_at(path);
        v.key(Press::Go(PageId::Ranking), None, 100, &texts("en-GB"));
        assert!(matches!(v.ranking.listed, Some(Ok(ref l)) if l.total == 0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a page that shrank under the cursor — its videos hidden in
    /// another door — stays past the last one, empty.
    #[test]
    fn a_page_past_the_last_goes_back_to_the_last() {
        let mut r = Ranking::at(None);
        r.page = 3;
        r.keep(
            Want::Page(3),
            Ok(Got::Page(Listed {
                page: 3,
                rows: Vec::new(),
                total: u64::from(PER_PAGE) + 1,
            })),
        );
        assert_eq!(r.page, 1);
    }

    /// FAILS IF: what a hide, a delete or a download came to stops being
    /// said — the refusal's reason included — in either language.
    #[test]
    fn what_was_done_is_said() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let title = "Ünïcode title";
            for o in [
                Outcome::Hidden(video(1).id, title.into(), Ok(())),
                Outcome::Deleted(video(1).id, title.into(), Ok(())),
                Outcome::Hidden(video(1).id, title.into(), Err(Why::Busy)),
                Outcome::Downloading {
                    title: title.into(),
                    taken: true,
                    keys: Some("SUPER t t".into()),
                },
                Outcome::Downloading {
                    title: title.into(),
                    taken: true,
                    keys: None,
                },
                Outcome::Downloading {
                    title: title.into(),
                    taken: false,
                    keys: None,
                },
            ] {
                let s = said(&o, &t);
                assert!(s.contains(title), "{language}: {s}");
                assert!(!s.contains("player-"), "{language}: {s}");
            }
            let busy = said(
                &Outcome::Deleted(video(1).id, title.into(), Err(Why::Busy)),
                &t,
            );
            assert!(busy.contains(&t.t(crate::video_metrics_words::why_key(Why::Busy))));
        }
    }
}
