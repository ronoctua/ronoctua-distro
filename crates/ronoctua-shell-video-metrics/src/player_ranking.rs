//! THE RANKING — the video catalogue's videos, best first, as a page of the
//! player (`LEADER p k`, a row of "Listen and watch"): what the person's
//! channels published, scored by their own rules, one click from the queue.
//!
//! # What it reads, and how
//!
//! The video catalogue (`ronoctua_video_metrics`), opened READ-ONLY
//! (`Catalog::open_ro`) by a thread of this door's own — the player's daemon
//! never sees it: a page of the ranking is a question this door asks the
//! file, as channels by theme are (`ronoctua_shell_catalogue::player_catalog`). The thread
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
//! # Two lists
//!
//! The RANKING (`PageId::Ranking`) and the VIDEOS SET ASIDE for later
//! (`PageId::SetAside`, `LEADER p K`): the second only those, the last set
//! aside first, hidden ones included ([`List`], [`Ranking::asked`]). Each
//! keeps its own page and its own Shorts — with them, without them, only
//! them, `S` or its row's ring — the ranking opening WITHOUT them; kept while
//! the door is open, never written down. One thread and one held page serve
//! both, and what is held says which list it answers: a list never draws the
//! other's rows.
//!
//! # A row, a tile
//!
//! Each video is its thumbnail (the catalogue's file on disk, decoded off the
//! loop by `crate::player_posters`), its title, its channel and its score —
//! a bare number, the four arrows of its rating after it and its marks at the
//! line's edge (set aside, hidden, deleted, kept for ever), each a glyph a
//! click acts on at once, and a key (`b`, `v`, `d`, `F`, `1`–`4`) — a row
//! each, the picture at its head (`player_posters::strip`), or a grid of
//! thumbnails (`player_posters::tiled`); `L` and the glyph on the title turn
//! one into the other, and `<prior>`/`<next>`/`<home>`/`<end>` walk the
//! pages. `RET` — the second click — puts the video at the queue's end,
//! TITLED (`Thing::Named`) and ONCE: one the queue has already is the
//! player's refusal, which the view asks about, "no" first
//! (`player::overruled`, `Told::Asks`). `P` plays it now, `a` adds it.
//!
//! # More of one video
//!
//! The `⋯` at the end of a row — `e` on the keyboard — opens the video's own
//! page (`PageId::RankedVideo`): play now, add, watch it in the Chrome kiosk
//! (the footer's `kiosk-open`) or in Nyxt (its entry in the launcher),
//! download it (the transfers' queue), rate it, set it aside, keep it for
//! ever, hide it from the ranking, or delete it from the catalogue for good —
//! after a question, "no" first. Hiding and deleting are the two writes a
//! reader of the catalogue may make (`Catalog::hide`, `Catalog::delete`); a
//! rating, setting aside and keeping are the person's, written by the video
//! catalogue's own door (`video_metrics_feed::mark`). Each is carried out off
//! the loop ([`carry`]) and said when it is done. A mark is on its row from
//! the click until the catalogue is read again, and the next press composes
//! from it; one more on the same video waits, said so, while it is written.
//!
//! # Without the catalogue
//!
//! A machine with no catalogue yet — nothing ever added — says so in a
//! sentence that teaches where channels are added; a catalogue that cannot
//! be read says why. Never an empty page that reads as "nothing ranks".

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use crate::key;
use ronoctua_i18n::Arg;
use ronoctua_player::wire::{Ask, State, Thing};
use ronoctua_tokens as tokens;
use ronoctua_video_metrics::read::{
    Catalog, Filter, Forever, Hidden, Order, Page, RankedVideo, SetAside, Shorts, Video,
};
use ronoctua_video_metrics::{Rating, VideoId, Why};

use crate::player::{Errand, PageId, Press, Target, View};
use crate::player_posters::{Control, Said, Shape, Tile};
use crate::player_rows::Builder;

// THE WORDS THE PLAYER'S VIEW NAMES this page by — its keys, a video's own
// page's rows, the errands and answers a door carries for it — live under it
// (`crate::player_seam::ranking`): a line here recompiles nothing of the
// player's.
pub use crate::player_seam::ranking::{
    Act, LAYOUT, MORE, Mark, Outcome, Quick, RankedAct, RankingKey,
};

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

/// WHICH OF THE TWO LISTS a page of the ranking shows: every video, best
/// first, or the ones the person set aside for later, the last set aside
/// first. One reading thread and one held page serve both, so what is held
/// says which it is ([`Asked::list`]) — and a list never draws the other's
/// rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum List {
    /// The ranking: every video not hidden, best first — the ones set aside
    /// among them, marked.
    Ranking,
    /// The videos set aside, the last set aside first — the hidden ones too:
    /// the person set them aside, and the eye beside one shows it again.
    SetAside,
}

impl List {
    /// The list page `page` shows — `None` for a page that shows none.
    #[must_use]
    pub const fn of(page: PageId) -> Option<Self> {
        match page {
            PageId::Ranking => Some(Self::Ranking),
            PageId::SetAside => Some(Self::SetAside),
            _ => None,
        }
    }
}

/// WHERE A LIST STANDS, as the person left it: its page and its Shorts —
/// each list its own, kept while the door is open and never written down
/// (the owner's choice: the filter is the session's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown {
    /// Which page, from 0.
    pub page: u32,
    /// Its Shorts: with them, without them, only them.
    pub shorts: Shorts,
}

impl Shown {
    /// Where list `list` opens: its first page — the ranking WITHOUT ITS
    /// SHORTS, which a scan brings by the dozen and which push the long
    /// videos the ranking is for off its first pages; the videos set aside
    /// with theirs, since the person chose each one.
    #[must_use]
    pub const fn first(list: List) -> Self {
        Self {
            page: 0,
            shorts: match list {
                List::Ranking => Shorts::Without,
                List::SetAside => Shorts::Any,
            },
        }
    }
}

/// WHAT IS ASKED OF THE CATALOGUE for a list: which list, its filter, its
/// page and how many it holds — the one question the reading thread answers,
/// and the one a page held answers ([`Ranking::holds`]): a page read under
/// another filter, or at another size, is not the page asked.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Asked {
    /// Which list.
    pub list: List,
    /// What it shows, and in which order.
    pub filter: Filter,
    /// Which page, from 0.
    pub page: u32,
    /// How many videos a page holds (`paging::PER_PAGES`).
    pub per_page: u32,
}

/// What the reading thread is asked.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Want {
    /// A page of a list.
    Page(Asked),
    /// One video, whole.
    Video(VideoId),
}

/// A page of a list, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// What it answers: its list, its filter, its page.
    pub asked: Asked,
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
    page: Option<Asked>,
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
                Want::Page(asked) => self.page = Some(asked.clone()),
                Want::Video(id) => self.video = Some(id.clone()),
            }
            wants.retain(|w| std::mem::discriminant(w) != std::mem::discriminant(&want));
            wants.push(want);
        }
        if stirred && self.moved() {
            if !wants.iter().any(|w| matches!(w, Want::Page(_))) {
                wants.extend(self.page.clone().map(Want::Page));
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
            Want::Page(asked) => c
                .ranked(
                    &asked.filter,
                    Page {
                        number: asked.page,
                        per_page: asked.per_page,
                    },
                )
                .and_then(|rows| {
                    c.count(&asked.filter).map(|total| {
                        Got::Page(Listed {
                            asked: asked.clone(),
                            rows,
                            total,
                        })
                    })
                }),
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
    fn start(path: PathBuf, wake: crate::wake::Wake) -> Option<Self> {
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
/// page that says "nothing yet" waits for. A log's watch (`Notice::on_log`),
/// as the video catalogue's door's: the scanner writes into a `-wal` it
/// never closes while it runs, and the videos a scan adds are on this page
/// as they land, not once it is over.
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
    ronoctua_files::notice::Notice::on_log(
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

/// The ranking as this view holds it — both of its lists.
#[derive(Debug)]
pub struct Ranking {
    /// The catalogue: this machine's, or a photograph's; `None` with no home
    /// to keep one in.
    path: Option<PathBuf>,
    /// Where the ranking stands: its page, and its Shorts — left out until
    /// the person turns them in ([`Shown::first`]).
    pub ranking: Shown,
    /// Where the list of the videos set aside stands.
    pub aside: Shown,
    /// How both are drawn.
    pub layout: Layout,
    /// HOW MANY VIDEOS A PAGE OF EITHER HOLDS — a ring of the session's, as
    /// the catalogue's own list's (`paging::PerPage`).
    pub per_page: crate::paging::PerPage,
    /// The video whose own page is open.
    pub chosen: Option<VideoId>,
    /// The last page read, of either list, or why it was not.
    listed: Option<Result<Listed, Why>>,
    /// What `listed` answers — read, or REFUSED: a refusal is an answer
    /// too, held until the catalogue moves (the thread's watch reads it
    /// again) or another page is asked. Asked again on every settle, a
    /// machine with no catalogue read, woke and repainted without end. And
    /// WHICH LIST it is: the other list's page is never drawn as this one's.
    answered: Option<Asked>,
    /// The page held was changed by a write of this door's, and is read
    /// again at the next look (the video read whole is dropped instead).
    stale: bool,
    /// The last video read, or why it was not.
    video: Option<(VideoId, Whole)>,
    /// What was asked and not answered yet — asked once.
    asked_page: Option<Asked>,
    asked_video: Option<VideoId>,
    /// The reading thread, once a door with a loop asked.
    worker: Option<Worker>,
    /// The thumbnails of what is on screen, decoded off the loop.
    pub posters: crate::player_posters::Posters,
    /// How the video catalogue is opened, as the leader teaches it — read
    /// once, for the sentence a machine with no catalogue says.
    opens_with: Option<String>,
    /// What the rows were handed and not read: a test's.
    given: bool,
    /// The spot the cursor goes back to at the next settle — the row that
    /// opened a video's page this door closed when the video went.
    follow_owed: Option<String>,
    /// THE MARKS THIS DOOR SENT and has not read back yet, in the order
    /// sent: a row is drawn, and the next press on it composes, with these
    /// put on it ([`Ranking::row_of`]). Composed from the row as read, a
    /// second press before the catalogue was read again wrote the same next
    /// rating twice, and set aside again what the first took out.
    marking: Vec<Sent>,
}

/// A MARK THIS DOOR SENT: being written, or written and not read back yet —
/// let go of at the next reading kept ([`Ranking::read_back`]).
#[derive(Debug, Clone)]
struct Sent {
    mark: Mark,
    /// The catalogue's door told it written. Until then the video's next
    /// mark waits ([`Ranking::writing`]): two writes, each on a thread of
    /// its own, land in either order behind a scan that holds the
    /// catalogue.
    written: bool,
}

/// The video `mark` is on.
const fn video_of(mark: &Mark) -> &VideoId {
    match mark {
        Mark::Rating(id, _) | Mark::SetAside(id, _) | Mark::Keep(id, _) | Mark::Hidden(id, _) => id,
    }
}

/// `mark` put on `v`, as the catalogue reads it back once written — but for
/// a channel that keeps every video, behind the video's own mark taken off:
/// the row never said it, and the next reading does.
fn put_on(mark: &Mark, v: &mut RankedVideo) {
    match mark {
        Mark::Rating(_, rating) => v.rating = *rating,
        Mark::SetAside(_, on) => {
            v.set_aside_at_s = if *on {
                Some(v.set_aside_at_s.unwrap_or_else(now_s))
            } else {
                None
            };
        }
        Mark::Keep(_, on) => {
            v.forever = if *on { Forever::ByVideo } else { Forever::No };
        }
        Mark::Hidden(_, on) => v.hidden = *on,
    }
}

/// BY HAND, AND NOT DERIVED: a derived default would open the ranking with
/// its Shorts in (`Shorts::Any`, the catalogue's own default), and a view's
/// ranking is built from this one (`Ranked::state`).
impl Default for Ranking {
    fn default() -> Self {
        Self {
            path: None,
            ranking: Shown::first(List::Ranking),
            aside: Shown::first(List::SetAside),
            // THE THUMBNAILS FIRST (the owner, 2026-11-10: "o player abre em
            // miniaturas") — both lists, in both doors; `L` still turns to
            // the list. The enum's own default stays the list, which nothing
            // opens on.
            layout: Layout::Grid,
            per_page: crate::paging::PerPage::default(),
            chosen: None,
            listed: None,
            answered: None,
            stale: false,
            video: None,
            asked_page: None,
            asked_video: None,
            worker: None,
            posters: crate::player_posters::Posters::default(),
            opens_with: None,
            given: false,
            follow_owed: None,
            marking: Vec::new(),
        }
    }
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

    /// A page of the RANKING HANDED to the view, read from nowhere — a
    /// test's (`test-support`).
    #[cfg(any(test, feature = "test-support"))]
    pub fn given(&mut self, rows: Vec<RankedVideo>, total: u64) {
        self.given_to(List::Ranking, rows, total);
    }

    /// A page of the VIDEOS SET ASIDE handed to the view — a test's.
    #[cfg(any(test, feature = "test-support"))]
    pub fn given_aside(&mut self, rows: Vec<RankedVideo>, total: u64) {
        self.given_to(List::SetAside, rows, total);
    }

    /// A page of `list` handed to the view, as asked now.
    #[cfg(any(test, feature = "test-support"))]
    fn given_to(&mut self, list: List, rows: Vec<RankedVideo>, total: u64) {
        let asked = self.asked(list);
        self.given = true;
        self.answered = Some(asked.clone());
        self.listed = Some(Ok(Listed { asked, rows, total }));
        self.read_back();
    }

    /// Where `list` stands.
    #[must_use]
    pub const fn shown(&self, list: List) -> Shown {
        match list {
            List::Ranking => self.ranking,
            List::SetAside => self.aside,
        }
    }

    /// The same, to change.
    const fn shown_mut(&mut self, list: List) -> &mut Shown {
        match list {
            List::Ranking => &mut self.ranking,
            List::SetAside => &mut self.aside,
        }
    }

    /// WHAT `list` ASKS of the catalogue, where it stands now. The RANKING:
    /// every video not hidden, best first, its Shorts as turned — the ones
    /// set aside among them, marked. The VIDEOS SET ASIDE: only those, the
    /// last set aside first, hidden or not.
    #[must_use]
    pub fn asked(&self, list: List) -> Asked {
        let shown = self.shown(list);
        let filter = match list {
            List::Ranking => Filter {
                shorts: shown.shorts,
                ..Filter::default()
            },
            List::SetAside => Filter {
                shorts: shown.shorts,
                set_aside: SetAside::Only,
                hidden: Hidden::Any,
                order: Order::NewestSetAside,
                ..Filter::default()
            },
        };
        Asked {
            list,
            filter,
            page: shown.page,
            per_page: self.per_page.get(),
        }
    }

    /// How many pages `list` takes, at least one — as its page held says, at
    /// the size asked.
    fn pages(&self, list: List) -> u32 {
        match self.listed(list) {
            Some(Ok(l)) => crate::paging::pages_of(l.total, self.per_page.get()).max(1),
            _ => 1,
        }
    }

    /// The page held, read or refused, WHEN IT IS `list`'s: the other list's
    /// is never drawn as this one's.
    fn listed(&self, list: List) -> Option<&Result<Listed, Why>> {
        self.listed
            .as_ref()
            .filter(|_| self.answered.as_ref().is_some_and(|a| a.list == list))
    }

    /// The list the page held is of — the one under a video's own page.
    fn listed_list(&self) -> Option<List> {
        self.answered.as_ref().map(|a| a.list)
    }

    /// A video of the page, or the one read whole — whichever holds it —
    /// with the marks this door sent and has not read back put on it.
    fn row_of(&self, id: &VideoId) -> Option<Cow<'_, RankedVideo>> {
        let whole = self
            .video
            .as_ref()
            .filter(|(v, _)| v == id)
            .and_then(|(_, got)| got.as_ref().ok()?.as_ref())
            .map(|v| &v.row);
        let read = whole.or_else(|| match &self.listed {
            Some(Ok(l)) => l.rows.iter().find(|r| &r.id == id),
            _ => None,
        })?;
        Some(self.as_marked(read))
    }

    /// `v` as read, with the marks this door sent on it and has not read
    /// back put on it, in the order sent: what its glyphs light, what its
    /// own page's rows say, and what the next press on it composes from.
    fn as_marked<'r>(&self, v: &'r RankedVideo) -> Cow<'r, RankedVideo> {
        let mut sent = self
            .marking
            .iter()
            .filter(|s| video_of(&s.mark) == &v.id)
            .peekable();
        if sent.peek().is_none() {
            return Cow::Borrowed(v);
        }
        let mut marked = v.clone();
        for s in sent {
            put_on(&s.mark, &mut marked);
        }
        Cow::Owned(marked)
    }

    /// Whether a mark this door sent on video `id` is still being written.
    fn writing(&self, id: &VideoId) -> bool {
        self.marking
            .iter()
            .any(|s| !s.written && video_of(&s.mark) == id)
    }

    /// What a mark this door sent came to: WRITTEN, it stays on the rows
    /// until the catalogue is read again; REFUSED, it is dropped, and the
    /// row is the catalogue's as it was.
    fn mark_told(&mut self, mark: &Mark, written: bool) {
        let Some(at) = self
            .marking
            .iter()
            .position(|s| !s.written && s.mark == *mark)
        else {
            return;
        };
        if written {
            self.marking[at].written = true;
        } else {
            self.marking.remove(at);
        }
    }

    /// A READING KEPT — a page, a video, refused too — says what the
    /// catalogue holds now: the marks told written are in it, and let go of.
    fn read_back(&mut self) {
        self.marking.retain(|s| !s.written);
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

    /// Whether what `asked` asks is held, read or refused, and not stale.
    fn holds(&self, asked: &Asked) -> bool {
        !self.stale && self.answered.as_ref() == Some(asked)
    }

    /// An answer kept: a page past the last — its list shrank under it —
    /// goes back to the last one, and is asked for.
    fn keep(&mut self, want: Want, got: Result<Got, Why>) {
        match (want, got) {
            (Want::Page(asked), Ok(Got::Page(l))) => {
                if self.asked_page.as_ref() == Some(&asked) {
                    self.asked_page = None;
                }
                let last = crate::paging::pages_of(l.total, asked.per_page).saturating_sub(1);
                let shrank = l.rows.is_empty() && asked.page > last;
                let list = asked.list;
                let n = asked.page;
                self.listed = Some(Ok(l));
                self.answered = Some(asked);
                self.stale = false;
                self.read_back();
                if shrank && self.shown(list).page == n {
                    self.shown_mut(list).page = last;
                }
            }
            (Want::Page(asked), Err(why)) => {
                if self.asked_page.as_ref() == Some(&asked) {
                    self.asked_page = None;
                }
                self.listed = Some(Err(why));
                self.answered = Some(asked);
                self.stale = false;
                self.read_back();
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
                self.read_back();
            }
            (Want::Page(_), Ok(Got::Video(_))) => {}
        }
    }

    /// `want` asked — of the thread, or read in place with no loop.
    fn ask(&mut self, want: Want, wake: Option<&crate::wake::Wake>) {
        let Some(path) = self.path.clone() else {
            let got = Err(Why::NotThere);
            self.keep(want, got);
            return;
        };
        match &want {
            Want::Page(asked) => self.asked_page = Some(asked.clone()),
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
            let keys = crate::bindings::taught("transfers-open", None);
            Outcome::Downloading { title, taken, keys }
        }
        // A MARK OF THE PERSON'S, written by the video catalogue's own door —
        // the one that may write the person's catalogue.
        Act::Mark {
            catalogue,
            mark,
            title,
        } => {
            let done = crate::video_metrics_feed::mark(&catalogue, &mark);
            Outcome::Marked(mark, title, done)
        }
    }
}

/// What a door says of `outcome` — the video by its title, told as the
/// name the sentence quotes (`Arg::Name`), drawn in a name's ink.
#[must_use]
pub fn said(outcome: &Outcome, texts: &ronoctua_i18n::Texts) -> crate::chrome::Named {
    let titled = |k: &str, title: &str| -> crate::chrome::Named {
        texts
            .t_args_marked(k, &[("title", Arg::Name(title))])
            .into()
    };
    let refused = |title: &str, why: Why| -> crate::chrome::Named {
        texts
            .t_args_marked(
                key::PLAYER_RANKING_NOT_CHANGED,
                &[
                    ("title", Arg::Name(title)),
                    (
                        "why",
                        Arg::Text(&texts.t(crate::video_metrics_words::why_key(why))),
                    ),
                ],
            )
            .into()
    };
    match outcome {
        Outcome::Hidden(_, title, Ok(())) => titled(key::PLAYER_RANKING_HIDDEN, title),
        Outcome::Deleted(_, title, Ok(())) => titled(key::PLAYER_RANKING_DELETED, title),
        Outcome::Marked(Mark::Rating(_, Some(rating)), title, Ok(())) => texts
            .t_args_marked(
                key::PLAYER_RANKING_RATED,
                &[
                    ("title", Arg::Name(title)),
                    (
                        "rating",
                        Arg::Text(&texts.t(crate::video_metrics_words::rating_key(*rating))),
                    ),
                ],
            )
            .into(),
        Outcome::Marked(mark, title, Ok(())) => {
            let said = match mark {
                Mark::Rating(_, _) => key::PLAYER_RANKING_UNRATED,
                Mark::SetAside(_, true) => key::PLAYER_RANKING_NOW_SET_ASIDE,
                Mark::SetAside(_, false) => key::PLAYER_RANKING_NO_LONGER_SET_ASIDE,
                Mark::Keep(_, true) => key::PLAYER_RANKING_NOW_FOREVER,
                Mark::Keep(_, false) => key::PLAYER_RANKING_NO_LONGER_FOREVER,
                Mark::Hidden(_, true) => key::PLAYER_RANKING_HIDDEN,
                Mark::Hidden(_, false) => key::PLAYER_RANKING_SHOWN_AGAIN,
            };
            texts
                .t_args_marked(said, &[("title", Arg::Name(title))])
                .into()
        }
        Outcome::Hidden(_, title, Err(why))
        | Outcome::Deleted(_, title, Err(why))
        | Outcome::Marked(_, title, Err(why)) => refused(title, *why),
        Outcome::Downloading {
            title,
            taken: true,
            keys: Some(keys),
        } => texts
            .t_args_marked(
                key::PLAYER_RANKING_DOWNLOADING,
                &[("title", Arg::Name(title)), ("keys", Arg::Text(keys))],
            )
            .into(),
        Outcome::Downloading {
            title,
            taken: true,
            keys: None,
        } => titled(key::PLAYER_RANKING_DOWNLOADING_PLAIN, title),
        Outcome::Downloading {
            title,
            taken: false,
            ..
        } => titled(key::PLAYER_RANKING_NOT_DOWNLOADED, title),
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
            texts
                .t_args_marked(key::PLAYER_PLAYING_NOW, &[("name", Arg::Name(&v.title))])
                .into()
        } else {
            texts
                .t_args_marked(key::PLAYER_RANKING_ADDED, &[("title", Arg::Name(&v.title))])
                .into()
        },
    )
}

/// A row's spot: the video's id — the cursor keeps it when the ranking
/// reorders under it.
#[must_use]
pub fn spot_of(id: &VideoId) -> String {
    format!("ranked-{}", id.0)
}

/// What is said under a video's title, each line in its parts, with its
/// glyphs (`player_posters::Tile::under`). Its SCORE IS A BARE NUMBER — the
/// list is by score, and "points" on every line was a word read once and
/// then only in the way — with the FOUR ARROWS of its rating right after it
/// ([`ratings`]), and its MARKS at the line's right edge ([`marks`]). In a
/// ROW: its channel and its score, then when it was published and how long
/// it runs, the marks at that line's end — or at the first's, with neither
/// known. In a TILE of the grid, a box too narrow for both on a line: its
/// score first — what the ranking is by, never cut — and its channel under
/// it, the marks beside the channel.
fn under(v: &RankedVideo, layout: Layout, now_s: i64, texts: &ronoctua_i18n::Texts) -> Vec<Said> {
    let score = crate::video_metrics_words::score_number(v.score, texts);
    if layout == Layout::Grid {
        return vec![
            Said {
                parts: vec![score],
                after: ratings(v),
                end: Vec::new(),
            },
            Said {
                parts: vec![v.channel.clone()],
                after: Vec::new(),
                end: marks(v),
            },
        ];
    }
    let when: Vec<String> = v
        .published_at_s
        .map(|p| crate::video_metrics_words::age(p, now_s, texts))
        .into_iter()
        .chain(v.duration_s.map(crate::video_metrics_words::length))
        .collect();
    let mut first = Said {
        parts: vec![v.channel.clone(), score],
        after: ratings(v),
        end: Vec::new(),
    };
    if when.is_empty() {
        first.end = marks(v);
        return vec![first];
    }
    vec![
        first,
        Said {
            parts: when,
            after: Vec::new(),
            end: marks(v),
        },
    ]
}

/// THE FOUR ARROWS OF A VIDEO'S RATING, from very negative to very positive:
/// the one it has lit, and a click on it takes the rating off; on another, it
/// rates the video so.
fn ratings(v: &RankedVideo) -> Vec<Control> {
    Rating::ALL
        .into_iter()
        .map(|r| {
            let q = Quick::Rate(r);
            Control {
                glyph: crate::video_metrics_words::rating_glyph(r),
                lit: v.rating == Some(r),
                target: Target::RankedQuick(v.id.clone(), q),
                about: q.command(),
            }
        })
        .collect()
}

/// A VIDEO'S MARKS, each a click that acts at once: set aside — the bookmark
/// filled while it is — hidden, or shown again when it is (the set-aside list
/// holds the hidden ones too), deleted after a question, and kept for ever
/// (the ∞, lit while it is kept, by its own mark or by its channel's).
fn marks(v: &RankedVideo) -> Vec<Control> {
    let id = &v.id;
    let aside = v.set_aside_at_s.is_some();
    vec![
        Control {
            glyph: crate::video_metrics_words::set_aside_glyph(aside),
            lit: aside,
            target: Target::RankedQuick(id.clone(), Quick::SetAside),
            about: Quick::SetAside.command(),
        },
        Control {
            glyph: crate::video_metrics_words::hidden_glyph(v.hidden),
            lit: v.hidden,
            target: Target::RankedQuick(id.clone(), Quick::Hide),
            about: if v.hidden {
                crate::player_seam::ranking::SHOW
            } else {
                Quick::Hide.command()
            },
        },
        Control {
            glyph: tokens::icon::VIDEO_DELETE,
            lit: false,
            target: Target::RankedQuick(id.clone(), Quick::Delete),
            about: Quick::Delete.command(),
        },
        Control {
            glyph: tokens::icon::VIDEO_FOREVER,
            lit: v.forever != Forever::No,
            target: Target::RankedQuick(id.clone(), Quick::Forever),
            about: Quick::Forever.command(),
        },
    ]
}

/// The rating a turn of the ring gives after `now` — none, then the four,
/// from very negative to very positive — `forward`, else back.
fn rating_turned(now: Option<Rating>, forward: bool) -> Option<Rating> {
    let ring: Vec<Option<Rating>> = std::iter::once(None)
        .chain(Rating::ALL.into_iter().map(Some))
        .collect();
    let at = ring.iter().position(|r| *r == now).unwrap_or(0);
    let next = if forward {
        (at + 1) % ring.len()
    } else {
        (at + ring.len() - 1) % ring.len()
    };
    ring[next]
}

/// THE RATING'S RING on video `id`'s own page, turned once — `forward`, else
/// back — and written as the person's mark ([`marked`]): the one way the
/// ring turns, whichever key or click turned it.
fn rating_ring(
    ranking: &mut Ranking,
    id: &VideoId,
    forward: bool,
    texts: &ronoctua_i18n::Texts,
) -> Errand {
    marked(
        ranking,
        id,
        |v| Mark::Rating(id.clone(), rating_turned(v.rating, forward)),
        texts,
    )
}

/// The video a quick mark acts on: the one whose own page is open, else the
/// one under the cursor on the ranking's rows — none on a row that is no
/// video.
fn video_here(
    view: &View,
    state: Option<&State>,
    columns: u16,
    texts: &ronoctua_i18n::Texts,
) -> Option<VideoId> {
    if view.page() == PageId::RankedVideo {
        return view.ranking().chosen.clone();
    }
    match view
        .rows(state, columns, texts)
        .get(view.cursor())
        .map(|r| r.target.clone())
    {
        Some(Target::Ranked(id)) => Some(id),
        _ => None,
    }
}

/// Why nothing is sent for video `title`: a mark on it is still being
/// written ([`Ranking::writing`]) — said, and pressed again in a moment.
fn still_writing(title: &str, texts: &ronoctua_i18n::Texts) -> Errand {
    Errand::Said(
        texts
            .t_args_marked(
                key::PLAYER_RANKING_STILL_MARKING,
                &[("title", Arg::Name(title))],
            )
            .into(),
    )
}

/// A MARK ON VIDEO `id`, as the person's mark makes it of the video held —
/// with the marks sent before it on ([`Ranking::row_of`]) — written off the
/// loop by the video catalogue's door, kept on the rows until it is read
/// back, and said when it is done; or why not, at once: no such video held,
/// a mark on it still being written, no catalogue.
fn marked(
    ranking: &mut Ranking,
    id: &VideoId,
    mark: impl FnOnce(&RankedVideo) -> Mark,
    texts: &ronoctua_i18n::Texts,
) -> Errand {
    let Some(v) = ranking.row_of(id) else {
        return Errand::Said(
            texts
                .t(crate::video_metrics_words::why_key(Why::NotFound))
                .into(),
        );
    };
    let (mark, title) = (mark(&v), v.title.clone());
    if ranking.writing(id) {
        return still_writing(&title, texts);
    }
    let Some(catalogue) = ranking.path.clone() else {
        return Errand::Said(
            texts
                .t(crate::video_metrics_words::why_key(Why::NotThere))
                .into(),
        );
    };
    ranking.marking.push(Sent {
        mark: mark.clone(),
        written: false,
    });
    Errand::Ranking(Act::Mark {
        catalogue,
        mark,
        title,
    })
}

/// A QUICK MARK on video `id` — its glyph clicked, its key, a row of its own
/// page: a rating — the one it has taken off — setting it aside or taking it
/// out, keeping it for ever by its own mark — whatever its channel keeps, so
/// it stays if the channel changes — or the mark taken off, a hidden video
/// shown again, all written off the loop; hidden at once; deleted after a
/// question, "no" first. None while a mark on it is still being written.
fn quick(view: &mut View, id: &VideoId, q: Quick, texts: &ronoctua_i18n::Texts) -> Errand {
    let hidden = view.ranking().row_of(id).is_some_and(|v| v.hidden);
    let ranking = view.ranking_mut();
    match q {
        // ONE HIDDEN ALREADY — the set-aside list holds them — is shown in
        // the ranking again: a mark of the person's, as the eye beside it
        // says.
        Quick::Hide if hidden => marked(ranking, id, |_| Mark::Hidden(id.clone(), false), texts),
        Quick::Rate(r) => marked(
            ranking,
            id,
            |v| Mark::Rating(id.clone(), (v.rating != Some(r)).then_some(r)),
            texts,
        ),
        Quick::SetAside => marked(
            ranking,
            id,
            |v| Mark::SetAside(id.clone(), v.set_aside_at_s.is_none()),
            texts,
        ),
        Quick::Forever => marked(
            ranking,
            id,
            |v| Mark::Keep(id.clone(), v.forever != Forever::ByVideo),
            texts,
        ),
        Quick::Hide | Quick::Delete => {
            let Some(title) = ranking.row_of(id).map(|v| v.title.clone()) else {
                return Errand::Said(
                    texts
                        .t(crate::video_metrics_words::why_key(Why::NotFound))
                        .into(),
                );
            };
            // NOT RACING A MARK: shown again and hidden, each on a thread of
            // its own, land in either order.
            if ranking.writing(id) {
                return still_writing(&title, texts);
            }
            let Some(catalogue) = ranking.path.clone() else {
                return Errand::Said(
                    texts
                        .t(crate::video_metrics_words::why_key(Why::NotThere))
                        .into(),
                );
            };
            let id = id.clone();
            if q == Quick::Hide {
                return Errand::Ranking(Act::Hide {
                    catalogue,
                    id,
                    title,
                });
            }
            // FOR GOOD, after a question — "no" first, the yes in the colour
            // of what is lost: a deleted video never comes back. Its title is
            // a NAME the sentence quotes, in a name's ink.
            let sentence = texts.t_args_marked(
                key::ASK_PLAYER_RANKING_DELETE,
                &[("title", Arg::Name(&title))],
            );
            view.ask_about(
                sentence,
                Errand::Ranking(Act::Delete {
                    catalogue,
                    id,
                    title,
                }),
            )
        }
    }
}

/// The Shorts a turn of the ring gives after `now` — with them, without
/// them, only them — `forward`, else back.
fn shorts_after(now: Shorts, forward: bool) -> Shorts {
    const RING: [Shorts; 3] = [Shorts::Any, Shorts::Without, Shorts::Only];
    let at = RING.iter().position(|s| *s == now).unwrap_or(0);
    let next = if forward {
        (at + 1) % RING.len()
    } else {
        (at + RING.len() - 1) % RING.len()
    };
    RING[next]
}

/// THE SHORTS OF `list`, turned once — `forward`, else back — whichever key
/// or click turned them: a new filter is a new list, read from its first
/// page, the cursor staying on the ring that turned. Kept while the door is
/// open, never written down.
fn shorts_turned(
    view: &mut View,
    list: List,
    forward: bool,
    state: Option<&State>,
    columns: u16,
    texts: &ronoctua_i18n::Texts,
) {
    let shown = view.ranking_mut().shown_mut(list);
    shown.shorts = shorts_after(shown.shorts, forward);
    shown.page = 0;
    view.follow("ranking-shorts", state, columns, texts);
    view.settle_in(state, columns, texts);
}

/// Now, in seconds since the epoch — an age is counted from it.
fn now_s() -> i64 {
    crate::video_metrics::now_s()
}

/// THE RANKING as the player's view holds it — this crate's half of the view,
/// over the view's own (the seam: `crate::player_seam`): the ranking's state
/// is the view's plugged [`Ranking`].
pub trait RankingView {
    /// The ranking as this view holds it — the view's plugged [`Ranking`].
    fn ranking(&self) -> &Ranking;

    /// The same, to change.
    fn ranking_mut(&mut self) -> &mut Ranking;

    /// The catalogue the ranking reads is `path` — a photograph's, a
    /// test's.
    fn ranking_at(&mut self, path: PathBuf);

    /// WHAT THE RANKING'S PAGES READ, on each settle: the answers that came
    /// back — the cursor kept on its video, wherever the ranking moved it —
    /// the page or the video on screen asked when it is not the one held,
    /// and the thumbnails of what is shown decoded.
    fn ranking_look(&mut self, state: Option<&State>, columns: u16, texts: &ronoctua_i18n::Texts);

    /// WHERE THE CURSOR IS, by the video's id: on a row of the list its
    /// spot, and on a shelf of the grid the spot of the cell it is on — the
    /// shelf's is its first video's, and a ranking that reordered under it
    /// put another video in that cell.
    fn ranking_here(
        &self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<String>;

    /// The cursor on `spot` — a row's, or a cell's of a shelf, cursor and
    /// cell both — wherever the rows put it now ([`Self::ranking_here`]).
    fn ranking_follow(
        &mut self,
        spot: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    );

    /// The title of list page `page` — the ranking's, or the videos set
    /// aside — its layout's glyph beside it.
    fn ranking_title(&self, page: PageId, b: &mut Builder<'_>);

    /// Why there is no ranking to show, in a sentence: no catalogue yet —
    /// and where channels are added — or why it cannot be read.
    fn ranking_refused(&self, why: Why, b: &mut Builder<'_>);

    /// The rows of `list`, under its title: the page, the layout, its
    /// Shorts, and the videos of the page — a row each or a grid.
    fn ranking_rows(&self, list: List, b: &mut Builder<'_>);

    /// A video's own page: its thumbnail, what it is, and what can be done
    /// with it.
    fn ranked_video_rows(&self, b: &mut Builder<'_>);

    /// A video's own page opened, over the ranking.
    fn open_ranked(
        &mut self,
        id: &VideoId,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    );

    /// The ranking's own keys.
    fn ranking_pressed(
        &mut self,
        key: RankingKey,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand;

    /// What a press does on a row of the ranking's pages — `None` for a
    /// target that is none of theirs.
    fn ranking_turned(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand>;

    /// A row of a video's own page, pressed.
    fn ranked_do(
        &mut self,
        act: RankedAct,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand;

    /// What a hide, a delete or a mark came to, for the view: hidden or
    /// deleted, the video's own page closes — it is no longer on the list
    /// under it — and the cursor goes back to the row that opened it at the
    /// next settle; but hidden from the videos set aside, which hold the
    /// hidden ones, or marked, the page stays. Either way the list is read
    /// again — and a mark refused is dropped from the rows it was drawn on.
    /// `true` when the rows changed.
    fn ranking_told(&mut self, outcome: &Outcome) -> bool;
}

impl RankingView for View {
    fn ranking(&self) -> &Ranking {
        self.plugged::<Ranking>().expect(PLUGGED)
    }

    fn ranking_mut(&mut self) -> &mut Ranking {
        self.plugged_mut::<Ranking>().expect(PLUGGED)
    }

    fn ranking_at(&mut self, path: PathBuf) {
        *self.ranking_mut() = Ranking::at(Some(path));
    }

    fn ranking_look(&mut self, state: Option<&State>, columns: u16, texts: &ronoctua_i18n::Texts) {
        let page = self.page();
        if !matches!(
            page,
            PageId::Ranking | PageId::SetAside | PageId::RankedVideo
        ) {
            return;
        }
        if self.ranking().opens_with.is_none() {
            self.ranking_mut().opens_with =
                Some(crate::bindings::taught("video-metrics-open", None).unwrap_or_default());
        }
        let came = self
            .ranking()
            .worker
            .as_ref()
            .map_or_else(Vec::new, Worker::take);
        if let Some(spot) = self.ranking_mut().follow_owed.take() {
            self.ranking_follow(&spot, state, columns, texts);
            self.top_mut().moved = true;
        }
        if !came.is_empty() {
            let here = self.ranking_here(state, columns, texts);
            for (want, got) in came {
                self.ranking_mut().keep(want, got);
            }
            if let Some(here) = here {
                self.ranking_follow(&here, state, columns, texts);
            }
        }
        if self.ranking().given {
            return;
        }
        let wake = self.wake.clone();
        // A PAGE ANSWERED IS HELD, refused too: it is read again when the
        // catalogue moves — the thread's watch — or another page, another
        // filter or the other list is asked.
        if let Some(list) = List::of(page) {
            let asked = self.ranking().asked(list);
            let held = self.ranking().holds(&asked);
            if !held && self.ranking().asked_page.as_ref() != Some(&asked) {
                self.ranking_mut().ask(Want::Page(asked), wake.as_ref());
            }
        }
        if page == PageId::RankedVideo
            && let Some(id) = self.ranking().chosen.clone()
        {
            let held = self.ranking().video.as_ref().is_some_and(|(v, _)| *v == id);
            if !held && self.ranking().asked_video.as_ref() != Some(&id) {
                self.ranking_mut().ask(Want::Video(id), wake.as_ref());
            }
        }
        // THE THUMBNAILS OF WHAT IS SHOWN — the page's, or the video's own.
        let files: Vec<PathBuf> = match List::of(page) {
            Some(list) => match self.ranking().listed(list) {
                Some(Ok(l)) => l.rows.iter().filter_map(|r| r.thumb_file.clone()).collect(),
                _ => Vec::new(),
            },
            None => self
                .ranking()
                .chosen
                .as_ref()
                .and_then(|id| self.ranking().row_of(id))
                .and_then(|r| r.thumb_file.clone())
                .into_iter()
                .collect(),
        };
        self.ranking_mut().posters.look_files(&files, wake.as_ref());
    }

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

    fn ranking_title(&self, page: PageId, b: &mut Builder<'_>) {
        b.heading_controls_said(
            b.t(page.key()),
            &[(self.ranking().layout.glyph(), Target::RankingLayout, LAYOUT)],
        );
    }

    fn ranking_refused(&self, why: Why, b: &mut Builder<'_>) {
        let said = match why {
            Why::NotThere => b.texts.t_with(
                key::PLAYER_RANKING_NONE,
                &[(
                    "keys",
                    self.ranking().opens_with.as_deref().unwrap_or_default(),
                )],
            ),
            other => b.t(crate::video_metrics_words::why_key(other)),
        };
        crate::player_find::note_said(&said, b);
    }

    fn ranking_rows(&self, list: List, b: &mut Builder<'_>) {
        // THE OTHER LIST'S PAGE is never drawn as this one's: until this
        // one's is read, it is being read.
        let listed = match self.ranking().listed(list) {
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
        let shown = self.ranking().shown(list);
        let n = |x: u64| Arg::Number(i64::try_from(x).unwrap_or(i64::MAX));
        b.pair(
            b.t(key::PLAYER_CATALOG_PAGE),
            b.texts.t_args(
                key::PLAYER_CATALOG_PAGE_OF,
                &[
                    ("page", Arg::Number(i64::from(listed.asked.page) + 1)),
                    ("pages", Arg::Number(i64::from(self.ranking().pages(list)))),
                ],
            ),
            Target::RankingPage,
            "ranking-page",
        );
        // HOW MANY A PAGE HOLDS, a ring of the session's, under the page —
        // in words of its own: the catalogue listing's "Per page" is a page
        // of the same view (`labels-of-a-view-are-distinct`). And SHORT: in
        // the small column, "Videos per page" beside "without Shorts" broke
        // over two lines.
        b.pair(
            b.t(key::PLAYER_RANKING_PAGE_SIZE),
            self.ranking().per_page.get().to_string(),
            Target::RankingPerPage,
            "ranking-per-page",
        );
        b.pair(
            b.t(key::PLAYER_CATALOG_LAYOUT),
            b.t(self.ranking().layout.key()),
            Target::RankingLayout,
            "ranking-layout",
        );
        // ITS SHORTS, a ring — drawn on a list with nothing on it too: the
        // filter that left it empty is turned back here.
        let shorts = b.t(crate::video_metrics_words::shorts_key(shown.shorts));
        b.pair(
            b.t(key::VIDEO_METRICS_FILTER_SHORTS),
            shorts,
            Target::RankingShorts,
            "ranking-shorts",
        );
        // NOTHING ON IT, said for what it is: the Shorts turned away — as the
        // page drawn was READ, which for a moment after a turn is the filter
        // before it — or nothing set aside yet, or nothing ranked at all. The
        // videos set aside say it of themselves: the scans have nothing to do
        // with what the person set aside.
        if listed.total == 0 {
            let read_under = listed.asked.filter.shorts;
            if read_under != Shorts::Any {
                let sentence = match list {
                    List::Ranking => key::PLAYER_RANKING_EMPTY_FILTERED,
                    List::SetAside => key::PLAYER_RANKING_ASIDE_EMPTY_FILTERED,
                };
                let filter = b.t(crate::video_metrics_words::shorts_key(read_under));
                let said = b.texts.t_with(sentence, &[("shorts", &filter)]);
                crate::player_find::note_said(&said, b);
            } else if list == List::SetAside {
                b.note(key::PLAYER_RANKING_ASIDE_EMPTY);
            } else {
                b.note(key::PLAYER_RANKING_EMPTY);
            }
            return;
        }
        let from = u64::from(listed.asked.page) * u64::from(listed.asked.per_page) + 1;
        let to = from + u64::try_from(listed.rows.len()).unwrap_or(0);
        // THE TITLE JUST ABOVE THE VIDEOS, with the arrows of the pages at
        // its edge (the owner, 2026-11-10) — of the page ASKED, so they never
        // jump when its reading lands; "1–25 of 230" says the page already,
        // and takes no mark. One page draws no arrow.
        let arrows = crate::paging::arrows(shown.page, self.ranking().pages(list));
        let controls = page_arrows(arrows);
        b.heading_paged(
            b.texts.t_args(
                key::PLAYER_CATALOG_SHOWN,
                &[
                    ("from", n(from)),
                    ("to", n(to.saturating_sub(1).max(from))),
                    ("total", n(listed.total)),
                ],
            ),
            &controls,
            crate::paging::LIST_TITLE,
        );
        let now = now_s();
        // EACH AS READ, with the marks this door sent on it: a glyph lights
        // at the click, not when the catalogue is read again.
        let tiles: Vec<Tile> = listed
            .rows
            .iter()
            .map(|v| self.ranking().as_marked(v))
            .map(|v| Tile {
                picture: v
                    .thumb_file
                    .as_deref()
                    .and_then(|f| self.ranking().posters.of_file(f)),
                glyph: tokens::icon::PLAYER_LANE_PICTURE,
                title: v.title.clone(),
                under: under(&v, self.ranking().layout, now, b.texts),
                target: Target::Ranked(v.id.clone()),
                spot: spot_of(&v.id),
                more: Some(Control {
                    glyph: tokens::icon::ELLIPSIS,
                    lit: false,
                    target: Target::RankedMore(v.id.clone()),
                    about: MORE,
                }),
            })
            .collect();
        let shape = Shape {
            cells: tokens::space::VIDEOMETRICS_THUMBCELLS,
            lines: tokens::space::VIDEOMETRICS_THUMBLINES,
            titles: tokens::space::VIDEOMETRICS_TITLELINES,
        };
        match self.ranking().layout {
            Layout::List => crate::player_posters::strip(&tiles, shape, b),
            Layout::Grid => crate::player_posters::tiled(&tiles, shape, b),
        }
        // AND THE ARROWS AGAIN AT THE LIST'S END, with the page's mark.
        if let Some(mark) = crate::paging::mark(shown.page, self.ranking().pages(list), b.texts) {
            b.pager(mark, &controls, crate::paging::LIST_PAGER);
        }
    }

    fn ranked_video_rows(&self, b: &mut Builder<'_>) {
        let Some(id) = self.ranking().chosen.as_ref() else {
            b.note(key::PLAYER_PAGE_GONE);
            return;
        };
        let whole = self.ranking().video.as_ref().filter(|(v, _)| v == id);
        if let Some((_, Err(why))) = whole {
            self.ranking_refused(*why, b);
            return;
        }
        if let Some((_, Ok(None))) = whole {
            b.note(crate::video_metrics_words::why_key(Why::NotFound));
            return;
        }
        let Some(v) = self.ranking().row_of(id) else {
            b.note(key::PLAYER_RANKING_READING);
            return;
        };
        if let Some((key, size)) = v
            .thumb_file
            .as_deref()
            .and_then(|f| self.ranking().posters.of_file(f))
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
            // THE RATING IS A RING, its value beside it; a row whose video is
            // so already says what pressing it does now — taken out of the
            // set-aside list, its own never-delete mark taken off.
            if act == RankedAct::Rate {
                b.pair(
                    b.t(act.key()),
                    b.t(v.rating.map_or(
                        key::VIDEO_METRICS_RATING_NONE,
                        crate::video_metrics_words::rating_key,
                    )),
                    Target::RankedRating,
                    act.spot(),
                );
                continue;
            }
            let on = match act {
                RankedAct::SetAside => v.set_aside_at_s.is_some(),
                RankedAct::Forever => v.forever == Forever::ByVideo,
                RankedAct::Hide => v.hidden,
                _ => false,
            };
            b.pair(
                b.t(act.key_when(on)),
                String::new(),
                Target::RankedDo(act),
                act.spot(),
            );
        }
    }

    fn open_ranked(
        &mut self,
        id: &VideoId,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let from = self.spot_here(state, columns, texts);
        self.ranking_mut().chosen = Some(id.clone());
        self.ranking_mut().video = None;
        self.push(PageId::RankedVideo, from);
        self.settle_in(state, columns, texts);
        // THE CURSOR ON WHAT IS DONE MOST — playing it — and not on the facts
        // above it, which `RET` does nothing on.
        self.follow(RankedAct::PlayNow.spot(), state, columns, texts);
    }

    fn ranking_pressed(
        &mut self,
        key: RankingKey,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        // A QUICK MARK on the video under the cursor — or on the one whose
        // own page is open.
        if let RankingKey::Quick(q) = key {
            return match video_here(self, state, columns, texts) {
                Some(id) => quick(self, &id, q, texts),
                None => Errand::Nothing,
            };
        }
        // THE REST ARE A LIST'S: the ranking's, or the videos set aside.
        let Some(list) = List::of(self.page()) else {
            return Errand::Nothing;
        };
        // ITS SHORTS, turned on: with them, without them, only them.
        if key == RankingKey::Shorts {
            shorts_turned(self, list, true, state, columns, texts);
            return Errand::Nothing;
        }
        let last = self.ranking().pages(list).saturating_sub(1);
        let r = self.ranking_mut();
        let page = r.shown(list).page;
        match key {
            RankingKey::Earlier => r.shown_mut(list).page = page.saturating_sub(1),
            RankingKey::Later => r.shown_mut(list).page = (page + 1).min(last),
            RankingKey::First => r.shown_mut(list).page = 0,
            RankingKey::Last => r.shown_mut(list).page = last,
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
            // THE SHORTS AND A QUICK MARK were answered above.
            RankingKey::Shorts | RankingKey::Quick(_) => return Errand::Nothing,
        }
        // A NEW PAGE STARTS AT ITS TITLE, brought to the top of the door and
        // the cursor on its `›` (the owner, 2026-11-10) — `RET` turns again;
        // on the last page, on its `‹`. The page held stays drawn while the
        // next one is read, so the title is there now. A key that turned
        // nothing — the first page's `[` — moves nothing.
        if self.ranking().shown(list).page != page {
            self.bring_to_top(
                crate::paging::LIST_TITLE,
                &TURNED_ONTO,
                state,
                columns,
                texts,
            );
        }
        self.settle_in(state, columns, texts);
        Errand::Nothing
    }

    fn ranking_turned(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        let errand = match (target, press) {
            (Target::Ranked(id), Press::Enter | Press::Add | Press::PlayNow) => {
                let v = self.ranking().row_of(id)?;
                put(&v, press == Press::PlayNow, texts)
            }
            (Target::RankedMore(id), Press::Enter) => {
                let id = id.clone();
                self.open_ranked(&id, state, columns, texts);
                Errand::Nothing
            }
            (Target::RankingLayout, Press::Enter | Press::Less | Press::More) => {
                return Some(self.ranking_pressed(RankingKey::Layout, state, columns, texts));
            }
            // ITS SHORTS' RING: `RET` and `→` on, `←` back.
            (Target::RankingShorts, Press::Enter | Press::Less | Press::More) => {
                let list = List::of(self.page())?;
                shorts_turned(self, list, press != Press::Less, state, columns, texts);
                Errand::Nothing
            }
            (Target::RankingPage, Press::Less) => {
                return Some(self.ranking_pressed(RankingKey::Earlier, state, columns, texts));
            }
            // AN ARROW OF THE PAGES, beside the title or at the list's end.
            (Target::RankingTurn(on), Press::Enter) => {
                let key = if *on {
                    RankingKey::Later
                } else {
                    RankingKey::Earlier
                };
                return Some(self.ranking_pressed(key, state, columns, texts));
            }
            // HOW MANY A PAGE HOLDS: `RET` and `→` on, `←` back — the page
            // that holds the first video on screen asked, and its title
            // brought to the top, as after a turn. THE SIZE IS BOTH LISTS',
            // and so is the turn: the other list keeps the page holding the
            // first video it showed, or a page number kept under the new
            // size showed it other videos — page 2 of the ones set aside,
            // 50 on at 25 a page, 200 on at 100.
            (Target::RankingPerPage, Press::Enter | Press::Less | Press::More) => {
                List::of(self.page())?;
                let r = self.ranking_mut();
                let from = r.per_page;
                r.per_page = from.turned(press != Press::Less);
                let to = r.per_page.get();
                for list in [List::Ranking, List::SetAside] {
                    let page = r.shown(list).page;
                    r.shown_mut(list).page = crate::paging::resized(page, from.get(), to);
                }
                self.bring_to_top(
                    crate::paging::LIST_TITLE,
                    &TURNED_ONTO,
                    state,
                    columns,
                    texts,
                );
                self.settle_in(state, columns, texts);
                Errand::Nothing
            }
            // `RET` ON THE PAGE goes on, and from the last back to the first.
            (Target::RankingPage, Press::Enter | Press::More) => {
                let list = List::of(self.page())?;
                let r = self.ranking();
                let key = if press == Press::Enter && r.shown(list).page + 1 >= r.pages(list) {
                    RankingKey::First
                } else {
                    RankingKey::Later
                };
                return Some(self.ranking_pressed(key, state, columns, texts));
            }
            (Target::RankedDo(act), Press::Enter) => self.ranked_do(*act, state, columns, texts),
            // A QUICK MARK'S GLYPH, clicked: at once.
            (Target::RankedQuick(id, q), Press::Enter) => {
                let id = id.clone();
                quick(self, &id, *q, texts)
            }
            // THE RATING'S RING on a video's own page: `RET` and `→` on,
            // `←` back.
            (Target::RankedRating, Press::Enter | Press::Less | Press::More) => {
                let id = self.ranking().chosen.clone()?;
                rating_ring(self.ranking_mut(), &id, press != Press::Less, texts)
            }
            _ => return None,
        };
        Some(errand)
    }

    fn ranked_do(
        &mut self,
        act: RankedAct,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(id) = self.ranking().chosen.clone() else {
            return Errand::Nothing;
        };
        let Some(v) = self.ranking().row_of(&id).map(Cow::into_owned) else {
            return Errand::Said(
                texts
                    .t(crate::video_metrics_words::why_key(Why::NotFound))
                    .into(),
            );
        };
        let _ = (state, columns);
        match act {
            RankedAct::PlayNow | RankedAct::Add => put(&v, act == RankedAct::PlayNow, texts),
            RankedAct::Kiosk => Errand::Kiosk(v.url),
            RankedAct::Nyxt => Errand::Nyxt(v.url),
            RankedAct::Download => Errand::Ranking(Act::Download {
                url: v.url,
                title: v.title,
            }),
            // NO ROW IS DRAWN AS THIS ONE: the rating's row is its ring
            // (`Target::RankedRating`, `ranking_turned`), which the arrows
            // turn too. The act is here for the match to be whole, and does
            // what `RET` on that row does — through the same ring.
            RankedAct::Rate => rating_ring(self.ranking_mut(), &id, true, texts),
            RankedAct::SetAside => quick(self, &id, Quick::SetAside, texts),
            RankedAct::Forever => quick(self, &id, Quick::Forever, texts),
            RankedAct::Hide => quick(self, &id, Quick::Hide, texts),
            RankedAct::Delete => quick(self, &id, Quick::Delete, texts),
        }
    }

    fn ranking_told(&mut self, outcome: &Outcome) -> bool {
        let gone = match outcome {
            // HIDDEN, FROM THE VIDEOS SET ASIDE: it stays on that list —
            // which holds the hidden ones — and its page stays, as a mark's.
            Outcome::Hidden(_, _, Ok(()))
                if self.ranking().listed_list() == Some(List::SetAside) =>
            {
                self.ranking_mut().forget();
                return true;
            }
            Outcome::Hidden(id, _, Ok(())) | Outcome::Deleted(id, _, Ok(())) => id,
            // A MARK keeps the page where it is: what is held is read again,
            // and the row says what the catalogue says now — the mark on it
            // until then. One REFUSED leaves the row as the catalogue has
            // it.
            Outcome::Marked(mark, _, done) => {
                let r = self.ranking_mut();
                r.mark_told(mark, done.is_ok());
                if done.is_ok() {
                    r.forget();
                }
                return true;
            }
            _ => return false,
        };
        self.ranking_mut().forget();
        if self.page() == PageId::RankedVideo
            && self.ranking().chosen.as_ref() == Some(gone)
            && self.levels.len() > 1
            && let Some(left) = self.levels.pop()
        {
            self.ranking_mut().follow_owed = left.opened_from;
        }
        true
    }
}

/// THE TWO ARROWS OF THE RANKING'S PAGES, `[earlier, later]`, each a control
/// named by the command its key runs (`chrome::hint_sentence` says it) —
/// `None` for one that cannot move, whose place stays empty
/// ([`crate::paging::arrows`]).
fn page_arrows(arrows: Option<[bool; 2]>) -> [Option<(char, Target, &'static str)>; 2] {
    let Some([earlier, later]) = arrows else {
        return [None, None];
    };
    [
        earlier.then_some((
            tokens::icon::LIST_PAGE_EARLIER,
            Target::RankingTurn(false),
            EARLIER,
        )),
        later.then_some((
            tokens::icon::LIST_PAGE_LATER,
            Target::RankingTurn(true),
            LATER,
        )),
    ]
}

/// The command of the page before, which its arrow says.
const EARLIER: &str = "player-ranking-earlier";

/// The command of the page after.
const LATER: &str = "player-ranking-later";

/// The cells of the title's arrows the cursor goes to after a turn: the
/// page after's, and the page before's on the last page, which has none
/// after (`player_rows::Builder::heading_paged` spots each `control-<id>`).
const TURNED_ONTO: [&str; 2] = [
    "control-player-ranking-later",
    "control-player-ranking-earlier",
];

/// What the view's plugged ranking is missing for: the ranking's page is
/// installed in every view whose methods here are called (the seam:
/// `crate::player_seam`).
const PLUGGED: &str = "the view holds the ranking's page";

/// THE RANKING of the video catalogue, as a page of the player
/// (`crate::player_seam::Page`): what the view asked of the ranking at each
/// of its places, asked here.
pub struct Ranked;

/// The ranking's page, as the binary installs it.
pub static PAGE: Ranked = Ranked;

impl crate::player_seam::Page for Ranked {
    fn name(&self) -> &'static str {
        "ranking"
    }

    fn state(&self, this_machine: bool) -> Box<dyn std::any::Any> {
        Box::new(if this_machine {
            Ranking::of_this_machine()
        } else {
            Ranking::default()
        })
    }

    fn title(&self, view: &View, page: PageId, b: &mut Builder<'_>) -> bool {
        // THE TWO LISTS carry their layout beside their title.
        if List::of(page).is_none() {
            return false;
        }
        view.ranking_title(page, b);
        true
    }

    fn rows(&self, view: &View, page: PageId, _: Option<&State>, b: &mut Builder<'_>) -> bool {
        // THE RANKING, THE VIDEOS SET ASIDE AND A VIDEO'S OWN PAGE: the video
        // catalogue's, read by this door itself.
        match page {
            PageId::Ranking => view.ranking_rows(List::Ranking, b),
            PageId::SetAside => view.ranking_rows(List::SetAside, b),
            PageId::RankedVideo => view.ranked_video_rows(b),
            _ => return false,
        }
        true
    }

    fn look(
        &self,
        view: &mut View,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        // THE RANKING'S PAGES read the video catalogue, off the loop in a
        // door that has one.
        view.ranking_look(state, columns, texts);
    }

    fn pressed(
        &self,
        view: &mut View,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        match press {
            Press::Ranking(key) => Some(view.ranking_pressed(key, state, columns, texts)),
            _ => None,
        }
    }

    fn turned(
        &self,
        view: &mut View,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        // A VIDEO, ITS `⋯`, THE PAGE AND THE LAYOUT, and what a video's own
        // page does.
        view.ranking_turned(target, press, state, columns, texts)
    }

    fn picture<'v>(&self, view: &'v View, key: &str) -> Option<&'v crate::player_find::Picture> {
        // A THUMBNAIL OF THE RANKING, there with no player running.
        view.ranking().posters.picture(key)
    }

    fn told(&self, view: &mut View, answer: &crate::player::Told) -> Option<bool> {
        // A VIDEO HIDDEN OR DELETED: its own page closes — but not one hidden
        // from the videos set aside, which hold the hidden ones — and a MARK
        // keeps it; either way the list is read again.
        match answer {
            crate::player::Told::Ranking(outcome) => Some(view.ranking_told(outcome)),
            _ => None,
        }
    }

    fn said(
        &self,
        answer: &crate::player::Told,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<crate::chrome::Named> {
        match answer {
            crate::player::Told::Ranking(outcome) => Some(said(outcome, texts)),
            _ => None,
        }
    }

    fn off_the_loop(&self, errand: Errand) -> Result<crate::player_seam::OffTheLoop, Errand> {
        // A VIDEO OF THE RANKING hidden, deleted, downloaded or marked, OFF
        // THE LOOP — a write waits for a scan that holds the catalogue — and
        // said when it is done (`Told::Ranking`).
        match errand {
            Errand::Ranking(act) => Ok(crate::player_seam::OffTheLoop {
                about: "a video of the ranking, hidden, deleted, downloaded or marked",
                thread: "player-ranking-act",
                work: Box::new(move || crate::player::Told::Ranking(carry(act))),
                unstarted: None,
            }),
            other => Err(other),
        }
    }

    fn scale_is(&self, view: &mut View, scale: u32) -> bool {
        // THE THUMBNAILS, decoded to the screen's pixels.
        let changed = view.ranking().posters.scale != scale;
        view.ranking_mut().posters.scale = scale;
        changed
    }

    fn detached(&self, view: &mut View, outbox: &crate::player_window::Outbox) {
        // THE VIDEO CATALOGUE IT WAS HANDED — a photograph's `--ranking`.
        if let Some(file) = outbox.ranking.clone() {
            view.ranking_at(file);
        }
    }
}

/// The videos a test hands the ranking — for the tests of this crate and above.
#[cfg(any(test, feature = "test-support"))]
pub mod fixture {
    use super::*;
    use ronoctua_video_metrics::ChannelId;

    /// A video of the ranking, `n` best.
    pub fn video(n: u32) -> RankedVideo {
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
            rating: None,
            set_aside_at_s: None,
            forever: ronoctua_video_metrics::read::Forever::No,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    pub(crate) use super::fixture::video;
    use super::*;
    use crate::player::fixture;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a language")
    }

    /// A view on the ranking, handed `count` videos — AS A LIST, which the
    /// tests of a video's row, its `⋯` and its keys are about; the ranking
    /// opens on the thumbnails (`opens_on_the_thumbnails`), and a test of
    /// the grid turns to it.
    fn on_the_ranking(count: u32) -> View {
        let mut v = View::blank(None).plugging(&PAGE);
        *v.ranking_mut() = Ranking::at(None);
        v.ranking_mut().layout = Layout::List;
        v.ranking_mut()
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

    /// The mark errand `e` sends, told written to view `v` as the catalogue's
    /// door tells it — not read back yet.
    fn written(v: &mut View, e: Errand) -> Mark {
        let Errand::Ranking(Act::Mark { mark, title, .. }) = e else {
            panic!("no mark: {e:?}");
        };
        assert!(v.ranking_told(&Outcome::Marked(mark.clone(), title, Ok(()))));
        mark
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
        assert_eq!(v.ranking().chosen, Some(video(2).id));
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
        v.ranking_mut().path = Some(PathBuf::from("/nowhere/catalog.sqlite"));
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
        v.ranking_mut().given(
            (0..crate::paging::PerPage::default().get())
                .map(video)
                .collect(),
            60,
        );
        assert_eq!(v.ranking().pages(List::Ranking), 3);
        v.key(Press::Ranking(RankingKey::Earlier), Some(&s), 100, &t);
        assert_eq!(v.ranking().ranking.page, 0);
        v.key(Press::Ranking(RankingKey::Last), Some(&s), 100, &t);
        assert_eq!(v.ranking().ranking.page, 2);
        v.key(Press::Ranking(RankingKey::Later), Some(&s), 100, &t);
        assert_eq!(v.ranking().ranking.page, 2, "past the last");
        v.key(Press::Ranking(RankingKey::First), Some(&s), 100, &t);
        assert_eq!(v.ranking().ranking.page, 0);
        v.key(Press::Ranking(RankingKey::Layout), Some(&s), 100, &t);
        assert_eq!(v.ranking().layout, Layout::Grid);
        let rows = v.rows(Some(&s), 100, &t);
        let shelf = row_at(&rows, &format!("shelf-{}", spot_of(&video(0).id)));
        // THE ⋯ LINE is a line of its own, under the titles — the score's,
        // the four arrows of its rating beside it — and the channel's under
        // it, the marks at its edge: a click on the picture takes the video,
        // and only a glyph acts.
        let more_line = (shelf..rows.len())
            .find(|&n| rows[n].cells.iter().any(|c| c.about == Some(MORE)))
            .expect("a line with the ⋯");
        assert!(rows[more_line].heading, "the ⋯'s line is the tile's row");
        assert!(rows[shelf].cells.iter().all(|c| !c.control));
        let tiles = rows[more_line]
            .cells
            .iter()
            .filter(|c| c.about == Some(MORE))
            .count();
        assert!(tiles >= 3, "a shelf of three at least: {tiles}");
        let said = |n: usize, q: Quick| {
            rows[n]
                .cells
                .iter()
                .filter(|c| c.about == Some(q.command()))
                .count()
        };
        for r in Rating::ALL {
            assert_eq!(said(more_line, Quick::Rate(r)), tiles, "{r:?}");
        }
        let marks_line = more_line + 1;
        for q in [Quick::SetAside, Quick::Hide, Quick::Delete, Quick::Forever] {
            assert_eq!(said(marks_line, q), tiles, "{q:?}");
        }
        assert!(
            rows[more_line..=marks_line]
                .iter()
                .flat_map(|r| &r.cells)
                .all(|c| c.control && c.width == tokens::space::PLAYER_QUEUEGLYPH)
        );
        // AND THEY LIGHT ONLY THEIR GLYPHS, as a click takes only them: the
        // words said under the titles light nothing, and the air under them
        // nothing.
        use crate::player_rows::{Lighting, lighting};
        let cells = |n: usize| -> Vec<(u16, u16)> {
            rows[n].cells.iter().map(|c| (c.from, c.width)).collect()
        };
        assert_eq!(lighting(&rows, more_line), Lighting::Cell(cells(more_line)));
        assert_eq!(
            lighting(&rows, marks_line),
            Lighting::Cell(cells(marks_line))
        );
        assert_eq!(lighting(&rows, marks_line + 1), Lighting::Cell(Vec::new()));
        v.key(Press::Ranking(RankingKey::Layout), Some(&s), 100, &t);
        assert_eq!(v.ranking().layout, Layout::List);
    }

    /// FAILS IF: a turned page of the ranking is not brought to its title —
    /// owed the top of the door once and once only, the cursor on the
    /// title's `›` (`RET` turns again), on its `‹` on the last page, which
    /// has no `›` — or `[` stops being the ranking's page before.
    #[test]
    fn a_turned_ranking_page_is_brought_to_its_title() {
        use crate::player::Target;
        let t = texts("en-GB");
        let s = fixture::idle();
        let mut v = on_the_ranking(3);
        v.ranking_mut().given(
            (0..crate::paging::PerPage::default().get())
                .map(video)
                .collect(),
            60,
        );
        assert_eq!(
            v.press_of_key(ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Char(
                '['
            ))),
            Some(Press::Ranking(RankingKey::Earlier))
        );
        assert_eq!(v.take_top(), None);
        v.key(Press::Ranking(RankingKey::Later), Some(&s), 100, &t);
        let rows = v.rows(Some(&s), 100, &t);
        let title = row_at(&rows, crate::paging::LIST_TITLE);
        assert_eq!(v.take_top(), Some(title), "the title was not owed the top");
        assert_eq!(v.take_top(), None, "owed twice");
        assert_eq!(v.cursor(), title);
        assert_eq!(
            rows[title].target,
            Target::RankingTurn(true),
            "not on its `›`"
        );
        // `RET` THERE TURNS AGAIN — onto the last page, on its `‹`.
        v.key(Press::Enter, Some(&s), 100, &t);
        assert_eq!(v.ranking().ranking.page, 2);
        let rows = v.rows(Some(&s), 100, &t);
        assert_eq!(v.take_top(), Some(title));
        assert_eq!(
            rows[title].target,
            Target::RankingTurn(false),
            "not on its `‹`"
        );
        // A KEY THAT TURNS NOTHING brings nothing.
        v.key(Press::Ranking(RankingKey::Later), Some(&s), 100, &t);
        assert_eq!(v.take_top(), None, "the last page's `]` moved the door");
    }

    /// FAILS IF: the arrows beside the ranking's list title stop turning its
    /// pages at a click, or draw an arrow that cannot move: no `‹` on the
    /// first page, no `›` on the last, none for a list of one page.
    #[test]
    fn the_ranking_title_turns_its_pages() {
        use crate::player::Target;
        let t = texts("pt-BR");
        let s = fixture::idle();
        let mut v = on_the_ranking(3);
        v.ranking_mut().given(
            (0..crate::paging::PerPage::default().get())
                .map(video)
                .collect(),
            60,
        );
        let turns = |rows: &[crate::player_rows::Row], n: usize| -> Vec<Target> {
            rows[n].cells.iter().map(|c| c.target.clone()).collect()
        };
        let rows = v.rows(Some(&s), 100, &t);
        let title = row_at(&rows, crate::paging::LIST_TITLE);
        assert_eq!(turns(&rows, title), [Target::RankingTurn(true)]);
        let later = rows[title].cells[0].from;
        assert!(
            v.clicked_at(title, later, &rows),
            "the `›` waits for a second click"
        );
        v.key(Press::Enter, Some(&s), 100, &t);
        assert_eq!(v.ranking().ranking.page, 1);
        let rows = v.rows(Some(&s), 100, &t);
        assert_eq!(
            turns(&rows, title),
            [Target::RankingTurn(false), Target::RankingTurn(true)]
        );
        assert_eq!(
            rows[title].cells[1].from, later,
            "the `›` moved when the `‹` came"
        );
        let mut one = on_the_ranking(3);
        let rows = one.rows(Some(&s), 100, &t);
        assert!(turns(&rows, row_at(&rows, crate::paging::LIST_TITLE)).is_empty());
        one.key(Press::Ranking(RankingKey::Later), Some(&s), 100, &t);
        assert_eq!(one.ranking().ranking.page, 0);
    }

    /// FAILS IF: the ranking's page size stops turning round 25, 50 and 100
    /// on its row — `RET` and `→` on, `←` back — or a new size is not asked
    /// of the catalogue, or its page loses the first video that was on
    /// screen; or THE OTHER LIST, which the size is also of, keeps its page
    /// number under the new size, and shows other videos when it is opened.
    #[test]
    fn the_ranking_page_size_is_a_ring() {
        let t = texts("en-GB");
        let s = fixture::idle();
        let mut v = on_the_ranking(3);
        v.ranking_mut().given(
            (0..crate::paging::PerPage::default().get())
                .map(video)
                .collect(),
            230,
        );
        v.ranking_mut().ranking.page = 3;
        v.ranking_mut().aside.page = 2;
        let rows = v.rows(Some(&s), 100, &t);
        v.top_mut().cursor = row_at(&rows, "ranking-per-page");
        v.key(Press::More, Some(&s), 100, &t);
        assert_eq!(v.ranking().per_page.get(), 50);
        assert_eq!(
            v.ranking().ranking.page,
            1,
            "75 is on the second page of fifty"
        );
        assert_eq!(
            v.ranking().aside.page,
            1,
            "50 set aside is on the second page of fifty"
        );
        assert_eq!(v.ranking().asked(List::Ranking).per_page, 50);
        assert!(v.take_top().is_some(), "a new size owes its title the top");
        let rows = v.rows(Some(&s), 100, &t);
        v.top_mut().cursor = row_at(&rows, "ranking-per-page");
        v.key(Press::Less, Some(&s), 100, &t);
        assert_eq!(v.ranking().per_page.get(), 25);
        let rows = v.rows(Some(&s), 100, &t);
        v.top_mut().cursor = row_at(&rows, "ranking-per-page");
        v.key(Press::Enter, Some(&s), 100, &t);
        v.top_mut().cursor = row_at(&v.rows(Some(&s), 100, &t), "ranking-per-page");
        v.key(Press::Enter, Some(&s), 100, &t);
        assert_eq!(v.ranking().per_page.get(), 100);
        assert_eq!(
            v.ranking().aside.page,
            0,
            "50 set aside is on the first page of a hundred"
        );
    }

    /// FAILS IF: the ranking's list does not end with the arrows of its
    /// pages and its mark — "2 of 3" — or they wear a title's ink (a section
    /// would begin there, `section_ring`), or the cursor stops there but on
    /// the arrows; or a list of one page ends with them.
    #[test]
    fn the_ranking_ends_with_its_arrows() {
        use crate::player::Target;
        let t = texts("en-GB");
        let s = fixture::idle();
        for layout in [Layout::List, Layout::Grid] {
            let mut v = on_the_ranking(3);
            v.ranking_mut().layout = layout;
            v.ranking_mut().given(
                (0..crate::paging::PerPage::default().get())
                    .map(video)
                    .collect(),
                60,
            );
            v.ranking_mut().ranking.page = 1;
            let rows = v.rows(Some(&s), 100, &t);
            let end = row_at(&rows, crate::paging::LIST_PAGER);
            assert_eq!(end, rows.len() - 1, "{layout:?}: not at the list's end");
            let pager = &rows[end];
            assert!(pager.heading && pager.walked_onto(), "{layout:?}");
            assert!(
                pager
                    .fields
                    .iter()
                    .all(|f| f.color != tokens::color::TEXT_HEADING),
                "{layout:?}: the end wears a title's ink"
            );
            assert_eq!(pager.fields[0].text, "2 of 3");
            assert_eq!(
                pager
                    .cells
                    .iter()
                    .map(|c| c.target.clone())
                    .collect::<Vec<_>>(),
                [Target::RankingTurn(false), Target::RankingTurn(true)]
            );
        }
        let one = on_the_ranking(3);
        let rows = one.rows(Some(&s), 100, &t);
        assert!(!rows.iter().any(|r| r.spot == crate::paging::LIST_PAGER));
    }

    /// FAILS IF: the list's `⋯` is taken — by a click, or said under the
    /// pointer — on a line of the video's row it is not drawn on, or the
    /// row's lines stop lighting from its words on, each line the same; or a
    /// click on a line's words, where no glyph is, stops being a click on the
    /// row.
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
            // UNDER IT, a mark of that line's own may stand — the ∞ at the
            // row's edge — and never the `⋯`.
            assert_ne!(
                crate::player_rows::control_under(&rows, l, more.from),
                Some(MORE),
                "the ⋯ said on line {l}, under it"
            );
            // A CLICK ON THE WORDS IS A CLICK ON THE ROW: the first chooses
            // it, the second queues it — and the video's page never opens.
            assert_eq!(
                crate::player_rows::control_under(&rows, l, words),
                None,
                "a glyph on line {l}'s words"
            );
            v.follow(&spot_of(&video(0).id), Some(&s), 100, &t);
            assert!(!v.clicked_at(l, words, &rows), "line {l} acted at once");
            assert!(v.clicked_at(l, words, &rows), "the second click");
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
        v.ranking_mut().layout = Layout::Grid;
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
                Want::Page(v.ranking().asked(List::Ranking)),
                Ok(Got::Page(Listed {
                    asked: v.ranking().asked(List::Ranking),
                    rows: now,
                    total: 9,
                })),
            ));
        v.ranking_mut().worker = Some(worker);
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
        let mut v = View::blank(None).plugging(&PAGE);
        v.wake = Some(crate::wake::Wake::new(move || {
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
            matches!(v.ranking().listed, Some(Err(Why::NotThere))),
            "{:?}",
            v.ranking().listed
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
            matches!(v.ranking().listed, Some(Ok(ref l)) if l.total == 0),
            "{:?}",
            v.ranking().listed
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
        let mut v = View::blank(None).plugging(&PAGE);
        v.ranking_at(path);
        v.key(Press::Go(PageId::Ranking), None, 100, &t);
        let asked = v.ranking().asked(List::Ranking);
        assert!(v.ranking().holds(&asked));
        assert!(v.ranking_told(&Outcome::Hidden(video(1).id, video(1).title, Ok(()))));
        assert!(!v.ranking().holds(&asked), "the page is held as it was");
        v.settle_in(None, 100, &t);
        assert!(v.ranking().holds(&asked), "read again");
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
            let mut v = View::blank(None).plugging(&PAGE);
            *v.ranking_mut() = Ranking::at(None);
            v.key(Press::Go(PageId::Ranking), Some(&s), 100, &t);
            let said = said_on(&v.rows(Some(&s), 100, &t));
            let none = t.t_with(
                key::PLAYER_RANKING_NONE,
                &[(
                    "keys",
                    v.ranking().opens_with.as_deref().unwrap_or_default(),
                )],
            );
            let first = crate::chrome::note_lines(&none, 40)[0].clone();
            assert!(said.contains(first.trim()), "{language}: {said}");
            v.ranking_mut().listed = Some(Err(Why::Unreadable));
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
            // EMPTY WITH ITS SHORTS IN, nothing ranks; with them left out —
            // the ranking's first filter — the sentence says the filter.
            v.ranking_mut().ranking.shorts = Shorts::Any;
            v.ranking_mut().given(Vec::new(), 0);
            let said = said_on(&v.rows(Some(&s), 100, &t));
            assert!(
                said.contains(
                    crate::chrome::note_lines(&t.t(key::PLAYER_RANKING_EMPTY), 40)[0].trim()
                ),
                "{language}: {said}"
            );
            v.ranking_mut().ranking.shorts = Shorts::Without;
            v.ranking_mut().given(Vec::new(), 0);
            let said = said_on(&v.rows(Some(&s), 100, &t));
            let filtered = t.t_with(
                key::PLAYER_RANKING_EMPTY_FILTERED,
                &[(
                    "shorts",
                    &t.t(crate::video_metrics_words::shorts_key(Shorts::Without)),
                )],
            );
            assert!(
                said.contains(crate::chrome::note_lines(&filtered, 40)[0].trim()),
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
        let ranking = Ranking::default();
        let page = |n: u32| {
            let mut r = Ranking::default();
            r.ranking.page = n;
            Want::Page(r.asked(List::Ranking))
        };
        let got = absent.handle(vec![Want::Page(ranking.asked(List::Ranking))], false);
        assert!(
            matches!(got.as_slice(), [(_, Err(Why::NotThere))]),
            "{got:?}"
        );
        let path = dir.join("video-metrics").join("catalog.sqlite");
        committed(&path);
        let mut r = Reader::new(path.clone());
        let got = r.handle(vec![page(1), page(0)], false);
        assert_eq!(got.len(), 1, "two asks of a page read twice");
        assert!(
            matches!(&got[0], (w, Ok(Got::Page(l))) if *w == page(0) && l.total == 0 && l.rows.is_empty()),
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
            got.iter().any(|(w, _)| *w == page(0))
                && got.iter().any(|(w, _)| *w == Want::Video(video(1).id)),
            "{got:?}"
        );
        let mut v = View::blank(None).plugging(&PAGE);
        v.ranking_at(path);
        v.key(Press::Go(PageId::Ranking), None, 100, &texts("en-GB"));
        assert!(matches!(v.ranking().listed, Some(Ok(ref l)) if l.total == 0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a page that shrank under the cursor — its videos hidden in
    /// another door — stays past the last one, empty.
    #[test]
    fn a_page_past_the_last_goes_back_to_the_last() {
        for list in [List::Ranking, List::SetAside] {
            let mut r = Ranking::at(None);
            r.shown_mut(list).page = 3;
            let asked = r.asked(list);
            r.keep(
                Want::Page(asked.clone()),
                Ok(Got::Page(Listed {
                    asked,
                    rows: Vec::new(),
                    total: u64::from(crate::paging::PerPage::default().get()) + 1,
                })),
            );
            assert_eq!(r.shown(list).page, 1, "{list:?}");
            let other = if list == List::Ranking {
                List::SetAside
            } else {
                List::Ranking
            };
            assert_eq!(r.shown(other).page, 0, "{list:?}: the other list moved");
        }
    }

    /// FAILS IF: a video's own page stops sending it to Nyxt — or sends
    /// another address — or a quick mark's key stops marking the video under
    /// the cursor as its glyph says: a rating, and the same rating taken off;
    /// set aside or taken out; kept for ever by its own mark even where its
    /// channel keeps it, and that mark taken off; hidden at once; deleted
    /// only after a question, "no" first. Or a mark with no catalogue to
    /// write is not said.
    #[test]
    fn the_quick_marks_do_what_they_say() {
        let t = texts("en-GB");
        let s = fixture::idle();
        let catalogue = PathBuf::from("/nowhere/catalog.sqlite");
        let on = |rated: Option<Rating>, aside: bool, forever: Forever| {
            let mut v = View::blank(None).plugging(&PAGE);
            *v.ranking_mut() = Ranking::at(Some(catalogue.clone()));
            // AS A LIST: the cursor follows a video's row.
            v.ranking_mut().layout = Layout::List;
            let mut one = video(1);
            one.rating = rated;
            one.set_aside_at_s = aside.then_some(1_700_000_000);
            one.forever = forever;
            v.ranking_mut().given(vec![video(0), one], 2);
            v.push(PageId::Ranking, None);
            v.follow(&spot_of(&video(1).id), Some(&s), 100, &t);
            v
        };
        // A QUICK MARK'S KEY, and the mark told written — the next composes
        // from it, as it would before the catalogue is read again.
        let mark_of = |v: &mut View, q: Quick| {
            let e = v.key(Press::Ranking(RankingKey::Quick(q)), Some(&s), 100, &t);
            written(v, e)
        };
        let id = video(1).id;
        let mut v = on(Some(Rating::Positive), true, Forever::ByChannel);
        assert_eq!(
            mark_of(&mut v, Quick::Rate(Rating::Positive)),
            Mark::Rating(id.clone(), None),
            "the rating it has is taken off"
        );
        assert_eq!(
            mark_of(&mut v, Quick::Rate(Rating::Negative)),
            Mark::Rating(id.clone(), Some(Rating::Negative))
        );
        assert_eq!(
            mark_of(&mut v, Quick::SetAside),
            Mark::SetAside(id.clone(), false)
        );
        assert_eq!(
            mark_of(&mut v, Quick::Forever),
            Mark::Keep(id.clone(), true),
            "its own mark, whatever its channel keeps"
        );
        let mut v = on(None, false, Forever::ByVideo);
        assert_eq!(
            mark_of(&mut v, Quick::SetAside),
            Mark::SetAside(id.clone(), true)
        );
        assert_eq!(
            mark_of(&mut v, Quick::Forever),
            Mark::Keep(id.clone(), false)
        );
        assert!(matches!(
            v.key(Press::Ranking(RankingKey::Quick(Quick::Hide)), Some(&s), 100, &t),
            Errand::Ranking(Act::Hide { id: hidden, .. }) if hidden == id
        ));
        assert_eq!(
            v.key(
                Press::Ranking(RankingKey::Quick(Quick::Delete)),
                Some(&s),
                100,
                &t
            ),
            Errand::Nothing,
            "deleted with no question"
        );
        let rows = v.rows(Some(&s), 100, &t);
        assert_eq!(rows.get(v.cursor()).map(|r| r.spot.as_str()), Some("no"));
        // ON A VIDEO'S OWN PAGE: its rows mark it, and Nyxt plays it.
        let mut v = on(None, false, Forever::No);
        v.key(Press::Ranking(RankingKey::More), Some(&s), 100, &t);
        assert_eq!(v.page(), PageId::RankedVideo);
        v.follow(RankedAct::Rate.spot(), Some(&s), 100, &t);
        let e = v.key(Press::Less, Some(&s), 100, &t);
        assert_eq!(
            written(&mut v, e),
            Mark::Rating(id.clone(), Some(Rating::VeryPositive)),
            "the ring turned back from no rating is the last one"
        );
        assert_eq!(
            mark_of(&mut v, Quick::Rate(Rating::VeryPositive)),
            Mark::Rating(id, None),
            "the rating just written is the one taken off"
        );
        v.follow(RankedAct::Nyxt.spot(), Some(&s), 100, &t);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 100, &t),
            Errand::Nyxt(video(1).url)
        );
        // NO CATALOGUE TO WRITE: said, at once.
        let mut v = on(None, false, Forever::No);
        v.ranking_mut().path = None;
        assert!(matches!(
            v.key(
                Press::Ranking(RankingKey::Quick(Quick::SetAside)),
                Some(&s),
                100,
                &t
            ),
            Errand::Said(_)
        ));
    }

    /// FAILS IF: a mark this door carries out is not written by the video
    /// catalogue's own door — a catalogue with no such video is not what it
    /// says — or a mark done closes the video's own page, or leaves what is
    /// held as it was read.
    #[test]
    fn a_mark_is_written_and_the_page_stays() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-ranking-mark-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("video-metrics").join("catalog.sqlite");
        committed(&path);
        let out = carry(Act::Mark {
            catalogue: path,
            mark: Mark::SetAside(video(1).id, true),
            title: video(1).title,
        });
        assert_eq!(
            out,
            Outcome::Marked(
                Mark::SetAside(video(1).id, true),
                video(1).title,
                Err(Why::NotFound)
            )
        );
        let _ = std::fs::remove_dir_all(&dir);
        let t = texts("en-GB");
        let s = fixture::idle();
        let mut v = on_the_ranking(3);
        v.follow(&spot_of(&video(1).id), Some(&s), 100, &t);
        v.key(Press::Ranking(RankingKey::More), Some(&s), 100, &t);
        assert!(v.ranking_told(&Outcome::Marked(
            Mark::Keep(video(1).id, true),
            video(1).title,
            Ok(())
        )));
        assert_eq!(v.page(), PageId::RankedVideo, "a mark closed the page");
        assert!(v.ranking().stale, "what is held is not read again");
    }

    /// FAILS IF: a mark this door carries out lands as another — set aside
    /// written as kept for ever, kept as set aside, hidden as a rating — or
    /// on nothing: each is read back off the video of a scratch catalogue
    /// it was put on, the others as they were, and each taken off again.
    #[test]
    fn every_mark_lands_on_its_video() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-ranking-marks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("video-metrics").join("catalog.sqlite");
        let one = video(1);
        crate::video_metrics_feed::holding_one(&path, &one.id, &one.title);
        // ITS MARKS, as the catalogue reads them back: rating, set aside,
        // kept for ever, hidden.
        let marks = || {
            let v = Catalog::open_ro(&path)
                .and_then(|c| c.video(&one.id))
                .expect("the catalogue read")
                .expect("the video planted")
                .row;
            (v.rating, v.set_aside_at_s.is_some(), v.forever, v.hidden)
        };
        let put = |mark: Mark| {
            assert_eq!(
                carry(Act::Mark {
                    catalogue: path.clone(),
                    mark: mark.clone(),
                    title: one.title.clone(),
                }),
                Outcome::Marked(mark.clone(), one.title.clone(), Ok(())),
                "{mark:?}"
            );
        };
        let id = || one.id.clone();
        assert_eq!(marks(), (None, false, Forever::No, false));
        put(Mark::Rating(id(), Some(Rating::VeryPositive)));
        assert_eq!(
            marks(),
            (Some(Rating::VeryPositive), false, Forever::No, false)
        );
        put(Mark::SetAside(id(), true));
        assert_eq!(
            marks(),
            (Some(Rating::VeryPositive), true, Forever::No, false)
        );
        put(Mark::Keep(id(), true));
        assert_eq!(
            marks(),
            (Some(Rating::VeryPositive), true, Forever::ByVideo, false)
        );
        put(Mark::Hidden(id(), true));
        assert_eq!(
            marks(),
            (Some(Rating::VeryPositive), true, Forever::ByVideo, true)
        );
        // AND TAKEN OFF, one by one.
        put(Mark::Rating(id(), None));
        assert_eq!(marks(), (None, true, Forever::ByVideo, true));
        put(Mark::SetAside(id(), false));
        assert_eq!(marks(), (None, false, Forever::ByVideo, true));
        put(Mark::Keep(id(), false));
        assert_eq!(marks(), (None, false, Forever::No, true));
        put(Mark::Hidden(id(), false));
        assert_eq!(marks(), (None, false, Forever::No, false));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: what a hide, a delete, a download or a mark came to stops
    /// being said — the refusal's reason included — in either language; or
    /// the video's title in it stops being told as the NAME it quotes, the
    /// one stretch a door draws in a name's ink (`Arg::Name`).
    #[test]
    fn what_was_done_is_said() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let title = "Ünïcode title";
            let id = || video(1).id;
            for o in [
                Outcome::Marked(
                    Mark::Rating(id(), Some(Rating::Negative)),
                    title.into(),
                    Ok(()),
                ),
                Outcome::Marked(Mark::Rating(id(), None), title.into(), Ok(())),
                Outcome::Marked(Mark::SetAside(id(), true), title.into(), Ok(())),
                Outcome::Marked(Mark::SetAside(id(), false), title.into(), Ok(())),
                Outcome::Marked(Mark::Keep(id(), true), title.into(), Ok(())),
                Outcome::Marked(Mark::Keep(id(), false), title.into(), Ok(())),
                Outcome::Marked(Mark::Hidden(id(), true), title.into(), Ok(())),
                Outcome::Marked(Mark::Hidden(id(), false), title.into(), Ok(())),
                Outcome::Marked(Mark::Keep(id(), true), title.into(), Err(Why::Busy)),
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
                let names: Vec<&str> = s.names().iter().map(|r| &s[r.clone()]).collect();
                assert_eq!(names, [title], "{language}: {s:?}");
            }
            let busy = said(
                &Outcome::Deleted(video(1).id, title.into(), Err(Why::Busy)),
                &t,
            );
            assert!(busy.contains(&t.t(crate::video_metrics_words::why_key(Why::Busy))));
        }
    }

    /// FAILS IF: the question a delete asks stops drawing the video's title
    /// in the ink a name in a question wears (`chrome::quoted_ink`) — on the
    /// question's own line, the rest of it in the question's ink — in
    /// either language, or a name's ink reaches any other stretch of it.
    #[test]
    fn the_delete_question_draws_the_video_as_a_name() {
        let s = fixture::idle();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let mut v = View::blank(None).plugging(&PAGE);
            *v.ranking_mut() = Ranking::at(Some(PathBuf::from("/nowhere/catalog.sqlite")));
            v.ranking_mut().layout = Layout::List;
            v.ranking_mut().given(vec![video(0), video(1)], 2);
            v.push(PageId::Ranking, None);
            v.follow(&spot_of(&video(1).id), Some(&s), 200, &t);
            v.key(
                Press::Ranking(RankingKey::Quick(Quick::Delete)),
                Some(&s),
                200,
                &t,
            );
            let asked = tokens::color::QUESTION_ASKED;
            let quoted = crate::chrome::quoted_ink(asked);
            assert_ne!(quoted, asked, "a name in a question wears its own ink");
            let rows = v.rows(Some(&s), 200, &t);
            let line = rows
                .iter()
                .find(|r| r.fields.iter().any(|f| f.color == quoted))
                .unwrap_or_else(|| panic!("{language}: no name in the question"));
            let read: String = line.fields.iter().map(|f| f.text.as_str()).collect();
            assert_eq!(
                read,
                t.t_with(
                    key::ASK_PLAYER_RANKING_DELETE,
                    &[("title", &video(1).title)]
                ),
                "{language}: the question is not its sentence"
            );
            let named: Vec<&str> = line
                .fields
                .iter()
                .filter(|f| f.color == quoted)
                .map(|f| f.text.as_str())
                .collect();
            assert_eq!(named, [video(1).title.as_str()], "{language}");
            assert!(
                line.fields
                    .iter()
                    .all(|f| f.color == quoted || f.color == asked),
                "{language}: {:?}",
                line.fields
            );
        }
    }

    /// A view on the videos set aside, handed `rows`.
    fn on_the_aside(rows: Vec<RankedVideo>) -> View {
        let mut v = View::blank(None).plugging(&PAGE);
        *v.ranking_mut() = Ranking::at(Some(PathBuf::from("/nowhere/catalog.sqlite")));
        // AS A LIST: its tests are about a video's row.
        v.ranking_mut().layout = Layout::List;
        let total = u64::try_from(rows.len()).unwrap_or(u64::MAX);
        v.ranking_mut().given_aside(rows, total);
        v.push(PageId::SetAside, None);
        v
    }

    /// FAILS IF: the ranking opens with its Shorts in — a scan brings them
    /// by the dozen, and they push the long videos off its first pages — in
    /// the view a door builds (`Ranked::state`) or in this machine's; or the
    /// videos set aside open with theirs left out, which the person chose.
    #[test]
    fn shorts_are_left_out_by_default() {
        use crate::player_seam::Page as _;
        let built = PAGE.state(false);
        let built = built
            .downcast_ref::<Ranking>()
            .expect("the ranking's state");
        for r in [&Ranking::default(), &Ranking::of_this_machine(), built] {
            assert_eq!(r.asked(List::Ranking).filter.shorts, Shorts::Without);
            assert_eq!(r.asked(List::SetAside).filter.shorts, Shorts::Any);
            assert_eq!((r.ranking.page, r.aside.page), (0, 0));
        }
        assert_eq!(shorts_after(Shorts::Without, true), Shorts::Only);
        assert_eq!(shorts_after(Shorts::Only, true), Shorts::Any);
        assert_eq!(shorts_after(Shorts::Any, true), Shorts::Without);
        assert_eq!(shorts_after(Shorts::Without, false), Shorts::Any);
    }

    /// FAILS IF: the ranking opens anywhere but on its thumbnails (the owner,
    /// 2026-11-10: "o player abre em miniaturas") — by default, on this
    /// machine, or in the view a door builds (`Ranked::state`), the window's
    /// and the footer's column's — or a page opened so draws no shelf.
    #[test]
    fn opens_on_the_thumbnails() {
        use crate::player_seam::Page as _;
        let built = PAGE.state(false);
        let built = built
            .downcast_ref::<Ranking>()
            .expect("the ranking's state");
        for r in [&Ranking::default(), &Ranking::of_this_machine(), built] {
            assert_eq!(r.layout, Layout::Grid);
        }
        let t = texts("en-GB");
        let s = fixture::idle();
        let mut v = View::blank(None).plugging(&PAGE);
        *v.ranking_mut() = Ranking::at(None);
        v.ranking_mut().given((0..3).map(video).collect(), 3);
        v.push(PageId::Ranking, None);
        let rows = v.rows(Some(&s), 100, &t);
        row_at(&rows, &format!("shelf-{}", spot_of(&video(0).id)));
    }

    /// FAILS IF: turning a list's Shorts — `S`, or the ring's row — leaves
    /// the page held as the answer to the new filter on the SAME page, or the
    /// list where it was and not on its first page, or the other list turned
    /// with it; or the Shorts' row is gone from a list the filter left empty,
    /// where it is the one way back.
    #[test]
    fn a_filter_change_is_a_new_answer() {
        let t = texts("en-GB");
        let s = fixture::idle();
        // ON THE FIRST PAGE, which the turn keeps: only the filter tells the
        // page held from the one asked now.
        let mut v = on_the_ranking(3);
        let held = v.ranking().asked(List::Ranking);
        assert!(v.ranking().holds(&held));
        v.key(Press::Ranking(RankingKey::Shorts), Some(&s), 100, &t);
        assert_eq!(v.ranking().ranking.shorts, Shorts::Only);
        let now = v.ranking().asked(List::Ranking);
        assert_eq!(now.page, held.page);
        assert!(
            !v.ranking().holds(&now),
            "the old page held as the new answer"
        );
        assert_eq!(
            v.ranking().aside.shorts,
            Shorts::Any,
            "the other list turned"
        );
        // FROM THE LAST PAGE: a new filter is read from its first.
        let mut v = on_the_ranking(3);
        v.ranking_mut().given(
            (0..crate::paging::PerPage::default().get())
                .map(video)
                .collect(),
            60,
        );
        v.key(Press::Ranking(RankingKey::Last), Some(&s), 100, &t);
        assert_eq!(v.ranking().ranking.page, 2);
        v.ranking_mut().given(vec![video(0)], 60);
        v.key(Press::Ranking(RankingKey::Shorts), Some(&s), 100, &t);
        assert_eq!(v.ranking().ranking.page, 0, "a new filter, its first page");
        // THE RING'S ROW, `RET` on and `←` back — the cursor stays on it.
        let rows = v.rows(Some(&s), 100, &t);
        v.top_mut().cursor = row_at(&rows, "ranking-shorts");
        v.key(Press::Enter, Some(&s), 100, &t);
        assert_eq!(v.ranking().ranking.shorts, Shorts::Any);
        v.key(Press::Less, Some(&s), 100, &t);
        assert_eq!(v.ranking().ranking.shorts, Shorts::Only);
        assert_eq!(
            v.spot_here(Some(&s), 100, &t).as_deref(),
            Some("ranking-shorts")
        );
        // NOTHING WITH ONLY SHORTS: the row that turns it back is there.
        v.ranking_mut().given(Vec::new(), 0);
        let rows = v.rows(Some(&s), 100, &t);
        let at = row_at(&rows, "ranking-shorts");
        assert_eq!(rows[at].target, Target::RankingShorts);
        let said = said_on(&rows);
        assert!(
            said.contains(&t.t(key::VIDEO_METRICS_SHORTS_ONLY)),
            "{said}"
        );
    }

    /// FAILS IF: a list with nothing on it says it of the Shorts as just
    /// turned, and not as the page drawn was read — a page read without them
    /// said empty "with only Shorts" the moment `S` turns them — or the
    /// videos set aside, filtered empty, say what the ranking says: the
    /// scans have nothing to do with what the person set aside.
    #[test]
    fn an_empty_list_says_the_filter_it_was_read_under() {
        let s = fixture::idle();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let shorts = |v: &View| {
                let key = crate::video_metrics_words::shorts_key(v.ranking().ranking.shorts);
                t.t(key)
            };
            // THE SENTENCE WHOLE, across the lines it was wrapped in.
            let words = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
            let filtered = |v: &View, sentence: &str, shorts: &str| {
                let said = words(&said_on(&v.rows(Some(&s), 100, &t)));
                said.contains(&words(&t.t_with(sentence, &[("shorts", shorts)])))
            };
            // READ WITHOUT SHORTS, then turned to only them: the page drawn
            // is still the one read without.
            let mut v = on_the_ranking(0);
            let read_under = shorts(&v);
            v.key(Press::Ranking(RankingKey::Shorts), Some(&s), 100, &t);
            let turned = shorts(&v);
            assert_ne!(read_under, turned);
            assert!(
                filtered(&v, key::PLAYER_RANKING_EMPTY_FILTERED, &read_under),
                "{language}: not said as read"
            );
            assert!(
                !filtered(&v, key::PLAYER_RANKING_EMPTY_FILTERED, &turned),
                "{language}: said as just turned"
            );
            // THE VIDEOS SET ASIDE, their Shorts turned away: a sentence of
            // their own.
            let mut v = on_the_aside(Vec::new());
            v.ranking_mut().aside.shorts = Shorts::Without;
            v.ranking_mut().given_aside(Vec::new(), 0);
            let without = t.t(key::VIDEO_METRICS_SHORTS_WITHOUT);
            assert!(
                filtered(&v, key::PLAYER_RANKING_ASIDE_EMPTY_FILTERED, &without),
                "{language}: the set-aside list says the ranking's sentence"
            );
            assert!(
                !filtered(&v, key::PLAYER_RANKING_EMPTY_FILTERED, &without),
                "{language}: the scans named on the set-aside list"
            );
        }
    }

    /// FAILS IF: the videos set aside are asked as anything but those, the
    /// last set aside first, hidden ones included — or the page is titled
    /// as the ranking, or a list with nothing set aside draws a page of
    /// nothing, or the ranking's rows are drawn on it before its own are
    /// read; or hiding a video there closes its own page though the list
    /// still holds it.
    #[test]
    fn the_set_aside_list() {
        let s = fixture::idle();
        let r = Ranking::default();
        let asked = r.asked(List::SetAside);
        assert_eq!(asked.filter.set_aside, SetAside::Only);
        assert_eq!(asked.filter.hidden, Hidden::Any);
        assert_eq!(asked.filter.order, Order::NewestSetAside);
        let ranking = r.asked(List::Ranking);
        assert_eq!(
            ranking.filter.set_aside,
            SetAside::Any,
            "the ranking left the set aside out"
        );
        assert_eq!(ranking.filter.hidden, Hidden::Without);
        assert_eq!(ranking.filter.order, Order::Score);
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            // THE RANKING HELD, THE SET-ASIDE LIST NOT YET READ: it is being
            // read, and the ranking's videos are not on it.
            let mut v = on_the_ranking(3);
            v.key(Press::Go(PageId::SetAside), Some(&s), 100, &t);
            assert_eq!(v.page(), PageId::SetAside);
            let rows = v.rows(Some(&s), 100, &t);
            assert!(
                rows.iter().all(|r| !matches!(r.target, Target::Ranked(_))),
                "{language}: the ranking's rows on the set-aside list"
            );
            let said = said_on(&rows);
            assert!(
                said.contains(&t.t(key::PLAYER_PAGE_SET_ASIDE)),
                "{language}: {said}"
            );
            assert!(
                said.contains(&t.t(key::PLAYER_RANKING_READING)),
                "{language}: {said}"
            );
            // NOTHING SET ASIDE.
            let v = on_the_aside(Vec::new());
            let said = said_on(&v.rows(Some(&s), 100, &t));
            let empty = t.t(key::PLAYER_RANKING_ASIDE_EMPTY);
            assert!(
                said.contains(crate::chrome::note_lines(&empty, 40)[0].trim()),
                "{language}: {said}"
            );
            // ITS OWN VIDEOS, and none of them on the ranking.
            let mut one = video(7);
            one.set_aside_at_s = Some(1_700_000_000);
            let mut v = on_the_aside(vec![one.clone()]);
            let rows = v.rows(Some(&s), 100, &t);
            row_at(&rows, &spot_of(&one.id));
            v.key(Press::Go(PageId::Ranking), Some(&s), 100, &t);
            let rows = v.rows(Some(&s), 100, &t);
            assert!(
                rows.iter().all(|r| r.spot != spot_of(&one.id)),
                "{language}: the set-aside list's rows on the ranking"
            );
        }
        // HIDDEN FROM ITS OWN PAGE, opened from the set-aside list: the page
        // stays, and what is held is read again.
        let t = texts("en-GB");
        let mut one = video(1);
        one.set_aside_at_s = Some(1_700_000_000);
        let mut v = on_the_aside(vec![one.clone()]);
        v.follow(&spot_of(&one.id), Some(&s), 100, &t);
        v.key(Press::Ranking(RankingKey::More), Some(&s), 100, &t);
        assert_eq!(v.page(), PageId::RankedVideo);
        assert!(v.ranking_told(&Outcome::Hidden(one.id.clone(), one.title, Ok(()))));
        assert_eq!(v.page(), PageId::RankedVideo, "hiding closed its page");
        assert!(v.ranking().stale);
    }

    /// FAILS IF: a glyph beside a video's words stops doing at once what it
    /// shows — the arrow of its rating lit, and a click on it taking the
    /// rating off; another arrow rating it so; the bookmark filled while it is
    /// set aside, and taking it out; the ∞ lit while its channel keeps it,
    /// and a click putting the video's own mark; the eye hiding it at once;
    /// the bin asking first, "no" first — or, on the set-aside list, the eye
    /// of a hidden video stops showing it again and saying so.
    #[test]
    fn the_quick_glyphs_do_what_they_say() {
        let t = texts("en-GB");
        let s = fixture::idle();
        let mut one = video(1);
        one.rating = Some(Rating::Positive);
        one.set_aside_at_s = Some(1_700_000_000);
        one.forever = Forever::ByChannel;
        let id = one.id.clone();
        let on = |layout: Layout| {
            let mut v = View::blank(None).plugging(&PAGE);
            *v.ranking_mut() = Ranking::at(Some(PathBuf::from("/nowhere/catalog.sqlite")));
            v.ranking_mut().layout = layout;
            v.ranking_mut().given(vec![video(0), one.clone()], 2);
            v.push(PageId::Ranking, None);
            v
        };
        for layout in [Layout::List, Layout::Grid] {
            let mut v = on(layout);
            let rows = v.rows(Some(&s), 100, &t);
            // WHERE A GLYPH OF THE VIDEO IS DRAWN, and in which colour.
            let glyph = |q: Quick| {
                let target = Target::RankedQuick(id.clone(), q);
                rows.iter()
                    .enumerate()
                    .find_map(|(n, r)| {
                        let c = r.cells.iter().find(|c| c.target == target)?;
                        let f = r.fields.iter().find(|f| f.at == c.from)?;
                        Some((n, c.from, f.color, c.about))
                    })
                    .unwrap_or_else(|| panic!("{layout:?}: no glyph for {q:?}"))
            };
            let lit = tokens::color::STATE_ACCENT;
            let quiet = tokens::color::TEXT_DIM;
            for (q, colour) in [
                (Quick::Rate(Rating::VeryNegative), quiet),
                (Quick::Rate(Rating::Negative), quiet),
                (Quick::Rate(Rating::Positive), lit),
                (Quick::Rate(Rating::VeryPositive), quiet),
                (Quick::SetAside, lit),
                (Quick::Hide, quiet),
                (Quick::Delete, quiet),
                (Quick::Forever, lit),
            ] {
                let (_, _, drawn, about) = glyph(q);
                assert_eq!(drawn, colour, "{layout:?}: {q:?}");
                assert_eq!(about, Some(q.command()), "{layout:?}: {q:?}");
            }
            // AND A CLICK ON EACH ACTS AT ONCE — a mark told written, the
            // next composing from it.
            let mut click = |q: Quick| {
                let (n, from, _, _) = glyph(q);
                assert!(
                    v.clicked_at(n, from, &rows),
                    "{layout:?}: {q:?} did not act"
                );
                let e = v.key(Press::Enter, Some(&s), 100, &t);
                if matches!(e, Errand::Ranking(Act::Mark { .. })) {
                    written(&mut v, e.clone());
                }
                e
            };
            let mark = |e: Errand| match e {
                Errand::Ranking(Act::Mark { mark, .. }) => mark,
                other => panic!("{other:?}"),
            };
            assert_eq!(
                mark(click(Quick::Rate(Rating::Positive))),
                Mark::Rating(id.clone(), None)
            );
            assert_eq!(
                mark(click(Quick::Rate(Rating::Negative))),
                Mark::Rating(id.clone(), Some(Rating::Negative))
            );
            assert_eq!(
                mark(click(Quick::SetAside)),
                Mark::SetAside(id.clone(), false)
            );
            assert_eq!(mark(click(Quick::Forever)), Mark::Keep(id.clone(), true));
            assert!(matches!(
                click(Quick::Hide),
                Errand::Ranking(Act::Hide { id: hidden, .. }) if hidden == id
            ));
            assert_eq!(
                click(Quick::Delete),
                Errand::Nothing,
                "deleted with no question"
            );
            let rows = v.rows(Some(&s), 100, &t);
            assert_eq!(rows.get(v.cursor()).map(|r| r.spot.as_str()), Some("no"));
        }
        // A HIDDEN VIDEO, SET ASIDE: its eye is open, lit, says so, and shows
        // it again.
        let mut hidden = video(2);
        hidden.hidden = true;
        hidden.set_aside_at_s = Some(1_700_000_000);
        let mut v = on_the_aside(vec![hidden.clone()]);
        let rows = v.rows(Some(&s), 100, &t);
        let target = Target::RankedQuick(hidden.id.clone(), Quick::Hide);
        let (n, c) = rows
            .iter()
            .enumerate()
            .find_map(|(n, r)| Some((n, r.cells.iter().find(|c| c.target == target)?.clone())))
            .expect("the eye");
        let drawn = rows[n]
            .fields
            .iter()
            .find(|f| f.at == c.from)
            .expect("its glyph");
        assert_eq!(drawn.text, tokens::icon::VIDEO_SHOW.to_string());
        assert_eq!(drawn.color, tokens::color::STATE_ACCENT);
        assert_eq!(c.about, Some(crate::player_seam::ranking::SHOW));
        assert!(crate::chrome::hint_sentence(crate::player_seam::ranking::SHOW, &t).is_some());
        assert!(v.clicked_at(n, c.from, &rows));
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 100, &t),
            Errand::Ranking(Act::Mark { mark: Mark::Hidden(shown, false), .. }) if shown == hidden.id
        ));
    }

    /// FAILS IF: a second press before the catalogue is read again composes
    /// from the row as it was read — the ring's next rating written twice,
    /// the bookmark setting aside again what the first click took out — or
    /// a mark does not show on the row at the click; or a second mark is sent
    /// while the first is still being written, to race it; or a mark refused
    /// stays drawn; or a mark read back stays on a row the catalogue changed
    /// since.
    #[test]
    fn a_second_mark_composes_from_the_first() {
        let t = texts("en-GB");
        let s = fixture::idle();
        let one = video(1);
        let id = one.id.clone();
        let mut v = View::blank(None).plugging(&PAGE);
        *v.ranking_mut() = Ranking::at(Some(PathBuf::from("/nowhere/catalog.sqlite")));
        // AS A LIST: the cursor follows a video's row.
        v.ranking_mut().layout = Layout::List;
        v.ranking_mut().given(vec![video(0), one.clone()], 2);
        v.push(PageId::Ranking, None);
        v.follow(&spot_of(&id), Some(&s), 100, &t);
        v.key(Press::Ranking(RankingKey::More), Some(&s), 100, &t);
        assert_eq!(v.page(), PageId::RankedVideo);
        v.follow(RankedAct::Rate.spot(), Some(&s), 100, &t);
        let said = |v: &View| said_on(&v.rows(Some(&s), 100, &t));
        // THE RING TURNED: shown at once, and the next turn waits for it.
        let e = v.key(Press::More, Some(&s), 100, &t);
        assert!(
            said(&v).contains(&t.t(crate::video_metrics_words::rating_key(Rating::VeryNegative))),
            "the rating sent is not on its row"
        );
        assert!(
            matches!(v.key(Press::More, Some(&s), 100, &t), Errand::Said(ref w) if w.contains(&one.title)),
            "a second mark sent while the first is being written"
        );
        assert_eq!(
            written(&mut v, e),
            Mark::Rating(id.clone(), Some(Rating::VeryNegative))
        );
        // WRITTEN, NOT READ BACK: the next turn is the next rating.
        let e = v.key(Press::More, Some(&s), 100, &t);
        assert_eq!(
            written(&mut v, e),
            Mark::Rating(id.clone(), Some(Rating::Negative)),
            "the same rating written twice"
        );
        // READ BACK, changed since by another door: the row is the
        // catalogue's, and the ring turns from it.
        let mut read = one;
        read.rating = Some(Rating::Positive);
        v.ranking_mut().given(vec![video(0), read], 2);
        let e = v.key(Press::More, Some(&s), 100, &t);
        assert_eq!(
            written(&mut v, e),
            Mark::Rating(id.clone(), Some(Rating::VeryPositive)),
            "a mark read back still drawn"
        );
        // THE BOOKMARK, twice: set aside, then taken out.
        let aside = |v: &mut View| {
            v.key(
                Press::Ranking(RankingKey::Quick(Quick::SetAside)),
                Some(&s),
                100,
                &t,
            )
        };
        let e = aside(&mut v);
        assert!(
            said(&v).contains(&t.t(RankedAct::SetAside.key_when(true))),
            "set aside, and its row does not say so"
        );
        assert_eq!(written(&mut v, e), Mark::SetAside(id.clone(), true));
        let e = aside(&mut v);
        assert_eq!(
            written(&mut v, e),
            Mark::SetAside(id.clone(), false),
            "set aside again"
        );
        // REFUSED: dropped, and the row is the catalogue's.
        let e = aside(&mut v);
        let Errand::Ranking(Act::Mark { mark, title, .. }) = e else {
            panic!("no mark: {e:?}");
        };
        assert_eq!(mark, Mark::SetAside(id, true));
        assert!(v.ranking_told(&Outcome::Marked(mark, title, Err(Why::Busy))));
        assert!(
            said(&v).contains(&t.t(RankedAct::SetAside.key())),
            "a mark refused still drawn"
        );
        assert!(matches!(
            aside(&mut v),
            Errand::Ranking(Act::Mark {
                mark: Mark::SetAside(_, true),
                ..
            })
        ));
    }

    /// FAILS IF: a list's lines say a video's score with a word after its
    /// number — "points" on every line, read once and then only in the way —
    /// in either language, either layout; or the number is not the one the
    /// language writes.
    #[test]
    fn the_score_has_no_word() {
        let s = fixture::idle();
        for (language, word) in [("en-GB", "point"), ("pt-BR", "ponto")] {
            let t = texts(language);
            for layout in [Layout::List, Layout::Grid] {
                let mut v = on_the_ranking(3);
                v.ranking_mut().layout = layout;
                let said = said_on(&v.rows(Some(&s), 100, &t));
                assert!(!said.contains(word), "{language} {layout:?}: {said}");
                let number = crate::video_metrics_words::score_number(video(1).score, &t);
                assert!(said.contains(&number), "{language} {layout:?}: {said}");
            }
        }
    }

    /// FAILS IF: a row of a video's own page — or the page, the layout or
    /// the Shorts' ring of a list — stops saying under the pointer what it
    /// does, in either language: Nyxt, the rating, setting aside and keeping
    /// for ever among them.
    #[test]
    fn every_row_of_a_video_s_page_explains_itself() {
        let s = fixture::idle();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let mut v = on_the_ranking(3);
            v.follow(&spot_of(&video(1).id), Some(&s), 100, &t);
            v.key(Press::Ranking(RankingKey::More), Some(&s), 100, &t);
            let rows = v.rows(Some(&s), 100, &t);
            for act in RankedAct::ALL {
                let row = &rows[row_at(&rows, act.spot())];
                let id = row
                    .target
                    .about()
                    .unwrap_or_else(|| panic!("{act:?}: no sentence"));
                assert!(
                    crate::chrome::hint_sentence(id, &t).is_some(),
                    "{language}: {act:?} says nothing"
                );
            }
            // THE THREE ROWS ABOVE A LIST'S VIDEOS, on both lists.
            for v in [on_the_ranking(3), on_the_aside(vec![video(1)])] {
                let rows = v.rows(Some(&s), 100, &t);
                for spot in ["ranking-page", "ranking-layout", "ranking-shorts"] {
                    assert!(
                        rows[row_at(&rows, spot)]
                            .target
                            .about()
                            .and_then(|id| crate::chrome::hint_sentence(id, &t))
                            .is_some(),
                        "{language}: {spot} says nothing"
                    );
                }
            }
            for q in Quick::ALL {
                assert!(
                    crate::chrome::hint_sentence(q.command(), &t).is_some(),
                    "{q:?}"
                );
            }
        }
    }
}
