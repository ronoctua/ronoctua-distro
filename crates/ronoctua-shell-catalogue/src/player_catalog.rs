//! CHANNELS BY THEME, on the local catalogue — as a door sees it: one source
//! at a time, its themes and categories with how many each holds, and the
//! LISTING a row of them opens (plan item 9).
//!
//! # The door reads, the daemon ingests
//!
//! The catalogue is two SQLite files the daemon writes
//! (`ronoctua_broadcast::catalog`); a door opens them READ-ONLY and asks them
//! itself — every filter an index, a few milliseconds — on a thread of its
//! own ([`Worker`]): the newest question wins, and the page stays on screen
//! while the next is read. A door with no loop (a photograph, a test) reads
//! in place. What moved is told by the state's summary
//! ([`ronoctua_player::wire::CatalogSummary::generation`]): the door asks
//! again when it moves, and opens a reader again when the file was made
//! anew (`Reader::replaced`).
//!
//! # A source shown stale is asked for, once
//!
//! Nothing ingests on a clock. A source this door SHOWS — on the ring, on
//! the sources' page — whose last read is older than its freshness
//! (`assets/channels.nix`), or that was never read, is asked for
//! (`Ask::Ingest`), once for that reading of it: a source whose last ingest
//! FAILED is not asked again on its own (the page says why, and a row reads
//! it again), and one whose hosts the person did not let waits for the row
//! that lets them — a press on a row that names the hosts is the consent.
//! Channels by theme reads its ring's source's row; the sources' page reads
//! every source's ([`Want::Sources`]), so each says when it was read and
//! each stale one is asked for.
//!
//! # The letters that are there
//!
//! The letter row lists the letters the listing's titles are filed under —
//! `#` first, then as they fold: `Ç` after `C`, `Ñ` after `N`, Cyrillic and
//! CJK as their own — each a cell a click takes where the row has room, and
//! a ring the arrows turn where it has not; `C-l` types one.
//!
//! # The adult catalogue is behind the person's password
//!
//! Its rows are in a file of their own (`catalog-adult.sqlite`), which this
//! door reads only after the person typed their system password on a masked
//! line and PAM took it (`ronoctua_pam`, on a thread). The door stays open
//! until it closes or for [`UNLOCKED_FOR`] with no key and no click — a
//! thread wakes it then, and only while it is open. The page says what this
//! is: a lock on the doors, not encryption; and a system with no lock to ask
//! (the `.#dev` loop on a host without the `ronoctua-unlock` service) says
//! so on the page, where the row that would ask stands inert.
//!
//! # The whole screen is a window's
//!
//! `F` puts the listing alone on the whole screen: in the window, that window
//! (`Stage::Listing`); from the footer and the column, a window opened that
//! way, handed the listing in a runtime file ([`hand_over`]). An ADULT
//! listing is handed over too — what it lists, never a row of it — and the
//! window it opens is shut: it asks the password first, masked, and opens
//! the listing once the password is taken ([`View::lock_owed`]).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::key;
use ronoctua_broadcast::catalog::{
    self, CategoryId, CategoryRow, ItemId, ItemKind, ListPage, ListQuery, Reader,
    Row as CatalogRow, Scope, Sort, SourceRow, Sources, Store, Trouble as CatalogTrouble,
};
use ronoctua_broadcast::sources::Kind as SourceKind;
use ronoctua_broadcast::table::{self, DefaultSource, Theme};
use ronoctua_i18n::Arg;
use ronoctua_player::wire::{Ask, SourceId, SourceStatus, State, Thing};
use ronoctua_tokens as tokens;
use serde::{Deserialize, Serialize};

use crate::player::{Errand, PageId, Press, Target, Typing, View};
use crate::player_question::Question;
use crate::player_rows::Builder;

// THE WORDS THE PLAYER'S VIEW NAMES these pages by — their keys and their
// troubles' sentences — live under it (`crate::player_seam`): a line here
// recompiles nothing of the player's.
pub use crate::player_seam::{ListingKey, catalog_trouble_key, unlock_refused_key};

/// The source a temporary search of the archive is written as — the
/// daemon's (`engine/catalog.rs`): never on the ring.
pub const SEARCHED: &str = "archive-search";

/// The prefix of an ingest's category that searches the archive.
pub const SEARCH_PREFIX: &str = "search:";

/// How many rows a page of a listing holds — the ring's values.
pub const PER_PAGE: [u32; 4] = [25, 50, 100, 200];

/// The heights the quality ring asks for at least, in lines.
pub const QUALITIES: [u16; 3] = [480, 720, 1080];

/// How long the adult catalogue stays open with no key and no click.
pub const UNLOCKED_FOR: Duration = Duration::from_secs(15 * 60);

/// A letter typed (`C-l`): the question's id.
pub const ASK_LETTER: &str = "player-catalog-letter";
/// A page typed.
pub const ASK_PAGE: &str = "player-catalog-page";
/// The words a listing is searched by.
pub const ASK_SEARCH: &str = "player-catalog-search";
/// A link pasted to add a source (`crate::player_sources`).
pub const ASK_SOURCE: &str = "player-source-url";
/// A link pasted to add a source straight to the adult catalogue.
pub const ASK_ADULT_SOURCE: &str = "player-adult-source-url";
/// The person's system password, masked.
pub const ASK_PASSWORD: &str = "player-adult-password";

/// Whether question `id` is the catalogue's to answer.
#[must_use]
pub fn asks(id: &str) -> bool {
    matches!(
        id,
        ASK_LETTER | ASK_PAGE | ASK_SEARCH | ASK_SOURCE | ASK_ADULT_SOURCE | ASK_PASSWORD
    )
}

/// How a listing is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    /// A row an item: its title, and what it is beside it.
    #[default]
    List,
    /// Its title, and under it what it is.
    Lines,
    /// Titles only, in columns.
    Columns,
    /// A grid of posters, each with its title under it.
    Posters,
    /// The same, larger.
    Big,
}

impl Layout {
    /// Every layout, in the ring's order.
    pub const ALL: [Self; 5] = [
        Self::List,
        Self::Lines,
        Self::Columns,
        Self::Posters,
        Self::Big,
    ];

    /// The next one of the ring.
    #[must_use]
    pub fn next(self) -> Self {
        let at = Self::ALL.iter().position(|l| *l == self).unwrap_or(0);
        Self::ALL[(at + 1) % Self::ALL.len()]
    }

    /// The one before.
    #[must_use]
    pub fn previous(self) -> Self {
        let at = Self::ALL.iter().position(|l| *l == self).unwrap_or(0);
        Self::ALL[(at + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    /// Its name's key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::List => key::PLAYER_CATALOG_LAYOUT_LIST,
            Self::Lines => key::PLAYER_CATALOG_LAYOUT_LINES,
            Self::Columns => key::PLAYER_CATALOG_LAYOUT_COLUMNS,
            Self::Posters => key::PLAYER_CATALOG_LAYOUT_POSTERS,
            Self::Big => key::PLAYER_CATALOG_LAYOUT_BIG,
        }
    }

    /// Its glyph — the title's control says which layout it is now.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::List => tokens::icon::CATALOG_LAYOUT_LIST,
            Self::Lines => tokens::icon::CATALOG_LAYOUT_LINES,
            Self::Columns => tokens::icon::CATALOG_LAYOUT_COLUMNS,
            Self::Posters => tokens::icon::CATALOG_LAYOUT_POSTERS,
            Self::Big => tokens::icon::CATALOG_LAYOUT_BIG,
        }
    }

    /// Whether it draws posters: its page never holds more than a screen of
    /// them (`space.catalog.postersMost`).
    #[must_use]
    pub const fn posters(self) -> bool {
        matches!(self, Self::Posters | Self::Big)
    }
}

/// What a listing lists, and how — every choice its rows turn. Kept whole
/// by the view, and handed to the window a stage opens for it
/// ([`hand_over`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing {
    /// Which of the two files.
    pub store: Store,
    /// Its source.
    pub source: String,
    /// What it takes of it: a category, a theme, everything.
    pub scope: Scope,
    /// Every source's, and not its source's alone.
    pub every_source: bool,
    /// The words searched every category's, and not the scope's alone.
    pub every_category: bool,
    /// Only titles filed under this letter.
    pub letter: Option<char>,
    /// Only what these words find.
    pub words: Option<String>,
    /// Only this country.
    pub country: Option<String>,
    /// Only this language.
    pub language: Option<String>,
    /// Only pictures at least this tall.
    pub min_height: Option<u16>,
    /// The order.
    pub sort: Sort,
    /// Which page, from 1.
    pub page: u32,
    /// How many rows a page holds.
    pub per_page: u32,
    /// How it is drawn.
    pub layout: Layout,
}

impl Listing {
    /// A listing of `scope` of `source`, from its first page, as the person
    /// last drew listings (`layout`, `per_page`, `sort`).
    #[must_use]
    pub fn of(store: Store, source: &str, scope: Scope, like: Option<&Self>) -> Self {
        Self {
            store,
            source: source.to_owned(),
            scope,
            every_source: false,
            every_category: false,
            letter: None,
            words: None,
            country: None,
            language: None,
            min_height: None,
            sort: like.map_or(Sort::Title, |l| l.sort),
            page: 1,
            per_page: like.map_or(PER_PAGE[1], |l| l.per_page),
            layout: like.map_or(Layout::List, |l| l.layout),
        }
    }

    /// How many rows its page holds, as asked: a poster layout's never more
    /// than a screen of posters.
    #[must_use]
    pub fn rows_asked(&self) -> u32 {
        if self.layout.posters() {
            self.per_page
                .min(u32::from(tokens::space::CATALOG_POSTERSMOST))
        } else {
            self.per_page
        }
    }

    /// The question the catalogue is asked.
    #[must_use]
    pub fn query(&self) -> ListQuery {
        let words = self.words.clone().filter(|w| !w.trim().is_empty());
        ListQuery {
            scope: if words.is_some() && self.every_category {
                Scope::AllCategories
            } else {
                self.scope.clone()
            },
            sources: if self.every_source {
                Sources::All
            } else {
                Sources::One(self.source.clone())
            },
            letter: self.letter,
            words,
            country: self.country.clone(),
            language: self.language.clone(),
            min_height: self.min_height,
            sort: self.sort,
            page: self.page.max(1),
            per_page: self.rows_asked(),
        }
    }
}

/// What the view asks the catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Want {
    /// A source's themes and categories — and, open, the adult file's.
    Home {
        /// The source.
        source: String,
        /// The adult file is open to this door.
        adult: bool,
    },
    /// A listing's page, its letters and its facets.
    List(Box<Listing>),
    /// Every source's row — the sources' page: when each was read, and why
    /// it failed — and, open, the adult file's.
    Sources {
        /// The adult file is open to this door.
        adult: bool,
    },
}

/// What the catalogue answered a source's home.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Home {
    /// Every source of the main file.
    pub sources: Vec<SourceRow>,
    /// The source's themes, and how many items each holds.
    pub themes: Vec<(String, u64)>,
    /// Its categories.
    pub categories: Vec<CategoryRow>,
    /// How many of its items the adult file holds — told while it is shut
    /// too: a row says it is there, and nothing of it.
    pub adult_items: u64,
    /// The adult file's rows of its sources, while it is open — an adult
    /// source's row is there alone.
    pub adult_sources: Vec<SourceRow>,
    /// The adult file's themes of it, while it is open.
    pub adult_themes: Vec<(String, u64)>,
    /// Its categories there.
    pub adult_categories: Vec<CategoryRow>,
}

/// What the catalogue answered a listing.
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    /// The page.
    pub page: ListPage,
    /// The letters its titles are filed under, the letter aside.
    pub letters: Vec<char>,
    /// Its countries, the most items first, the country aside.
    pub countries: Vec<(String, u64)>,
    /// Its languages.
    pub languages: Vec<(String, u64)>,
    /// The category it is of, when it is one's — its title.
    pub category: Option<CategoryRow>,
}

/// What came back.
#[derive(Debug, Clone, PartialEq)]
pub enum Got {
    /// A home.
    Home(Box<Home>),
    /// A listing.
    List(Box<Listed>),
    /// Every source's row.
    Sources(Vec<SourceRow>),
}

/// An answer, and what it was to.
#[derive(Debug, Clone, PartialEq)]
struct Answer {
    want: Want,
    /// The state's generation when it was asked.
    generation: u64,
    got: Result<Got, CatalogTrouble>,
}

/// The two files, open — each opened again when it was made anew.
#[derive(Debug, Default)]
struct Readers {
    main: Option<Reader>,
    adult: Option<Reader>,
}

impl Readers {
    /// The reader of `store`, opened when it is not, or was replaced.
    fn get(&mut self, dir: &Path, store: Store) -> Result<&Reader, CatalogTrouble> {
        let slot = match store {
            Store::Main => &mut self.main,
            Store::Adult => &mut self.adult,
        };
        if slot.as_ref().is_none_or(Reader::replaced) {
            *slot = None;
            *slot = Some(Reader::open(dir, store)?);
        }
        slot.as_ref().ok_or(CatalogTrouble::Unreadable)
    }

    /// Forgotten after a read that said the file is not this build's: the
    /// writer makes it again, and the next read opens that one.
    fn forget(&mut self, store: Store) {
        match store {
            Store::Main => self.main = None,
            Store::Adult => self.adult = None,
        }
    }
}

/// `want`, asked of the files in `dir`. Blocks for as long as the reads take
/// — milliseconds: every one is on an index.
fn answer(readers: &mut Readers, dir: &Path, want: &Want) -> Result<Got, CatalogTrouble> {
    let got = match want {
        Want::Home { source, adult } => home_of(readers, dir, source, *adult),
        Want::List(listing) => list_of(readers, dir, listing),
        Want::Sources { adult } => sources_of(readers, dir, *adult),
    };
    if got == Err(CatalogTrouble::OtherSchema) {
        readers.forget(Store::Main);
        readers.forget(Store::Adult);
    }
    got
}

fn home_of(
    readers: &mut Readers,
    dir: &Path,
    source: &str,
    adult: bool,
) -> Result<Got, CatalogTrouble> {
    let one = Sources::One(source.to_owned());
    let main = readers.get(dir, Store::Main)?;
    let mut home = Home {
        sources: main.sources()?,
        themes: main.themes(&one)?,
        categories: main.categories(&one)?,
        ..Home::default()
    };
    // THE ADULT FILE, ASKED HOW MANY and nothing more while it is shut; a file
    // that is not there has nothing.
    match readers.get(dir, Store::Adult) {
        Ok(r) => {
            let rows = r.sources()?;
            home.adult_items = rows.iter().find(|s| s.id == source).map_or(0, |s| s.items);
            if adult {
                home.adult_sources = rows;
                home.adult_themes = r.themes(&one)?;
                home.adult_categories = r.categories(&one)?;
            }
        }
        Err(CatalogTrouble::Absent) => {}
        Err(t) => return Err(t),
    }
    Ok(Got::Home(Box::new(home)))
}

/// Every source's row of the main file — and of the adult file while it is
/// `adult`, open: shut, nothing of it is read.
fn sources_of(readers: &mut Readers, dir: &Path, adult: bool) -> Result<Got, CatalogTrouble> {
    let mut rows = readers.get(dir, Store::Main)?.sources()?;
    if adult {
        match readers.get(dir, Store::Adult) {
            Ok(r) => rows.extend(r.sources()?),
            Err(CatalogTrouble::Absent) => {}
            Err(t) => return Err(t),
        }
    }
    Ok(Got::Sources(rows))
}

fn list_of(readers: &mut Readers, dir: &Path, listing: &Listing) -> Result<Got, CatalogTrouble> {
    let q = listing.query();
    let r = readers.get(dir, listing.store)?;
    let page = r.list(&q)?;
    let letters = r.letters(&q)?;
    let countries = r.countries(&q)?;
    let languages = r.languages(&q)?;
    let category = match &listing.scope {
        Scope::Category(id) => r
            .categories(&Sources::One(listing.source.clone()))?
            .into_iter()
            .find(|c| c.id == *id),
        Scope::Theme(_) | Scope::AllCategories => None,
    };
    Ok(Got::List(Box::new(Listed {
        page,
        letters,
        countries,
        languages,
        category,
    })))
}

/// What the worker is asked, and what it answered.
#[derive(Debug, Default)]
struct Shared {
    /// The newest question, waiting.
    asked: Option<(Want, u64)>,
    /// The answer, waiting for the door.
    answered: Option<Answer>,
    /// The view is gone: the thread ends.
    ended: bool,
}

/// The thread that reads the catalogue for this door: one question at a
/// time, the newest waiting one next — a question overtaken before it was
/// read is never read.
#[derive(Debug)]
struct Worker {
    shared: Arc<(Mutex<Shared>, Condvar)>,
}

impl Worker {
    fn start(dir: PathBuf, wake: crate::wake::Wake) -> Option<Self> {
        let shared: Arc<(Mutex<Shared>, Condvar)> = Arc::default();
        let theirs = shared.clone();
        let born = std::thread::Builder::new()
            .name("player-catalog".to_owned())
            .spawn(move || {
                let mut readers = Readers::default();
                let (lock, bell) = &*theirs;
                loop {
                    let (want, generation) = {
                        let mut s = lock.lock().unwrap_or_else(PoisonError::into_inner);
                        loop {
                            if s.ended {
                                return;
                            }
                            if let Some(asked) = s.asked.take() {
                                break asked;
                            }
                            s = bell.wait(s).unwrap_or_else(PoisonError::into_inner);
                        }
                    };
                    let got = answer(&mut readers, &dir, &want);
                    lock.lock().unwrap_or_else(PoisonError::into_inner).answered = Some(Answer {
                        want,
                        generation,
                        got,
                    });
                    wake.call();
                }
            });
        match born {
            Ok(_) => Some(Self { shared }),
            Err(e) => {
                tracing::warn!(error = %e, "could not start the thread that reads the catalogue");
                None
            }
        }
    }

    fn ask(&self, want: Want, generation: u64) {
        let (lock, bell) = &*self.shared;
        lock.lock().unwrap_or_else(PoisonError::into_inner).asked = Some((want, generation));
        bell.notify_all();
    }

    fn take(&self) -> Option<Answer> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .answered
            .take()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let (lock, bell) = &*self.shared;
        lock.lock().unwrap_or_else(PoisonError::into_inner).ended = true;
        bell.notify_all();
    }
}

/// The adult file's lock, for this door process.
#[derive(Debug, Default)]
pub struct Lock {
    /// Open: when the last key or click was.
    touched: Option<Instant>,
    /// A password is being checked.
    pub checking: bool,
    /// Why the last check could not be made — no lock service on this system,
    /// PAM not answering — said on the page for as long as this door is open
    /// (a wrong password is not kept: it is the person's to type again).
    pub refused: Option<ronoctua_pam::Refusal>,
    /// The thread that wakes the door when the time is up, while it is open.
    watch: Option<Arc<Mutex<Option<Instant>>>>,
}

impl Lock {
    /// Whether the adult file is open to this door now.
    #[must_use]
    pub fn open(&self) -> bool {
        self.touched.is_some_and(|at| at.elapsed() < UNLOCKED_FOR)
    }

    /// Opened — the person's password taken — and a thread armed to wake
    /// the door when it shuts again, for as long as it is open.
    pub fn opened(&mut self, wake: Option<&crate::wake::Wake>) {
        self.checking = false;
        self.refused = None;
        let now = Instant::now();
        self.touched = Some(now);
        let Some(wake) = wake else {
            return;
        };
        let last: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(Some(now)));
        self.watch = Some(last.clone());
        let wake = wake.clone();
        let born = std::thread::Builder::new()
            .name("player-adult-lock".to_owned())
            .spawn(move || {
                loop {
                    let Some(at) = *last.lock().unwrap_or_else(PoisonError::into_inner) else {
                        return;
                    };
                    let left = UNLOCKED_FOR.saturating_sub(at.elapsed());
                    if left.is_zero() {
                        wake.call();
                        return;
                    }
                    std::thread::sleep(left);
                }
            });
        if let Err(e) = born {
            tracing::warn!(error = %e, "could not start the adult lock's watch");
        }
    }

    /// A key or a click: the fifteen minutes start again.
    pub fn touched(&mut self) {
        if self.open() {
            let now = Instant::now();
            self.touched = Some(now);
            if let Some(w) = &self.watch {
                *w.lock().unwrap_or_else(PoisonError::into_inner) = Some(now);
            }
        }
    }

    /// Shut, by a press or by the time.
    pub fn shut(&mut self) {
        self.touched = None;
        self.checking = false;
        if let Some(w) = self.watch.take() {
            *w.lock().unwrap_or_else(PoisonError::into_inner) = None;
        }
    }
}

/// The catalogue as this view holds it.
#[derive(Debug, Default)]
pub struct Catalog {
    /// The catalogue's folder: this machine's, or a photograph's.
    dir: Option<PathBuf>,
    /// The source on the ring.
    pub source: Option<String>,
    /// The listing on its page.
    pub listing: Option<Listing>,
    /// The last answer to each kind of question.
    home: Option<Answer>,
    list: Option<Answer>,
    sources: Option<Answer>,
    /// The reading thread, once a door with a loop asked.
    worker: Option<Worker>,
    /// The adult file's lock.
    pub lock: Lock,
    /// The ingests this door asked: a source and the reading of it that was
    /// stale — asked once.
    asked: BTreeSet<(String, Option<i64>)>,
    /// Which default sources' hosts the person let, as last read.
    pub let_hosts: BTreeSet<String>,
    /// The consent file is to be read again.
    consent_stirred: bool,
    /// The person's consent file is this machine's to read — never a
    /// photograph's or a test's catalogue, which lets nothing: the same
    /// picture on every machine.
    reads_consent: bool,
    /// The posters this door holds (`crate::player_posters`).
    pub posters: crate::player_posters::Posters,
    /// What a door with no loop was owed — a photograph's, a test's: the
    /// asks a stale source shown there would have sent.
    pub owed: Vec<Ask>,
    /// The machine's zone, a source's reading is dated in — `None` for a
    /// photograph's and a test's, which say every date in UTC, and say so.
    zone: Option<ronoctua_files::when::Zone>,
    /// `F` writes the listing where the window opened for its stage reads it
    /// ([`hand_over`]) — the footer's and the column's: a window takes the
    /// stage itself, and a photograph writes nothing of the machine's.
    pub hands_over: bool,
    /// An ADULT listing handed over to this window, waiting for the person's
    /// password: opened once it is taken ([`View::unlocked`]).
    pub waiting: Option<Listing>,
}

impl Catalog {
    /// This machine's catalogue, and the person's consent read for it.
    #[must_use]
    pub fn of_this_machine() -> Self {
        Self {
            consent_stirred: true,
            reads_consent: true,
            zone: Some(ronoctua_files::when::Zone::read_from(Path::new(
                "/etc/localtime",
            ))),
            hands_over: true,
            ..Self::at(catalog::folder())
        }
    }

    /// The catalogue in `dir` — a photograph's, a test's: no host is let,
    /// and no consent file of the machine's is read.
    #[must_use]
    pub fn at(dir: Option<PathBuf>) -> Self {
        Self {
            posters: crate::player_posters::Posters::at(dir.as_ref()),
            dir,
            ..Self::default()
        }
    }

    /// Which hosts are let — read again at the next look.
    pub fn stir_the_consent(&mut self) {
        self.consent_stirred = true;
    }

    /// The last home, when it is the source asked.
    fn home_of(&self, source: &str) -> Option<&Result<Got, CatalogTrouble>> {
        self.home
            .as_ref()
            .filter(|a| matches!(&a.want, Want::Home { source: s, .. } if s == source))
            .map(|a| &a.got)
    }

    /// The last listing answered, whatever it was asked with: a page stays
    /// on screen while the next is read.
    fn listed(&self) -> Option<&Result<Got, CatalogTrouble>> {
        self.list.as_ref().map(|a| &a.got)
    }

    /// Whether the answer held for `want` is the one to show at
    /// `generation`.
    fn holds(held: Option<&Answer>, want: &Want, generation: u64) -> bool {
        held.is_some_and(|a| a.want == *want && a.generation == generation)
    }

    /// `want` asked — of the worker, or in place with no loop.
    fn ask(
        &mut self,
        want: Want,
        generation: u64,
        wake: Option<&crate::wake::Wake>,
        readers: &mut Readers,
    ) {
        let Some(dir) = self.dir.clone() else {
            return;
        };
        match wake {
            Some(wake) => {
                if self.worker.is_none() {
                    self.worker = Worker::start(dir, wake.clone());
                }
                if let Some(w) = &self.worker {
                    w.ask(want, generation);
                }
            }
            None => {
                let got = answer(readers, &dir, &want);
                self.keep(Answer {
                    want,
                    generation,
                    got,
                });
            }
        }
    }

    fn keep(&mut self, a: Answer) {
        match &a.want {
            Want::Home { .. } => self.home = Some(a),
            Want::List(_) => self.list = Some(a),
            Want::Sources { .. } => self.sources = Some(a),
        }
    }

    /// What the catalogue last said of source `id`'s row, in the answer the
    /// page on screen reads — every source's on the sources' page, the
    /// ring's own elsewhere: `None` while no answer could say (no evidence
    /// either way), `Some(None)` when one says it holds no row of it — it
    /// was never read, or there is no catalogue yet.
    fn row_of(&self, id: &str, page: PageId) -> Option<Option<SourceRow>> {
        let got = if page == PageId::Sources {
            self.sources.as_ref().map(|a| &a.got)
        } else {
            self.home_of(id)
        }?;
        Some(match got {
            Ok(Got::Home(h)) => h
                .sources
                .iter()
                .chain(&h.adult_sources)
                .find(|r| r.id == id)
                .cloned(),
            Ok(Got::Sources(rows)) => rows.iter().find(|r| r.id == id).cloned(),
            Ok(Got::List(_)) | Err(_) => None,
        })
    }
}

/// The name a source is shown by: a default one's key, a person's own name
/// or its address, masked.
#[must_use]
pub fn source_name(id: &str, state: Option<&State>, texts: &ronoctua_i18n::Texts) -> String {
    if let Some(d) = DefaultSource::of(id) {
        return texts.t(d.key);
    }
    let status = state.and_then(|s| s.catalog.sources.iter().find(|x| x.id.0 == id));
    status
        .and_then(|s| s.title.clone())
        .or_else(|| status.map(|s| s.shown.clone()))
        .unwrap_or_else(|| id.to_owned())
}

/// Every source on the ring, in order: the defaults, then the person's —
/// never the temporary search, and a source kept in the adult catalogue only
/// while the adult catalogue is `open` to this door: shut, its name is not
/// on screen either.
#[must_use]
pub fn ring(state: Option<&State>, open: bool) -> Vec<String> {
    let mut out: Vec<String> = table::DEFAULT_SOURCES
        .iter()
        .map(|d| d.id.to_owned())
        .collect();
    if let Some(s) = state {
        out.extend(
            s.catalog
                .sources
                .iter()
                .filter(|x| DefaultSource::of(&x.id.0).is_none() && x.id.0 != SEARCHED)
                .filter(|x| open || !x.adult)
                .map(|x| x.id.0.clone()),
        );
    }
    out
}

/// How long a source stays fresh, in minutes.
#[must_use]
pub fn fresh_minutes(id: &str, kind: SourceKind) -> u64 {
    if let Some(d) = DefaultSource::of(id) {
        return d.fresh_minutes;
    }
    match kind {
        SourceKind::Feed => table::FRESH_FEED_MINUTES,
        SourceKind::YouTube => table::FRESH_YOUTUBE_MINUTES,
        SourceKind::Collection => table::FRESH_COLLECTION_MINUTES,
        _ => table::FRESH_PLAYLIST_MINUTES,
    }
}

/// Whether a source read at `last_s` is stale at `now_s`: never read, or
/// read longer ago than its freshness. Pure.
#[must_use]
pub fn stale(last_s: Option<i64>, fresh_minutes: u64, now_s: i64) -> bool {
    last_s.is_none_or(|at| {
        now_s.saturating_sub(at)
            > i64::try_from(fresh_minutes.saturating_mul(60)).unwrap_or(i64::MAX)
    })
}

/// Seconds since the epoch.
fn now_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// A count, as a row's value.
fn counted(n: u64, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_args(
        key::PLAYER_META_COUNT,
        &[("count", Arg::Number(i64::try_from(n).unwrap_or(i64::MAX)))],
    )
}

/// A theme's name.
fn theme_name(id: &str, texts: &ronoctua_i18n::Texts) -> String {
    Theme::of(id).map_or_else(|| id.to_owned(), |t| texts.t(t.key))
}

/// A category's name: what its source calls it, or "no category".
fn category_name(c: &CategoryRow, texts: &ronoctua_i18n::Texts) -> String {
    if c.title.trim().is_empty() {
        texts.t(key::PLAYER_CATALOG_UNGROUPED)
    } else {
        c.title.clone()
    }
}

/// A country's name in the person's language, by its code.
fn country_named(code: &str, texts: &ronoctua_i18n::Texts) -> String {
    ronoctua_broadcast::country::normal(code).map_or_else(
        |_| code.to_owned(),
        |c| crate::player_globe::country_name(c, texts),
    )
}

/// A language's name, by what the source wrote: the table's name for a code
/// it knows, the word itself otherwise.
fn language_named(word: &str, texts: &ronoctua_i18n::Texts) -> String {
    let lower = word.trim().to_ascii_lowercase();
    ronoctua_player::tongues::of(&lower).map_or_else(
        || word.to_owned(),
        |t| crate::player_find::tongue_name(t.code, texts),
    )
}

/// What a row of a listing says of its item: its kind when it is no film,
/// its source (every source's listing), its country, its height, how long it
/// runs, its year.
fn said_of(
    row: &CatalogRow,
    every_source: bool,
    state: Option<&State>,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let mut out: Vec<String> = Vec::new();
    match row.kind {
        ItemKind::Live => out.push(texts.t(key::PLAYER_CATALOG_KIND_LIVE)),
        ItemKind::Audio => out.push(texts.t(key::PLAYER_CATALOG_KIND_AUDIO)),
        ItemKind::Video => {}
    }
    if every_source {
        out.push(source_name(&row.source, state, texts));
    }
    if let Some(c) = &row.country {
        out.push(country_named(c, texts));
    }
    if let Some(lines) = row.height {
        out.push(texts.t_args(
            key::PLAYER_TV_LINES,
            &[("lines", Arg::Number(i64::from(lines)))],
        ));
    }
    if let Some(d) = row.duration_s {
        out.push(crate::player::clock(f64::from(d)));
    }
    if let Some(y) = row.year {
        out.push(y.to_string());
    }
    out.join(&texts.t(key::PLAYER_META_JOINER))
}

/// The glyph a kind of item is drawn with in the compact columns.
const fn kind_glyph(kind: ItemKind) -> char {
    match kind {
        ItemKind::Live => tokens::icon::CATALOG_LIVE,
        ItemKind::Video => tokens::icon::PLAYER_LANE_PICTURE,
        ItemKind::Audio => tokens::icon::PLAYER_LANE_SOUND,
    }
}

// ---------------------------------------------------------------------------
// THE VIEW'S SIDE
// ---------------------------------------------------------------------------

/// CHANNELS BY THEME, ITS LISTING AND ITS SOURCES as the player's view holds
/// them — this crate's half of the view, over the view's own (the seam:
/// `crate::player_seam`): the catalogue's state is the view's plugged
/// [`Catalog`], and what is drawn, read and turned on its pages is here.
pub trait CatalogueView {
    /// The catalogue as this view holds it — the view's plugged [`Catalog`].
    fn catalog(&self) -> &Catalog;

    /// The same, to change.
    fn catalog_mut(&mut self) -> &mut Catalog;

    /// The source on the ring now.
    fn catalog_source(&self, state: Option<&State>) -> String;

    /// The source's status in the state.
    fn status_of<'s>(state: Option<&'s State>, id: &str) -> Option<&'s SourceStatus>;

    /// Whether the person let every host a default source asks; a person's
    /// source is its own consent.
    fn consented(&self, id: &str) -> bool;

    /// WHAT THE CATALOGUE'S PAGES READ, on each settle: the consent file when
    /// it was stirred, the answer that came back, and the question for what
    /// is on screen asked again when it is not the one held — or the
    /// daemon's generation moved; and the ingests a stale source shown here
    /// owes the player.
    fn catalog_look(&mut self, state: Option<&State>) -> Vec<Ask>;

    /// The ingest a stale source shown here owes the player — once for that
    /// reading of it; never for one whose last ingest failed, is under way,
    /// or whose hosts are not let.
    fn ingest_owed(&mut self, id: &str, s: &State, page: PageId) -> Option<Ask>;

    /// The title of the listing's page: what it lists — a theme, a category,
    /// a search's words.
    #[must_use]
    fn catalog_title(&self, texts: &ronoctua_i18n::Texts) -> Option<String>;

    /// The rows of the catalogue's two pages, `page` on top.
    fn catalog_rows(&self, page: PageId, state: Option<&State>, b: &mut Builder<'_>);

    /// A source's standing, under its row: when it was read, that it is
    /// being read, why it gave nothing and a row that reads it again — or,
    /// for a default source not let, the hosts it asks and the row that lets
    /// them and reads.
    fn source_standing(&self, id: &str, state: Option<&State>, b: &mut Builder<'_>);

    /// CHANNELS BY THEME: the source on the ring, its standing, the row to
    /// the sources and the search; its themes and its categories, each with
    /// how many it holds — and the adult catalogue, shut or open.
    fn catalog_home(&self, state: Option<&State>, b: &mut Builder<'_>);

    /// THE ADULT CATALOGUE SHUT: why it cannot open, when the system said so
    /// — no lock service, PAM not answering — and the row that asks the
    /// password, `count` beside it; inert, and saying only that it is locked,
    /// where there is no lock to ask.
    fn lock_rows(&self, count: u64, b: &mut Builder<'_>);

    /// One file's themes and categories of a source.
    fn store_rows(
        store: Store,
        items: u64,
        themes: &[(String, u64)],
        categories: &[CategoryRow],
        b: &mut Builder<'_>,
    );

    /// A LISTING: the letters, the page, the search and its scopes, the
    /// order, the rows a page holds, the country, the language, the quality
    /// and the layout — then how many, and its items, as the layout draws
    /// them.
    fn catalog_listing(&self, state: Option<&State>, b: &mut Builder<'_>);

    /// THE LETTER ROW: every letter the listing's titles are filed under,
    /// each a cell a click takes — where the row has room for them all; a
    /// ring the arrows turn where it has not.
    fn letter_row(&self, l: &Listing, letters: &[char], b: &mut Builder<'_>);

    /// A LISTING OPENED on `scope` of the source on the ring, over the page
    /// on screen — as the last listing was drawn.
    fn open_listing_of(
        &mut self,
        store: Store,
        scope: Scope,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    );

    /// How many pages the listing on screen has, as its last answer said —
    /// `None` until one came for it (whichever page it asked).
    fn pages_known(&self) -> Option<u32>;

    /// The listing changed: from its first page — the rows it holds are
    /// others — and read again.
    fn listing_changed(&mut self, first_page: bool);

    /// What a press does on a row of the catalogue's pages — `None` for a
    /// target that is none of theirs. What it changed is read again at once:
    /// nothing else asks before the next key.
    fn catalog_turned(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand>;

    /// [`View::catalog_turned`]'s answer, before the settle.
    fn catalog_turn(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand>;

    /// A ring of the listing turned — the arrows, and `RET` on the ones a
    /// press asks typed.
    fn listing_turned(
        &mut self,
        target: &Target,
        back: bool,
        press: Press,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand>;

    /// The next source of the ring — or the one before.
    fn next_source(&mut self, state: Option<&State>, back: bool);

    /// `/` on the catalogue: the words, typed.
    fn search_the_catalogue(&mut self) -> Errand;

    /// `F`, and the title's glyph: the listing alone on the whole screen —
    /// the window's stage; a door without one (the footer, the column) hands
    /// the listing over to the window it opens, with the source the ring is
    /// on — as it is, a person's source too.
    fn listing_full(&mut self) -> Errand;

    /// What `F` hands over, in a door that hands it over: the listing, and
    /// the source the ring is on as it is — never one worked out again
    /// without the state, which knows only the default sources.
    fn handed(&self) -> Option<Handed>;

    /// A key of the listing's (`Press::Listing`).
    fn listing_pressed(
        &mut self,
        key: ListingKey,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand;

    /// What was typed on one of the catalogue's questions — and the listing
    /// it changed read again at once: nothing else asks before the next key.
    fn catalog_answered(
        &mut self,
        id: &str,
        said: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand;

    /// [`View::catalog_answered`]'s answer, before the settle.
    fn catalog_answer(
        &mut self,
        id: &str,
        said: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand;

    /// An item of the catalogue, now (after what plays, the queue kept) or at
    /// the queue's end — read from the file by its id: its stream is never in
    /// a page, and never crosses a wire before this.
    fn item_put(&self, store: Store, id: i64, play: bool, texts: &ronoctua_i18n::Texts) -> Errand;

    /// A window opened for the listing's stage opens on the listing handed
    /// over — channels by theme under it, where `Esc` goes back to. An adult
    /// one waits, shut, for the person's password: the window asks it
    /// ([`View::lock_owed`]) and opens the listing once it is taken.
    fn open_the_handed_listing(&mut self, handed: Handed);

    /// `listing` on its page, over channels by theme.
    fn open_handed(&mut self, listing: Listing);

    /// An adult listing handed over waits for the person's password.
    #[must_use]
    fn catalog_waits(&self) -> bool;

    /// The masked question the waiting adult listing owes the person — when
    /// one waits, nothing is being checked or typed, and the system has a
    /// lock to ask.
    fn catalog_lock_owed(&mut self) -> Option<Errand>;

    /// This door takes the listing's stage itself — the window: `F` hands
    /// nothing over.
    fn catalog_takes_its_own_stage(&mut self);

    /// The catalogue of `dir` — a photograph's (`--catalog`).
    fn catalogue_at(&mut self, dir: PathBuf);

    /// How a password check ended (`Told::Unlock`): open — and an adult
    /// listing handed over opened — or shut, and why kept on the page when
    /// the system could not check it at all.
    fn unlocked(&mut self, answer: Result<(), ronoctua_pam::Refusal>);

    /// The screen's scale the door draws at — a poster is decoded to its
    /// pixels. `true` when it changed.
    fn catalog_scale_is(&mut self, scale: u32) -> bool;

    /// A key or a click on the view: the adult lock's time starts again.
    fn catalog_touched(&mut self);
}

impl CatalogueView for View {
    fn catalog(&self) -> &Catalog {
        self.plugged::<Catalog>().expect(PLUGGED)
    }

    fn catalog_mut(&mut self) -> &mut Catalog {
        self.plugged_mut::<Catalog>().expect(PLUGGED)
    }

    fn catalog_source(&self, state: Option<&State>) -> String {
        let ring = ring(state, self.catalog().lock.open());
        self.catalog()
            .source
            .clone()
            .filter(|s| ring.contains(s))
            .or_else(|| ring.first().cloned())
            .unwrap_or_default()
    }

    fn status_of<'s>(state: Option<&'s State>, id: &str) -> Option<&'s SourceStatus> {
        state.and_then(|s| s.catalog.sources.iter().find(|x| x.id.0 == id))
    }

    fn consented(&self, id: &str) -> bool {
        DefaultSource::of(id).is_none_or(|d| {
            d.hosts
                .iter()
                .all(|h| self.catalog().let_hosts.contains(*h))
        })
    }

    fn catalog_look(&mut self, state: Option<&State>) -> Vec<Ask> {
        let page = self.page();
        if !matches!(page, PageId::Channels | PageId::Catalog | PageId::Sources) {
            return Vec::new();
        }
        if self.catalog().consent_stirred && self.catalog().reads_consent {
            self.catalog_mut().consent_stirred = false;
            self.catalog_mut().let_hosts = table::DEFAULT_SOURCES
                .iter()
                .flat_map(|d| d.hosts.iter())
                .filter(|h| ronoctua_consent::allows(h))
                .map(|h| (*h).to_owned())
                .collect();
        }
        // THE LOCK SHUTS BY ITSELF after its time, and an adult listing on
        // screen goes with it.
        if self.catalog().lock.touched.is_some() && !self.catalog().lock.open() {
            self.catalog_mut().lock.shut();
        }
        if !self.catalog().lock.open()
            && self
                .catalog()
                .listing
                .as_ref()
                .is_some_and(|l| l.store == Store::Adult)
        {
            self.catalog_mut().listing = None;
            self.catalog_mut().list = None;
            if self.page() == PageId::Catalog {
                self.levels.pop();
            }
        }
        let mut came = Vec::new();
        if let Some(w) = &self.catalog().worker {
            while let Some(a) = w.take() {
                came.push(a);
            }
        }
        for a in came {
            self.catalog_mut().keep(a);
        }
        let generation = state.map_or(0, |s| s.catalog.generation);
        let wake = self.wake.clone();
        let mut readers = Readers::default();
        let source = self.catalog_source(state);
        let adult = self.catalog().lock.open();
        let home = Want::Home {
            source: source.clone(),
            adult,
        };
        if matches!(self.page(), PageId::Channels)
            && !Catalog::holds(self.catalog().home.as_ref(), &home, generation)
        {
            self.catalog_mut()
                .ask(home, generation, wake.as_ref(), &mut readers);
        }
        if self.page() == PageId::Catalog
            && let Some(listing) = self.catalog().listing.clone()
        {
            let want = Want::List(Box::new(listing));
            if !Catalog::holds(self.catalog().list.as_ref(), &want, generation) {
                self.catalog_mut()
                    .ask(want, generation, wake.as_ref(), &mut readers);
            }
        }
        // THE SOURCES' PAGE reads every source's row: each says when it was
        // read, and each stale one is asked for.
        let every = Want::Sources { adult };
        if page == PageId::Sources
            && !Catalog::holds(self.catalog().sources.as_ref(), &every, generation)
        {
            self.catalog_mut()
                .ask(every, generation, wake.as_ref(), &mut readers);
        }
        // THE POSTERS OF WHAT IS ON SCREEN: decoded here, fetched by the
        // daemon when a source's switch is on and one is not here yet.
        let mut asks = Vec::new();
        if let Some(Ok(Got::List(listed))) = self.catalog().listed()
            && self
                .catalog()
                .listing
                .as_ref()
                .is_some_and(|l| l.layout.posters())
        {
            let rows = listed.page.rows.clone();
            let posters_generation = state.map_or(0, |s| s.catalog.posters_generation);
            // THE TEMPORARY SEARCH'S posters are the archive's, by the
            // archive's switch — the daemon's rule for them too.
            let on = |source: &str| {
                let whose = if source == SEARCHED {
                    "archive"
                } else {
                    source
                };
                Self::status_of(state, whose).is_some_and(|s| s.posters)
            };
            let fetch: Vec<ronoctua_player::wire::ItemId> = self
                .catalog_mut()
                .posters
                .look(&rows, posters_generation, wake.as_ref(), &on)
                .into_iter()
                .map(|id| ronoctua_player::wire::ItemId(id.0))
                .collect();
            if !fetch.is_empty() && state.is_some() {
                asks.push(Ask::Posters { items: fetch });
            }
        }
        // A STALE SOURCE SHOWN — the ring's, and every one on the sources'
        // page — is asked for once, while the player runs.
        if let Some(s) = state {
            let shown: Vec<String> = if page == PageId::Sources {
                ring(state, self.catalog().lock.open())
            } else {
                vec![source]
            };
            for id in shown {
                if let Some(ask) = self.ingest_owed(&id, s, page) {
                    asks.push(ask);
                }
            }
        }
        asks
    }

    fn ingest_owed(&mut self, id: &str, s: &State, page: PageId) -> Option<Ask> {
        let status = Self::status_of(Some(s), id)?;
        if status.busy || status.trouble.is_some() || !self.consented(id) {
            return None;
        }
        // AN ANSWER NOT READ YET is no evidence the source was never read.
        let row = self.catalog().row_of(id, page)?;
        if row.as_ref().is_some_and(|r| r.last_error.is_some()) {
            return None;
        }
        let last = row.as_ref().and_then(|r| r.last_ingest_s);
        if !stale(last, fresh_minutes(id, status.kind), now_s()) {
            return None;
        }
        if !self.catalog_mut().asked.insert((id.to_owned(), last)) {
            return None;
        }
        Some(Ask::Ingest {
            source: SourceId(id.to_owned()),
            category: None,
        })
    }

    fn catalog_title(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        if self.page() != PageId::Catalog {
            return None;
        }
        let l = self.catalog().listing.as_ref()?;
        let named = match &l.scope {
            Scope::Theme(t) => theme_name(t, texts),
            Scope::AllCategories => texts.t(key::PLAYER_CATALOG_EVERYTHING),
            Scope::Category(id) => match self.catalog().listed() {
                Some(Ok(Got::List(listed))) => listed
                    .category
                    .as_ref()
                    .filter(|c| c.id == *id)
                    .map_or_else(
                        || texts.t(key::PLAYER_PAGE_CATALOG),
                        |c| category_name(c, texts),
                    ),
                _ => texts.t(key::PLAYER_PAGE_CATALOG),
            },
        };
        Some(named)
    }

    fn catalog_rows(&self, page: PageId, state: Option<&State>, b: &mut Builder<'_>) {
        match page {
            PageId::Channels => self.catalog_home(state, b),
            PageId::Catalog => self.catalog_listing(state, b),
            PageId::Sources => crate::player_sources::page(self, state, b),
            _ => {}
        }
    }

    fn source_standing(&self, id: &str, state: Option<&State>, b: &mut Builder<'_>) {
        let status = Self::status_of(state, id);
        // THE ROW THE PAGE ON SCREEN READ: every source's on the sources'
        // page, the ring's own on channels by theme. Not answered yet, it
        // says nothing — a source read long ago is no "not read yet".
        let answered = self.catalog().row_of(id, self.page());
        let row = answered.clone().flatten();
        if !self.consented(id)
            && let Some(d) = DefaultSource::of(id)
        {
            let hosts = d.hosts.join(&b.t(key::PLAYER_LIST_JOINER));
            b.pair_under(
                b.texts
                    .t_with(key::PLAYER_CONSENT_ASK, &[("hosts", &hosts)]),
                String::new(),
                &b.t(key::PLAYER_CONSENT_FIRST),
                Target::CatalogLet(id.to_owned()),
                &format!("let-{id}"),
            );
            return;
        }
        if status.is_some_and(|s| s.busy) {
            b.note(key::PLAYER_CATALOG_INGESTING);
        } else if let Some(t) = status.and_then(|s| s.trouble.as_ref()) {
            b.warning(&crate::player::trouble_sentence(t, b.texts));
        }
        match row.as_ref().and_then(|r| r.last_ingest_s) {
            Some(at) => {
                // IN THE PERSON'S ZONE — and where it could not be read (a
                // photograph's), in UTC, which the date says.
                let zone = self
                    .catalog()
                    .zone
                    .clone()
                    .unwrap_or_else(ronoctua_files::when::Zone::utc);
                let date = crate::chrome::when_text(at, &zone, true, b.texts);
                let fresh = status.map_or(SourceKind::Playlist, |s| s.kind);
                let said = if stale(Some(at), fresh_minutes(id, fresh), now_s()) {
                    b.texts
                        .t_with(key::PLAYER_CATALOG_STALE, &[("date", &date)])
                } else {
                    b.texts.t_with(key::PLAYER_LISTED_AT, &[("date", &date)])
                };
                crate::player_find::note_said(&said, b);
            }
            None if answered.is_some() && status.is_none_or(|s| !s.busy) => {
                b.note(key::PLAYER_CATALOG_NEVER);
            }
            None => {}
        }
        if state.is_some() && status.is_some_and(|s| !s.busy) {
            b.pair(
                b.t(key::PLAYER_CATALOG_READ_AGAIN),
                String::new(),
                Target::CatalogRead(id.to_owned()),
                &format!("read-{id}"),
            );
        }
    }

    fn catalog_home(&self, state: Option<&State>, b: &mut Builder<'_>) {
        b.note(key::PLAYER_ABOUT_CATALOG);
        let source = self.catalog_source(state);
        b.pair(
            b.t(key::PLAYER_CATALOG_SOURCE),
            source_name(&source, state, b.texts),
            Target::CatalogSource,
            "catalog-source",
        );
        self.source_standing(&source, state, b);
        let person = state.map_or(0, |s| {
            s.catalog
                .sources
                .iter()
                .filter(|x| DefaultSource::of(&x.id.0).is_none() && x.id.0 != SEARCHED)
                .count()
        });
        b.pair(
            b.t(PageId::Sources.key()),
            counted(u64::try_from(person).unwrap_or(u64::MAX), b.texts),
            Target::Go(PageId::Sources),
            PageId::Sources.spot(),
        );
        b.pair(
            b.t(key::PLAYER_CATALOG_SEARCH),
            String::new(),
            Target::CatalogSearch,
            "catalog-search",
        );
        let home = match self.catalog().home_of(&source) {
            None => return,
            Some(Err(CatalogTrouble::Absent)) => {
                b.note(key::PLAYER_CATALOG_EMPTY);
                return;
            }
            Some(Err(t)) => {
                b.warning(&b.t(catalog_trouble_key(*t)));
                return;
            }
            Some(Ok(Got::Home(h))) => h,
            Some(Ok(Got::List(_) | Got::Sources(_))) => return,
        };
        let items: u64 = home
            .sources
            .iter()
            .find(|r| r.id == source)
            .map_or(0, |r| r.items);
        if items == 0 && home.categories.is_empty() {
            b.note(key::PLAYER_CATALOG_EMPTY);
        } else {
            Self::store_rows(Store::Main, items, &home.themes, &home.categories, b);
        }
        if home.adult_items > 0 {
            b.heading(key::PLAYER_CATALOG_ADULT);
            b.note(key::PLAYER_ADULT_HONEST);
            if self.catalog().lock.open() {
                b.pair(
                    b.t(key::PLAYER_ADULT_LOCK),
                    String::new(),
                    Target::CatalogLock,
                    "adult-lock",
                );
                Self::store_rows(
                    Store::Adult,
                    home.adult_items,
                    &home.adult_themes,
                    &home.adult_categories,
                    b,
                );
            } else {
                self.lock_rows(home.adult_items, b);
            }
        }
    }

    fn lock_rows(&self, count: u64, b: &mut Builder<'_>) {
        let refused = self.catalog().lock.refused;
        if let Some(r) = refused {
            b.warning(&b.t(unlock_refused_key(r)));
        }
        let many = counted(count, b.texts);
        if refused == Some(ronoctua_pam::Refusal::NoService) {
            b.marked(tokens::icon::CATALOG_LOCKED, |b| {
                b.pair(
                    b.t(key::PLAYER_ADULT_SHUT),
                    many,
                    Target::Nothing,
                    "adult-unlock",
                );
            });
            return;
        }
        let value = if self.catalog().lock.checking {
            b.t(key::PLAYER_ADULT_CHECKING)
        } else {
            many
        };
        b.marked(tokens::icon::CATALOG_LOCKED, |b| {
            b.pair(
                b.t(key::PLAYER_ADULT_LOCKED),
                value,
                Target::CatalogUnlock,
                "adult-unlock",
            );
        });
    }

    fn store_rows(
        store: Store,
        items: u64,
        themes: &[(String, u64)],
        categories: &[CategoryRow],
        b: &mut Builder<'_>,
    ) {
        let adult = store == Store::Adult;
        if !adult {
            b.heading(key::PLAYER_CATALOG_THEMES);
        }
        b.pair(
            b.t(key::PLAYER_CATALOG_EVERYTHING),
            counted(items, b.texts),
            Target::CatalogScope(store, Scope::AllCategories),
            &format!("every-{}", store_word(store)),
        );
        for (theme, n) in themes {
            b.pair(
                theme_name(theme, b.texts),
                counted(*n, b.texts),
                Target::CatalogScope(store, Scope::Theme(theme.clone())),
                &format!("theme-{}-{theme}", store_word(store)),
            );
        }
        if categories.is_empty() {
            return;
        }
        if adult {
            b.heading_within(b.t(key::PLAYER_CATALOG_CATEGORIES));
        } else {
            b.heading(key::PLAYER_CATALOG_CATEGORIES);
        }
        for c in categories {
            b.pair(
                category_name(c, b.texts),
                counted(c.items, b.texts),
                Target::CatalogScope(store, Scope::Category(c.id)),
                &format!("category-{}-{}", store_word(store), c.id.0),
            );
        }
    }

    fn catalog_listing(&self, state: Option<&State>, b: &mut Builder<'_>) {
        let Some(l) = self.catalog().listing.as_ref() else {
            b.note(key::PLAYER_CATALOG_EMPTY);
            return;
        };
        let listed = match self.catalog().listed() {
            Some(Ok(Got::List(listed))) => Some(listed.as_ref()),
            Some(Err(CatalogTrouble::Absent)) => {
                b.note(key::PLAYER_CATALOG_EMPTY);
                None
            }
            Some(Err(t)) => {
                b.warning(&b.t(catalog_trouble_key(*t)));
                None
            }
            _ => None,
        };
        self.letter_row(l, listed.map_or(&[][..], |x| x.letters.as_slice()), b);
        let (page, pages) = listed.map_or((1, 1), |x| (x.page.page, x.page.pages));
        b.pair(
            b.t(key::PLAYER_CATALOG_PAGE),
            b.texts.t_args(
                key::PLAYER_CATALOG_PAGE_OF,
                &[
                    ("page", Arg::Number(i64::from(page))),
                    ("pages", Arg::Number(i64::from(pages))),
                ],
            ),
            Target::CatalogPage,
            "catalog-page",
        );
        let words = l.words.clone().filter(|w| !w.trim().is_empty());
        b.pair(
            b.t(key::PLAYER_CATALOG_SEARCH),
            words.as_ref().map_or_else(String::new, |w| {
                b.texts
                    .t_with(key::PLAYER_CATALOG_SEARCHED, &[("words", w)])
            }),
            Target::CatalogSearch,
            "catalog-search",
        );
        if words.is_some() {
            b.pair(
                b.t(key::PLAYER_CATALOG_SCOPE_IN),
                b.t(if l.every_category {
                    key::PLAYER_CATALOG_SCOPE_EVERY_CATEGORY
                } else {
                    key::PLAYER_CATALOG_SCOPE_CATEGORY
                }),
                Target::CatalogEveryCategory,
                "scope-category",
            );
            // THE WHOLE ARCHIVE, past what the catalogue keeps of it: a
            // temporary search the daemon reads.
            if l.source == "archive" && state.is_some() {
                let w = words.clone().unwrap_or_default();
                b.pair(
                    b.texts
                        .t_with(key::PLAYER_CATALOG_SEARCH_ARCHIVE, &[("words", &w)]),
                    String::new(),
                    Target::CatalogSearchArchive,
                    "search-archive",
                );
            }
        }
        // THE WHOLE ARCHIVE'S SEARCH, read by the daemon: under way, or why
        // it gave nothing and the row that asks again — never an empty page
        // that reads as "nothing matches".
        let searching = Self::status_of(state, SEARCHED).filter(|_| l.source == SEARCHED);
        if let Some(s) = searching {
            let asked = s.title.clone().unwrap_or_default();
            if s.busy {
                let said = b
                    .texts
                    .t_with(key::PLAYER_CATALOG_SEARCHING, &[("words", &asked)]);
                crate::player_find::note_said(&said, b);
            } else if let Some(t) = &s.trouble {
                b.warning(&crate::player::trouble_sentence(t, b.texts));
                b.pair(
                    b.texts
                        .t_with(key::PLAYER_CATALOG_SEARCH_ARCHIVE, &[("words", &asked)]),
                    String::new(),
                    Target::CatalogSearchArchive,
                    "search-archive",
                );
            }
        }
        let unanswered = searching.is_some_and(|s| s.busy || s.trouble.is_some());
        b.pair(
            b.t(key::PLAYER_CATALOG_SCOPE_FROM),
            b.t(if l.every_source {
                key::PLAYER_CATALOG_SCOPE_EVERY_SOURCE
            } else {
                key::PLAYER_CATALOG_SCOPE_SOURCE
            }),
            Target::CatalogEverySource,
            "scope-source",
        );
        b.pair(
            b.t(key::PLAYER_CATALOG_SORT),
            b.t(sort_key(l.sort)),
            Target::CatalogSort,
            "catalog-sort",
        );
        b.pair(
            b.t(key::PLAYER_CATALOG_PER_PAGE),
            l.rows_asked().to_string(),
            Target::CatalogPerPage,
            "catalog-per-page",
        );
        let country = l.country.as_ref().map_or_else(
            || b.t(key::PLAYER_CATALOG_COUNTRY_ALL),
            |c| country_named(c, b.texts),
        );
        b.pair(
            b.t(key::PLAYER_CATALOG_COUNTRY),
            country,
            Target::CatalogCountry,
            "catalog-country",
        );
        let language = l.language.as_ref().map_or_else(
            || b.t(key::PLAYER_CATALOG_LANGUAGE_ALL),
            |c| language_named(c, b.texts),
        );
        b.pair(
            b.t(key::PLAYER_CATALOG_LANGUAGE),
            language,
            Target::CatalogLanguage,
            "catalog-language",
        );
        let quality = l.min_height.map_or_else(
            || b.t(key::PLAYER_CATALOG_QUALITY_ANY),
            |lines| {
                b.texts.t_args(
                    key::PLAYER_CATALOG_QUALITY_LINES,
                    &[("lines", Arg::Number(i64::from(lines)))],
                )
            },
        );
        b.pair(
            b.t(key::PLAYER_CATALOG_QUALITY),
            quality,
            Target::CatalogQuality,
            "catalog-quality",
        );
        b.pair(
            b.t(key::PLAYER_CATALOG_LAYOUT),
            b.t(l.layout.key()),
            Target::CatalogLayout,
            "catalog-layout",
        );
        let Some(listed) = listed else {
            return;
        };
        let p = &listed.page;
        if p.total == 0 {
            // A SEARCH UNDER WAY OR REFUSED said what it is above.
            if !unanswered {
                b.heading(if l.letter.is_some() || words.is_some() {
                    key::PLAYER_CATALOG_NOTHING_FOUND
                } else {
                    key::PLAYER_CATALOG_EMPTY
                });
            }
            return;
        }
        let from = u64::from(p.page.saturating_sub(1)) * u64::from(l.rows_asked()) + 1;
        let to = from + u64::try_from(p.rows.len()).unwrap_or(0) - 1;
        let n = |x: u64| Arg::Number(i64::try_from(x).unwrap_or(i64::MAX));
        b.heading_said(b.texts.t_args(
            key::PLAYER_CATALOG_SHOWN,
            &[("from", n(from)), ("to", n(to)), ("total", n(p.total))],
        ));
        let item = |r: &CatalogRow| Target::CatalogItem(l.store, r.id.0);
        let spot = |r: &CatalogRow| format!("item-{}", r.id.0);
        match l.layout {
            Layout::List => {
                for r in &p.rows {
                    b.pair(
                        r.title.clone(),
                        said_of(r, l.every_source, state, b.texts),
                        item(r),
                        &spot(r),
                    );
                }
            }
            Layout::Lines => {
                for r in &p.rows {
                    b.pair_under(
                        r.title.clone(),
                        String::new(),
                        &said_of(r, l.every_source, state, b.texts),
                        item(r),
                        &spot(r),
                    );
                }
            }
            Layout::Columns => {
                let entries: Vec<(char, String, Target, String)> = p
                    .rows
                    .iter()
                    .map(|r| (kind_glyph(r.kind), r.title.clone(), item(r), spot(r)))
                    .collect();
                crate::player_posters::columns(&entries, b);
            }
            Layout::Posters | Layout::Big => {
                crate::player_posters::grid(
                    &self.catalog().posters,
                    &p.rows,
                    l.layout == Layout::Big,
                    &item,
                    b,
                );
            }
        }
    }

    fn letter_row(&self, l: &Listing, letters: &[char], b: &mut Builder<'_>) {
        let _ = self;
        let all = b.t(key::PLAYER_CATALOG_LETTER_ALL);
        let chosen = l.letter.map_or_else(|| all.clone(), |c| c.to_string());
        let first = b.len();
        b.pair(
            b.t(key::PLAYER_CATALOG_LETTER),
            chosen,
            Target::CatalogLetter(l.letter),
            "catalog-letter",
        );
        let end = b.left + b.room;
        let Some(row) = b.out.get_mut(first) else {
            return;
        };
        let Some(value_at) = row.fields.get(1).map(|f| f.at) else {
            return;
        };
        // EVERY LETTER, "all" first — its own cells, and a cell of air after
        // it. Between two letters a cell of air too where the row has room
        // for it; else the letters stand side by side, an index strip.
        let mut cells: Vec<(String, Option<char>)> = vec![(all, None)];
        cells.extend(letters.iter().map(|c| (c.to_string(), Some(*c))));
        let widths: Vec<u16> = cells
            .iter()
            .map(|(t, _)| ronoctua_protocol::cells(t))
            .collect();
        let Some(air) = letter_air(&widths, end.saturating_sub(value_at)) else {
            return;
        };
        let focused = row.focused;
        row.fields.truncate(1);
        let mut at = value_at;
        for (n, ((text, letter), width)) in cells.into_iter().zip(widths).enumerate() {
            // THE TONES ARE THE ROW'S KIND: the letter chosen is the row's
            // value — a setting's, under the cursor or not — and every other
            // one an item of the strip, a listed setting's.
            let colour = if letter == l.letter {
                crate::chrome::Kind::Setting.tones(focused).1
            } else {
                crate::chrome::Kind::ListedSetting.tones(false).1
            };
            // "ALL" IS A WORD, and always has its cell of air after it.
            let step = width + if n == 0 { 1 } else { air };
            row.fields.push(Builder::piece(at, text, colour));
            row.cells.push(crate::player_rows::Cell {
                from: at,
                width: step.max(width),
                target: Target::CatalogLetterCell(letter),
                spot: format!(
                    "letter-{}",
                    letter.map_or_else(|| "all".to_owned(), String::from)
                ),
                control: true,
                about: None,
            });
            at += step;
        }
    }

    fn open_listing_of(
        &mut self,
        store: Store,
        scope: Scope,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let source = self.catalog_source(state);
        let listing = Listing::of(store, &source, scope, self.catalog().listing.as_ref());
        self.catalog_mut().listing = Some(listing);
        self.catalog_mut().list = None;
        let from = self.spot_here(state, columns, texts);
        self.push(PageId::Catalog, from);
        self.settle_in(state, columns, texts);
    }

    fn pages_known(&self) -> Option<u32> {
        let l = self.catalog().listing.as_ref()?;
        let a = self.catalog().list.as_ref()?;
        let Want::List(asked) = &a.want else {
            return None;
        };
        let same = Listing {
            page: l.page,
            ..(**asked).clone()
        } == *l;
        match &a.got {
            Ok(Got::List(listed)) if same => Some(listed.page.pages.max(1)),
            _ => None,
        }
    }

    fn listing_changed(&mut self, first_page: bool) {
        if let Some(l) = self.catalog_mut().listing.as_mut()
            && first_page
        {
            l.page = 1;
        }
        let top = self.top_mut();
        top.cursor = 0;
        top.cell = 0;
        top.moved = false;
    }

    fn catalog_turned(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        let errand = self.catalog_turn(target, press, state, columns, texts)?;
        self.settle_in(state, columns, texts);
        Some(errand)
    }

    fn catalog_turn(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        self.catalog_mut().lock.touched();
        let back = matches!(press, Press::Less);
        let turn = matches!(press, Press::Less | Press::More | Press::Enter);
        match (target, press) {
            (Target::CatalogSource, Press::Less | Press::More | Press::Enter) => {
                self.next_source(state, back);
                Some(Errand::Nothing)
            }
            (Target::CatalogRead(id), Press::Enter) => Some(Errand::TellSaying(
                Ask::Ingest {
                    source: SourceId(id.clone()),
                    category: None,
                },
                texts.t_with(
                    key::PLAYER_CATALOG_READING_NOW,
                    &[("name", &source_name(id, state, texts))],
                ),
            )),
            (Target::CatalogLet(id), Press::Enter) => {
                let hosts: Vec<String> = DefaultSource::of(id)
                    .map(|d| d.hosts.iter().map(|h| (*h).to_owned()).collect())
                    .unwrap_or_default();
                self.catalog_mut().stir_the_consent();
                Some(Errand::LetHosts {
                    hosts,
                    on: true,
                    then: Box::new(Errand::Tell(Ask::Ingest {
                        source: SourceId(id.clone()),
                        category: None,
                    })),
                })
            }
            (Target::CatalogScope(store, scope), Press::Enter) => {
                self.open_listing_of(*store, scope.clone(), state, columns, texts);
                Some(Errand::Nothing)
            }
            (Target::CatalogSearch, Press::Enter) => Some(self.search_the_catalogue()),
            (Target::CatalogUnlock, Press::Enter) => {
                self.typing = Some(Typing::AdultPassword);
                Some(Errand::Ask(Question::plain(ASK_PASSWORD, true)))
            }
            (Target::CatalogLock, Press::Enter) => {
                self.catalog_mut().lock.shut();
                Some(Errand::Said(texts.t(key::PLAYER_ADULT_LOCKED_AGAIN)))
            }
            (Target::CatalogItem(store, id), Press::Enter | Press::PlayNow | Press::Add) => {
                Some(self.item_put(*store, *id, press != Press::Add, texts))
            }
            _ if turn => self.listing_turned(target, back, press, state, texts),
            _ => None,
        }
    }

    fn listing_turned(
        &mut self,
        target: &Target,
        back: bool,
        press: Press,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        let facets = match self.catalog().listed() {
            Some(Ok(Got::List(listed))) => Some((
                listed.letters.clone(),
                listed.countries.clone(),
                listed.languages.clone(),
                listed.page.pages,
            )),
            _ => None,
        };
        let (letters, countries, languages, pages) =
            facets.unwrap_or((Vec::new(), Vec::new(), Vec::new(), 1));
        let known = self.pages_known();
        let l = self.catalog_mut().listing.as_mut()?;
        let enter = press == Press::Enter;
        let first = match target {
            // A LETTER CELL, clicked: that letter. The row's own value turns
            // with the arrows, and `RET` asks one typed.
            Target::CatalogLetterCell(letter) if enter => {
                l.letter = *letter;
                true
            }
            Target::CatalogLetter(_) if enter => {
                self.typing = Some(Typing::CatalogLetter);
                return Some(Errand::Ask(Question::plain(ASK_LETTER, false)));
            }
            Target::CatalogLetter(_) => {
                l.letter = turned(&letters, l.letter.as_ref(), back).copied();
                true
            }
            Target::CatalogPage if enter => {
                self.typing = Some(Typing::CatalogPage);
                return Some(Errand::Ask(Question::plain(ASK_PAGE, false)));
            }
            Target::CatalogPage => {
                l.page = if back {
                    earlier(l.page, known)
                } else {
                    (l.page + 1).min(pages.max(1))
                };
                false
            }
            Target::CatalogEveryCategory => {
                l.every_category = !l.every_category;
                true
            }
            Target::CatalogEverySource => {
                l.every_source = !l.every_source;
                true
            }
            Target::CatalogSort => {
                l.sort = *turned(&Sort::ALL, Some(&l.sort), back).unwrap_or(&Sort::Title);
                true
            }
            Target::CatalogPerPage => {
                l.per_page = *turned(&PER_PAGE, Some(&l.per_page), back).unwrap_or(&PER_PAGE[1]);
                true
            }
            Target::CatalogCountry => {
                let codes: Vec<String> = countries.iter().map(|(c, _)| c.clone()).collect();
                l.country = turned(&codes, l.country.as_ref(), back).cloned();
                true
            }
            Target::CatalogLanguage => {
                let words: Vec<String> = languages.iter().map(|(c, _)| c.clone()).collect();
                l.language = turned(&words, l.language.as_ref(), back).cloned();
                true
            }
            Target::CatalogQuality => {
                l.min_height = turned(&QUALITIES, l.min_height.as_ref(), back).copied();
                true
            }
            Target::CatalogLayout => {
                l.layout = if back {
                    l.layout.previous()
                } else {
                    l.layout.next()
                };
                // A POSTER LAYOUT holds fewer rows: the page is asked again
                // from where the first row shown was.
                true
            }
            Target::CatalogFull if enter => return Some(self.listing_full()),
            Target::CatalogSearchArchive if enter => {
                // THE WORDS: the listing's — or, on the search's own listing
                // asked again after a refusal, the search's.
                let words = if l.source == SEARCHED {
                    Self::status_of(state, SEARCHED).and_then(|s| s.title.clone())
                } else {
                    l.words.clone()
                }
                .unwrap_or_default();
                if words.trim().is_empty() {
                    return Some(Errand::Nothing);
                }
                let key = format!("{SEARCH_PREFIX}{}", words.trim());
                // THE SEARCH'S OWN LISTING, once the daemon wrote it: the one
                // category of the temporary source — the archive's search
                // is what found it, so no filter of words narrows it again
                // (its items carry their titles only, and a title need not
                // hold the words the archive matched).
                l.source = SEARCHED.to_owned();
                l.scope = Scope::Category(CategoryId::of(SEARCHED, &key));
                l.words = None;
                l.every_category = false;
                l.every_source = false;
                l.page = 1;
                self.listing_changed(false);
                return Some(Errand::TellSaying(
                    Ask::Ingest {
                        source: SourceId("archive".to_owned()),
                        category: Some(key),
                    },
                    texts.t_with(key::PLAYER_CATALOG_SEARCHING, &[("words", words.trim())]),
                ));
            }
            _ => return None,
        };
        self.listing_changed(first);
        Some(Errand::Nothing)
    }

    fn next_source(&mut self, state: Option<&State>, back: bool) {
        let ring = ring(state, self.catalog().lock.open());
        let now = self.catalog_source(state);
        self.catalog_mut().source = turned(&ring, Some(&now), back)
            .cloned()
            .or_else(|| ring.first().cloned());
        self.catalog_mut().home = None;
        // A LISTING OF THE SOURCE LEFT lists the new one's — its first page.
        let source = self.catalog().source.clone();
        if let Some(l) = self.catalog_mut().listing.as_mut()
            && let Some(s) = source
        {
            l.source = s;
            if matches!(l.scope, Scope::Category(_)) {
                l.scope = Scope::AllCategories;
            }
            l.page = 1;
            l.letter = None;
        }
    }

    fn search_the_catalogue(&mut self) -> Errand {
        self.typing = Some(Typing::CatalogSearch);
        let held = self
            .catalog()
            .listing
            .as_ref()
            .and_then(|l| l.words.clone())
            .filter(|_| self.page() == PageId::Catalog);
        Errand::Ask(match held {
            Some(words) => Question::holding(ASK_SEARCH, words),
            None => Question::plain(ASK_SEARCH, false),
        })
    }

    fn listing_full(&mut self) -> Errand {
        if let Some(handed) = self.handed() {
            hand_over(&handed.listing, &handed.source);
        }
        Errand::Show(crate::player::Show::Listing)
    }

    fn handed(&self) -> Option<Handed> {
        let l = self
            .catalog()
            .listing
            .as_ref()
            .filter(|_| self.catalog().hands_over)?;
        let source = self
            .catalog()
            .source
            .clone()
            .filter(|s| s != SEARCHED)
            .unwrap_or_else(|| {
                // THE SEARCH'S listing is the archive's.
                if l.source == SEARCHED {
                    "archive".to_owned()
                } else {
                    l.source.clone()
                }
            });
        Some(Handed {
            listing: l.clone(),
            source,
        })
    }

    fn listing_pressed(
        &mut self,
        key: ListingKey,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        self.catalog_mut().lock.touched();
        if key == ListingKey::Source {
            self.next_source(state, false);
            if self.page() == PageId::Catalog {
                self.listing_changed(true);
            }
            self.settle_in(state, columns, texts);
            return Errand::Nothing;
        }
        if self.page() != PageId::Catalog {
            return Errand::Nothing;
        }
        let pages = match self.catalog().listed() {
            Some(Ok(Got::List(listed))) => listed.page.pages,
            _ => 1,
        };
        let known = self.pages_known();
        let Some(l) = self.catalog_mut().listing.as_mut() else {
            return Errand::Nothing;
        };
        let first_page = match key {
            ListingKey::Earlier => {
                l.page = earlier(l.page, known);
                false
            }
            ListingKey::Later => {
                l.page = (l.page + 1).min(pages.max(1));
                false
            }
            ListingKey::First => {
                l.page = 1;
                false
            }
            ListingKey::Last => {
                l.page = pages.max(1);
                false
            }
            ListingKey::Layout => {
                l.layout = l.layout.next();
                true
            }
            ListingKey::Order => {
                l.sort = *turned(&Sort::ALL, Some(&l.sort), false).unwrap_or(&Sort::Title);
                true
            }
            ListingKey::Letter => {
                self.typing = Some(Typing::CatalogLetter);
                return Errand::Ask(Question::plain(ASK_LETTER, false));
            }
            ListingKey::Full => return self.listing_full(),
            ListingKey::Source => false,
        };
        self.listing_changed(first_page);
        self.settle_in(state, columns, texts);
        Errand::Nothing
    }

    fn catalog_answered(
        &mut self,
        id: &str,
        said: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let errand = self.catalog_answer(id, said, state, columns, texts);
        self.settle_in(state, columns, texts);
        errand
    }

    fn catalog_answer(
        &mut self,
        id: &str,
        said: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let typing = self.typing.take();
        let said_trimmed = said.trim();
        match (id, typing) {
            (ASK_LETTER, Some(Typing::CatalogLetter)) => {
                let Some(l) = self.catalog_mut().listing.as_mut() else {
                    return Errand::Nothing;
                };
                l.letter = said_trimmed
                    .chars()
                    .next()
                    .map(|c| catalog::letter_of(&c.to_string()));
                self.listing_changed(true);
                Errand::Nothing
            }
            (ASK_PAGE, Some(Typing::CatalogPage)) => {
                if said_trimmed.is_empty() {
                    return Errand::Nothing;
                }
                let Ok(n) = said_trimmed.parse::<u32>() else {
                    return Errand::Said(texts.t_with(
                        key::PLAYER_CATALOG_PAGE_NOT_A_NUMBER,
                        &[("said", said_trimmed)],
                    ));
                };
                // PAST THE LAST, the last: the page kept is the one shown.
                let known = self.pages_known();
                if let Some(l) = self.catalog_mut().listing.as_mut() {
                    l.page = known.map_or(n, |pages| n.min(pages)).max(1);
                }
                self.listing_changed(false);
                Errand::Nothing
            }
            (ASK_SEARCH, Some(Typing::CatalogSearch)) => {
                let words = (!said_trimmed.is_empty()).then(|| said_trimmed.to_owned());
                if self.page() == PageId::Catalog {
                    if let Some(l) = self.catalog_mut().listing.as_mut() {
                        l.words = words;
                    }
                    self.listing_changed(true);
                } else if let Some(words) = words {
                    // FROM THE SOURCE'S PAGE, a listing of everything it
                    // holds, searched.
                    self.open_listing_of(Store::Main, Scope::AllCategories, state, columns, texts);
                    if let Some(l) = self.catalog_mut().listing.as_mut() {
                        l.words = Some(words);
                    }
                }
                Errand::Nothing
            }
            (ASK_SOURCE, Some(Typing::SourceUrl)) => {
                crate::player_sources::pasted(said, false, texts)
            }
            // THE ADULT CATALOGUE SHUT while the link was typed takes none.
            (ASK_ADULT_SOURCE, Some(Typing::AdultSourceUrl)) => {
                if self.catalog().lock.open() {
                    crate::player_sources::pasted(said, true, texts)
                } else {
                    Errand::Said(texts.t(key::PLAYER_ADULT_LOCKED_AGAIN))
                }
            }
            (ASK_PASSWORD, Some(Typing::AdultPassword)) => {
                if said.is_empty() {
                    return Errand::Nothing;
                }
                self.catalog_mut().lock.checking = true;
                Errand::Unlock(ronoctua_consent::keys::Secret::new(said))
            }
            (_, other) => {
                self.typing = other;
                Errand::Nothing
            }
        }
    }

    fn item_put(&self, store: Store, id: i64, play: bool, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(dir) = self.catalog().dir.as_ref() else {
            return Errand::Nothing;
        };
        let item = Reader::open(dir, store).and_then(|r| r.item(ItemId(id)));
        let Ok(Some(item)) = item else {
            return Errand::Said(texts.t(key::PLAYER_CATALOG_ITEM_GONE));
        };
        let name = item.row.title.clone();
        let mut stream = item.stream;
        if stream.title.is_none() {
            stream.title = Some(name.clone());
        }
        Errand::TellSaying(
            Ask::Put {
                what: vec![Thing::Stream { stream }],
                after: None,
                into: None,
                play,
                replace: false,
                once: false,
            },
            texts.t_with(
                if play {
                    key::PLAYER_PLAYING_NOW
                } else {
                    key::PLAYER_ADDED
                },
                &[("name", &name)],
            ),
        )
    }

    fn open_the_handed_listing(&mut self, handed: Handed) {
        self.catalog_mut().source = Some(handed.source);
        self.push(PageId::Channels, None);
        if handed.listing.store == Store::Adult && !self.catalog().lock.open() {
            self.catalog_mut().waiting = Some(handed.listing);
            return;
        }
        self.open_handed(handed.listing);
    }

    fn open_handed(&mut self, listing: Listing) {
        self.catalog_mut().listing = Some(listing);
        self.catalog_mut().list = None;
        self.push(PageId::Catalog, Some(PageId::Channels.spot().to_owned()));
    }

    fn catalog_waits(&self) -> bool {
        self.catalog().waiting.is_some()
    }

    fn catalog_lock_owed(&mut self) -> Option<Errand> {
        let lock = &self.catalog().lock;
        if self.catalog().waiting.is_none()
            || lock.open()
            || lock.checking
            || lock.refused == Some(ronoctua_pam::Refusal::NoService)
            || self.typing.is_some()
        {
            return None;
        }
        self.typing = Some(Typing::AdultPassword);
        Some(Errand::Ask(Question::plain(ASK_PASSWORD, true)))
    }

    fn catalog_takes_its_own_stage(&mut self) {
        self.catalog_mut().hands_over = false;
    }

    fn catalogue_at(&mut self, dir: PathBuf) {
        *self.catalog_mut() = Catalog::at(Some(dir));
    }

    fn unlocked(&mut self, answer: Result<(), ronoctua_pam::Refusal>) {
        match answer {
            Ok(()) => {
                let wake = self.wake.clone();
                self.catalog_mut().lock.opened(wake.as_ref());
                if let Some(l) = self.catalog_mut().waiting.take() {
                    self.open_handed(l);
                }
            }
            Err(why) => {
                self.catalog_mut().lock.shut();
                self.catalog_mut().lock.refused =
                    (why != ronoctua_pam::Refusal::Wrong).then_some(why);
            }
        }
        self.catalog_mut().home = None;
        self.catalog_mut().sources = None;
    }

    fn catalog_scale_is(&mut self, scale: u32) -> bool {
        let changed = self.catalog().posters.scale != scale;
        self.catalog_mut().posters.scale = scale;
        // THE RANKING'S THUMBNAILS are decoded to the same pixels by the
        // ranking's own page (`crate::player_seam`).
        changed
    }

    fn catalog_touched(&mut self) {
        self.catalog_mut().lock.touched();
    }
}

/// The page before `page` — of `pages` when they are known: a page kept past
/// the last (the listing shrank under it) goes back from the last one, the
/// one on screen, and not one by one from where it was. Pure.
#[must_use]
pub fn earlier(page: u32, pages: Option<u32>) -> u32 {
    pages
        .map_or(page, |last| page.min(last))
        .saturating_sub(1)
        .max(1)
}

/// The value after `now` in `values` — or before it — with "none" as a
/// value of its own at the ring's head: `None` when it lands there.
fn turned<'v, T: PartialEq>(values: &'v [T], now: Option<&T>, back: bool) -> Option<&'v T> {
    let at = now.and_then(|n| values.iter().position(|v| v == n));
    let len = values.len();
    if len == 0 {
        return None;
    }
    // THE RING: none, the first, …, the last, and none again.
    let slot = at.map_or(0, |i| i + 1);
    let next = if back {
        (slot + len) % (len + 1)
    } else {
        (slot + 1) % (len + 1)
    };
    next.checked_sub(1).and_then(|i| values.get(i))
}

/// The cells of air between two letters of the letter row whose cells are
/// `widths` ("all" first) in `room` cells: one where they fit so, none where
/// only side by side they do — `None` where not even then, and the row is a
/// ring the arrows turn. "All" keeps its cell of air after it either way.
/// Pure.
#[must_use]
pub fn letter_air(widths: &[u16], room: u16) -> Option<u16> {
    let (all, letters) = widths.split_first()?;
    let inked: u32 = letters.iter().map(|w| u32::from(*w)).sum();
    let gaps = u32::try_from(letters.len().saturating_sub(1)).unwrap_or(u32::MAX);
    let needed = |air: u32| u32::from(*all) + 1 + inked + air * gaps;
    [1, 0]
        .into_iter()
        .find(|air| needed(*air) <= u32::from(room))
        .and_then(|air| u16::try_from(air).ok())
}

/// A file's word in a row's spot.
const fn store_word(store: Store) -> &'static str {
    match store {
        Store::Main => "main",
        Store::Adult => "adult",
    }
}

/// An order's key.
const fn sort_key(sort: Sort) -> &'static str {
    match sort {
        Sort::Title => key::PLAYER_CATALOG_SORT_TITLE,
        Sort::Popular => key::PLAYER_CATALOG_SORT_POPULAR,
        Sort::Newest => key::PLAYER_CATALOG_SORT_NEWEST,
        Sort::Oldest => key::PLAYER_CATALOG_SORT_OLDEST,
    }
}

/// Whose system password opens the adult catalogue: this session's user.
#[must_use]
pub fn user() -> Option<String> {
    ["USER", "LOGNAME"]
        .iter()
        .find_map(|v| std::env::var(v).ok())
        .filter(|u| !u.is_empty())
}

/// `secret` checked as this session's user's system password — through the
/// door lock's PAM service. Blocks for as long as PAM does (a wrong answer
/// waits three seconds): call it on a thread.
///
/// # Errors
///
/// The PAM refusal; [`ronoctua_pam::Refusal::Unavailable`] when the session
/// names no user.
pub fn unlock(secret: &ronoctua_consent::keys::Secret) -> Result<(), ronoctua_pam::Refusal> {
    let user = user().ok_or(ronoctua_pam::Refusal::Unavailable)?;
    ronoctua_pam::verify(&user, secret)
}

/// The runtime file a door writes its listing to before the window opens for
/// its stage — what that window opens on.
pub const HANDED: &str = "player-listing.json";

/// What is handed over: the listing and the ring's source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handed {
    /// The listing.
    pub listing: Listing,
    /// The ring's source.
    pub source: String,
}

/// The listing written where the window opened for its stage reads it — the
/// footer's and the column's way of handing it over. An ADULT listing is
/// handed as what it lists, never a row of it, and the window it opens is
/// shut: it asks the password itself before it opens it
/// ([`View::open_the_handed_listing`]).
pub fn hand_over(listing: &Listing, source: &str) {
    let Some(path) = ronoctua_xdg::runtime_file(HANDED) else {
        return;
    };
    let handed = Handed {
        listing: listing.clone(),
        source: source.to_owned(),
    };
    match serde_json::to_vec(&handed) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(&path, bytes) {
                tracing::info!(error = %e, "the listing was not handed over");
            }
        }
        Err(e) => tracing::info!(error = %e, "the listing was not handed over"),
    }
}

/// The listing handed over, taken — the file removed.
#[must_use]
pub fn taken_over() -> Option<Handed> {
    let path = ronoctua_xdg::runtime_file(HANDED)?;
    let bytes = std::fs::read(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    serde_json::from_slice(&bytes).ok()
}

/// The person's source of the catalogue [`write_fixture`] writes — what a
/// photograph's `--handed` opens a listing of.
pub const FIXTURE_SOURCE: &str = "p1790000000";

/// A CATALOGUE FOR PHOTOGRAPHS AND TESTS, written into `dir`: the default
/// sources and one of a person's — an `http` IPTV list, its groups under their
/// themes, titles under every kind of letter (`#`, Latin with and without
/// accents, `Ç`, `Ñ`, Cyrillic, CJK), several countries, languages and
/// heights, enough items for pages — an adult group in the adult file, and a
/// poster drawn for every item that names one. Nothing of the network: the
/// rows are made here, and the posters are plain pictures.
///
/// # Errors
///
/// The catalogue's, or [`CatalogTrouble::Unwritable`] for a poster that
/// could not be written.
pub fn write_fixture(dir: &Path) -> Result<(), CatalogTrouble> {
    use ronoctua_broadcast::catalog::{Harvest, NewItem, NewSource, Writer};
    use ronoctua_player::own::Stream;
    let now = 1_790_000_000;
    let mut w = Writer::open(dir, 256 << 20, now)?;
    let item = |ext: &str, title: &str, kind: ItemKind, cats: &[&str], n: usize| {
        let countries = ["BR", "PT", "US", "JP", "RU", "ES"];
        let languages = ["por", "eng", "jpn", "rus", "spa"];
        let heights = [480_u16, 720, 1080];
        NewItem {
            ext_id: ext.to_owned(),
            kind,
            title: title.to_owned(),
            also: Vec::new(),
            description: None,
            country: Some(countries[n % countries.len()].to_owned()),
            language: Some(languages[n % languages.len()].to_owned()),
            height: (kind != ItemKind::Audio).then_some(heights[n % heights.len()]),
            duration_s: (kind != ItemKind::Live)
                .then_some(600 + u32::try_from(n).unwrap_or(0) * 97),
            year: (kind != ItemKind::Live).then_some(1920 + i32::try_from(n % 90).unwrap_or(0)),
            licence: None,
            popularity: u64::try_from(n * 37 % 1000).unwrap_or(0),
            published_s: None,
            poster_url: Some(format!("https://posters.example/{ext}.png")),
            stream: Stream::typed(format!("https://streams.example/{ext}")),
            categories: cats.iter().map(|c| (*c).to_owned()).collect(),
        }
    };
    // A PERSON'S LIST: its groups, and titles under every letter.
    let films = [
        "A Viagem",
        "Amanhecer",
        "Abismo",
        "Barravento",
        "Bossa Nova",
        "Casablanca",
        "Central do Brasil",
        "Çatalhöyük",
        "Cidade de Deus",
        "Dona Flor",
        "Édipo",
        "Eles Não Usam Black-Tie",
        "Faroeste",
        "Fitzcarraldo",
        "Garrincha",
        "Hotel",
        "Ilha das Flores",
        "Iracema",
        "Jeca Tatu",
        "Kiriku",
        "Limite",
        "Lavoura Arcaica",
        "Macunaíma",
        "Metrópolis",
        "Nosferatu",
        "Ñandutí",
        "O Bandido",
        "Orfeu Negro",
        "Pixote",
        "Pagador de Promessas",
        "Quilombo",
        "Rio 40 Graus",
        "São Paulo S/A",
        "Sertão",
        "Terra em Transe",
        "Tropa",
        "Ubirajara",
        "Vidas Secas",
        "Walachai",
        "Xica da Silva",
        "Yndio",
        "Zuzu Angel",
        "1917",
        "2001",
        "Андрей Рублёв",
        "Броненосец",
        "Солярис",
        "東京物語",
        "七人の侍",
        "羅生門",
    ];
    let mut h = Harvest::default();
    h.category("Filmes", "Filmes", Some("films"), false);
    h.category("Notícias", "Notícias", Some("news"), false);
    h.category("Esportes", "Esportes", Some("sports"), false);
    h.category("Adultos 18+", "Adultos 18+", None, true);
    for (n, title) in films.iter().enumerate() {
        h.items.push(item(
            &format!("film-{n}"),
            title,
            ItemKind::Video,
            &["Filmes"],
            n,
        ));
    }
    for n in 0..60 {
        h.items.push(item(
            &format!("more-{n}"),
            &format!("Filme {n:02}"),
            ItemKind::Video,
            &["Filmes"],
            n,
        ));
    }
    for (n, title) in [
        "Jornal Hoje",
        "BBC World",
        "Al Jazeera",
        "Deutsche Welle",
        "NHK World",
        "France 24",
    ]
    .iter()
    .enumerate()
    {
        h.items.push(item(
            &format!("news-{n}"),
            title,
            ItemKind::Live,
            &["Notícias"],
            n,
        ));
    }
    for (n, title) in ["Futebol ao vivo", "Olímpicos", "Tênis"].iter().enumerate() {
        h.items.push(item(
            &format!("sport-{n}"),
            title,
            ItemKind::Live,
            &["Esportes"],
            n,
        ));
    }
    for n in 0..5 {
        h.items.push(item(
            &format!("adult-{n}"),
            &format!("Noite {n}"),
            ItemKind::Video,
            &["Adultos 18+"],
            n,
        ));
    }
    let person = NewSource {
        id: FIXTURE_SOURCE.to_owned(),
        kind: SourceKind::Playlist,
        title: "Minha lista".to_owned(),
        shown: "iptv.example/…".to_owned(),
        adult: false,
        posters_on: true,
    };
    w.replace(&person, &h, now)?;
    // AND A DEFAULT SOURCE, under its themes.
    let mut archive = Harvest::default();
    archive.category("prelinger", "prelinger", Some("history"), false);
    archive.category("classic_tv", "classic_tv", Some("series"), false);
    for n in 0..24 {
        archive.items.push(item(
            &format!("archive-{n}"),
            &format!("Reel {n:02}"),
            ItemKind::Video,
            &[if n % 2 == 0 {
                "prelinger"
            } else {
                "classic_tv"
            }],
            n,
        ));
    }
    let default = NewSource {
        id: "archive".to_owned(),
        kind: SourceKind::Archive,
        title: String::new(),
        shown: "archive.org".to_owned(),
        adult: false,
        posters_on: true,
    };
    w.replace(&default, &archive, now)?;
    // THE POSTERS: a picture for every item, in a hue of its own, taller
    // than wide for a film and wide for a live.
    let posters = dir.join(catalog::POSTERS);
    std::fs::create_dir_all(&posters).map_err(|_| CatalogTrouble::Unwritable)?;
    for it in h.items.iter().chain(&archive.items) {
        let Some(url) = &it.poster_url else { continue };
        let (w_px, h_px) = if it.kind == ItemKind::Live {
            (160, 90)
        } else {
            (100, 150)
        };
        let seed = catalog::digest(url);
        let hue = u32::from_str_radix(&seed[..6], 16).unwrap_or(0);
        let picture = image::RgbaImage::from_fn(w_px, h_px, |x, y| {
            let r = u8::try_from((hue >> 16) & 0xff).unwrap_or(0);
            let g = u8::try_from((hue >> 8) & 0xff).unwrap_or(0);
            let b = u8::try_from(hue & 0xff).unwrap_or(0);
            let shade = u8::try_from((x + y) % 64).unwrap_or(0);
            image::Rgba([r.saturating_sub(shade), g, b.saturating_add(shade), 255])
        });
        picture
            .save_with_format(
                posters.join(catalog::poster_file(url)),
                image::ImageFormat::Png,
            )
            .map_err(|_| CatalogTrouble::Unwritable)?;
    }
    Ok(())
}

/// What the view's plugged catalogue is missing for: the catalogue's page
/// is installed in every view whose methods here are called (the seam:
/// `crate::player_seam`).
const PLUGGED: &str = "the view holds the catalogue's page";

/// CHANNELS BY THEME, ITS LISTING AND ITS SOURCES, as a page of the player
/// (`crate::player_seam::Page`): what the view asked of the catalogue at
/// each of its places, asked here.
pub struct Catalogue;

/// The catalogue's page, as the binary installs it.
pub static PAGE: Catalogue = Catalogue;

impl crate::player_seam::Page for Catalogue {
    fn name(&self) -> &'static str {
        "catalogue"
    }

    fn state(&self, this_machine: bool) -> Box<dyn std::any::Any> {
        Box::new(if this_machine {
            Catalog::of_this_machine()
        } else {
            Catalog::default()
        })
    }

    fn title(&self, view: &View, _: PageId, b: &mut Builder<'_>) -> bool {
        // A LISTING OF THE CATALOGUE carries its layout and its whole
        // screen beside its title.
        let Some(title) = view.catalog_title(b.texts) else {
            return false;
        };
        let layout = view
            .catalog()
            .listing
            .as_ref()
            .map_or(Layout::List, |l| l.layout);
        b.heading_controls_said(
            title,
            &[
                (
                    layout.glyph(),
                    Target::CatalogLayout,
                    "player-listing-layout",
                ),
                (
                    tokens::icon::CATALOG_FULLSCREEN,
                    Target::CatalogFull,
                    "player-listing-full",
                ),
            ],
        );
        true
    }

    fn rows(&self, view: &View, page: PageId, state: Option<&State>, b: &mut Builder<'_>) -> bool {
        // CHANNELS BY THEME, ITS LISTING AND ITS SOURCES: the local
        // catalogue's, read by this door itself.
        if !matches!(page, PageId::Channels | PageId::Catalog | PageId::Sources) {
            return false;
        }
        view.catalog_rows(page, state, b);
        true
    }

    fn look(&self, view: &mut View, state: Option<&State>, _: u16, _: &ronoctua_i18n::Texts) {
        // THE CATALOGUE'S PAGES read the catalogue — off the loop in a door
        // that has one — and what a stale source shown here owes the player
        // is told it, off the loop too; a door with no loop keeps it.
        let owed = view.catalog_look(state);
        if view.wake.is_some() {
            for ask in owed {
                crate::player::tell_then(ask, |answer| {
                    if answer != crate::player::Told::Taken {
                        tracing::debug!(?answer, "the catalogue's ask was not taken");
                    }
                });
            }
        } else {
            view.catalog_mut().owed.extend(owed);
        }
    }

    fn touched(&self, view: &mut View) {
        view.catalog_touched();
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
            // THE CATALOGUE'S PAGES search the catalogue.
            Press::Search if matches!(view.page(), PageId::Channels | PageId::Catalog) => {
                Some(view.search_the_catalogue())
            }
            Press::Listing(key) => Some(view.listing_pressed(key, state, columns, texts)),
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
        // ITS SOURCE, A LISTING'S RINGS, AN ITEM, THE ADULT LOCK — and its
        // sources' switches (`crate::player_sources`).
        view.catalog_turned(target, press, state, columns, texts)
            .or_else(|| {
                crate::player_sources::SourcesView::sources_turned(
                    view, target, press, state, texts,
                )
            })
    }

    fn asks(&self, id: &str) -> bool {
        asks(id)
    }

    fn answered(
        &self,
        view: &mut View,
        id: &str,
        said: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        // A LETTER, A PAGE, WORDS, A PASTED SOURCE, THE PASSWORD.
        view.catalog_answered(id, said, state, columns, texts)
    }

    fn let_go(&self, view: &mut View) {
        // AN ADULT LISTING HANDED OVER whose password was let go is let go
        // with it.
        if view.typing == Some(Typing::AdultPassword) {
            view.catalog_mut().waiting = None;
        }
    }

    fn picture<'v>(&self, view: &'v View, key: &str) -> Option<&'v crate::player_find::Picture> {
        // A POSTER OF THE LISTING (`crate::player_posters`): channels by
        // theme are there with no player running, and so are their posters.
        view.catalog().posters.picture(key)
    }

    fn told(&self, view: &mut View, answer: &crate::player::Told) -> Option<bool> {
        match answer {
            crate::player::Told::Unlock(answer) => {
                view.unlocked(*answer);
                Some(true)
            }
            _ => None,
        }
    }

    fn off_the_loop(&self, errand: Errand) -> Result<crate::player_seam::OffTheLoop, Errand> {
        // THE PERSON'S PASSWORD, checked OFF THE LOOP — PAM waits seconds on
        // a wrong one — and how it ended told back (`Told::Unlock`): the view
        // opens the adult catalogue, or says why not.
        match errand {
            Errand::Unlock(secret) => Ok(crate::player_seam::OffTheLoop {
                about: "the person's system password, checked",
                thread: "player-unlock",
                work: Box::new(move || {
                    let answer = unlock(&secret);
                    drop(secret);
                    crate::player::Told::Unlock(answer)
                }),
                unstarted: Some(key::PLAYER_ADULT_UNAVAILABLE),
            }),
            other => Err(other),
        }
    }

    fn owed(&self, view: &mut View) -> Option<Errand> {
        view.catalog_lock_owed()
    }

    fn scale_is(&self, view: &mut View, scale: u32) -> bool {
        view.catalog_scale_is(scale)
    }

    fn takes_its_own_stage(&self, view: &mut View) {
        view.catalog_takes_its_own_stage();
    }

    fn waits(&self, view: &View) -> bool {
        view.catalog_waits()
    }

    fn handed_over(&self, view: &mut View) -> bool {
        // WHAT A FOOTER'S OR A COLUMN'S `F` HANDED OVER (`hand_over`).
        let Some(handed) = taken_over() else {
            return false;
        };
        view.open_the_handed_listing(handed);
        true
    }

    fn hand_back(&self) {
        let _ = taken_over();
    }

    fn detached(&self, view: &mut View, outbox: &crate::player_window::Outbox) {
        // THE CATALOGUE IT WAS HANDED — a photograph's `--catalog`.
        if let Some(dir) = outbox.catalog.clone() {
            view.catalogue_at(dir);
        }
    }

    fn write_fixture(&self, dir: &Path) -> Option<Result<(), String>> {
        Some(write_fixture(dir).map_err(|t| format!("{t:?}")))
    }

    /// The listing of the fixture's person's source, every category of it,
    /// as a footer's `F` hands one over.
    fn handed_fixture(&self, view: &mut View, store: Store) -> bool {
        use ronoctua_broadcast::catalog::Scope;
        view.open_the_handed_listing(Handed {
            listing: Listing::of(store, FIXTURE_SOURCE, Scope::AllCategories, None),
            source: FIXTURE_SOURCE.to_owned(),
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-player-catalog-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    /// FAILS IF: the ring loses a source, shows the temporary search, shows
    /// a source of the adult catalogue while it is shut (or hides one while
    /// it is open), or turning it does not come round.
    #[test]
    fn the_ring_is_every_source_but_the_search() {
        let mut s = State::default();
        for id in ["archive", "p1", SEARCHED, "p2"] {
            s.catalog.sources.push(SourceStatus {
                id: SourceId(id.into()),
                kind: SourceKind::Playlist,
                title: None,
                shown: "x.example/…".into(),
                adult: id == "p2",
                posters: false,
                plain: false,
                items: 0,
                busy: false,
                trouble: None,
            });
        }
        let r = ring(Some(&s), false);
        assert_eq!(r.len(), table::DEFAULT_SOURCES.len() + 1);
        assert_eq!(r.last().map(String::as_str), Some("p1"));
        assert!(!r.iter().any(|x| x == SEARCHED));
        let open = ring(Some(&s), true);
        assert_eq!(open.last().map(String::as_str), Some("p2"), "{open:?}");
        // THE RING WITH "NONE" at its head: a filter's ring comes round.
        let v = [1, 2, 3];
        assert_eq!(turned(&v, None, false), Some(&1));
        assert_eq!(turned(&v, Some(&3), false), None);
        assert_eq!(turned(&v, None, true), Some(&3));
        assert_eq!(turned(&v, Some(&1), true), None);
        assert_eq!(turned::<u8>(&[], None, false), None);
    }

    /// FAILS IF: a source read longer ago than its freshness is not stale,
    /// or one never read is.
    #[test]
    fn a_source_is_stale_past_its_freshness() {
        assert!(stale(None, 60, 1_000));
        assert!(!stale(Some(1_000), 60, 1_000 + 60 * 60));
        assert!(stale(Some(1_000), 60, 1_000 + 60 * 60 + 1));
        assert_eq!(
            fresh_minutes("lives", SourceKind::Lives),
            table::FRESH_LIVES_MINUTES
        );
        assert_eq!(
            fresh_minutes("p1", SourceKind::Feed),
            table::FRESH_FEED_MINUTES
        );
    }

    /// FAILS IF: the letters stop standing apart where the row has room for
    /// it, stop standing side by side where it has room only for that, or
    /// are drawn as cells where not even that fits — the ring's case.
    #[test]
    fn the_letters_stand_apart_side_by_side_or_are_a_ring() {
        // "All", A, B and a CJK letter two cells wide.
        let widths = [3, 1, 1, 2];
        // Apart: 3 + 1, then 1 + 1 + 2, and a cell between two letters.
        assert_eq!(letter_air(&widths, 10), Some(1));
        assert_eq!(letter_air(&widths, 9), Some(0));
        assert_eq!(letter_air(&widths, 8), Some(0));
        assert_eq!(letter_air(&widths, 7), None);
        // "All" alone is a row of one cell, and nothing is no row.
        assert_eq!(letter_air(&[3], 4), Some(1));
        assert_eq!(letter_air(&[], 80), None);
    }

    /// FAILS IF: the letters of a listing stop being the ones its titles are
    /// filed under — Ç, Ñ, Cyrillic and CJK among them — or a letter cell
    /// stops taking its letter; or a poster layout asks for more rows than
    /// a screen of posters.
    #[test]
    fn a_listing_reads_its_page_its_letters_and_its_facets() {
        let dir = scratch("fixture");
        write_fixture(&dir).expect("the fixture");
        let mut listing = Listing::of(Store::Main, "p1790000000", Scope::AllCategories, None);
        let mut readers = Readers::default();
        let Ok(Got::List(listed)) =
            answer(&mut readers, &dir, &Want::List(Box::new(listing.clone())))
        else {
            panic!("a listing");
        };
        for c in ['#', 'A', 'Ç', 'E', 'Ñ', 'А', '東'] {
            assert!(listed.letters.contains(&c), "{c} in {:?}", listed.letters);
        }
        assert_eq!(listed.letters.first(), Some(&'#'));
        assert!(listed.page.total > 100, "{}", listed.page.total);
        assert!(listed.page.pages > 1);
        assert!(!listed.countries.is_empty() && !listed.languages.is_empty());
        // THE ADULT GROUP is not in the main file.
        assert!(
            !listed
                .page
                .rows
                .iter()
                .any(|r| r.title.starts_with("Noite"))
        );
        listing.letter = Some('Ç');
        let Ok(Got::List(one)) = answer(&mut readers, &dir, &Want::List(Box::new(listing.clone())))
        else {
            panic!("a letter's listing");
        };
        assert_eq!(one.page.total, 1);
        assert_eq!(one.page.rows[0].title, "Çatalhöyük");
        listing.layout = Layout::Posters;
        listing.per_page = 200;
        assert_eq!(
            listing.rows_asked(),
            u32::from(tokens::space::CATALOG_POSTERSMOST)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: the home of a source stops saying how many of its items the
    /// adult file holds while it is shut — or tells its themes then.
    #[test]
    fn a_shut_adult_file_says_how_many_and_nothing_else() {
        let dir = scratch("adult");
        write_fixture(&dir).expect("the fixture");
        let mut readers = Readers::default();
        let want = |adult| Want::Home {
            source: "p1790000000".into(),
            adult,
        };
        let Ok(Got::Home(shut)) = answer(&mut readers, &dir, &want(false)) else {
            panic!("a home");
        };
        assert_eq!(shut.adult_items, 5);
        assert!(shut.adult_themes.is_empty() && shut.adult_categories.is_empty());
        let Ok(Got::Home(open)) = answer(&mut readers, &dir, &want(true)) else {
            panic!("a home");
        };
        assert_eq!(open.adult_categories.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: the views's catalogue pages stop drawing: channels by theme
    /// with its source, themes and categories, and a listing with its
    /// letters as cells a click takes.
    #[test]
    fn the_pages_draw_and_a_letter_is_a_click() {
        let dir = scratch("pages");
        write_fixture(&dir).expect("the fixture");
        let t = texts();
        let mut s = State::default();
        s.catalog.sources.push(SourceStatus {
            id: SourceId("p1790000000".into()),
            kind: SourceKind::Playlist,
            title: Some("Minha lista".into()),
            shown: "iptv.example/…".into(),
            adult: false,
            posters: true,
            plain: true,
            items: 0,
            busy: false,
            trouble: None,
        });
        let s = Some(&s);
        let mut v = View::blank(None).plugging(&PAGE);
        v.catalogue_at(dir.clone());
        v.catalog_mut().source = Some("p1790000000".into());
        v.key(Press::Go(PageId::Channels), s, 100, &t);
        let rows = v.rows(s, 100, &t);
        let said: String = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join("|");
        assert!(said.contains("Filmes"), "{said}");
        assert!(said.contains(&t.t(key::PLAYER_ADULT_LOCKED)), "{said}");
        let films = rows
            .iter()
            .position(|r| r.fields.iter().any(|f| f.text == "Filmes"))
            .expect("the films' row");
        v.grabbed(films);
        v.key(Press::Enter, s, 100, &t);
        assert_eq!(v.page(), PageId::Catalog);
        let rows = v.rows(s, 100, &t);
        let letters = rows
            .iter()
            .position(|r| matches!(r.target, Target::CatalogLetter(_)))
            .expect("the letter row");
        let cell = rows[letters]
            .cells
            .iter()
            .find(|c| c.target == Target::CatalogLetterCell(Some('B')))
            .expect("a cell for B");
        v.clicked_at(letters, cell.from, &rows);
        v.key(Press::Enter, s, 100, &t);
        assert_eq!(
            v.catalog().listing.as_ref().and_then(|l| l.letter),
            Some('B')
        );
        let rows = v.rows(s, 100, &t);
        let said: String = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join("|");
        assert!(
            said.contains("Barravento") && !said.contains("Casablanca"),
            "{said}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a password is sent anywhere but a masked question and the
    /// door's own check, or the lock stays open once shut.
    #[test]
    fn the_adult_lock_asks_masked_and_shuts() {
        let t = texts();
        let mut v = View::blank(None).plugging(&PAGE);
        let Some(Errand::Ask(q)) =
            v.catalog_turned(&Target::CatalogUnlock, Press::Enter, None, 80, &t)
        else {
            panic!("a question");
        };
        assert!(q.secret, "the password is typed masked");
        let Errand::Unlock(secret) = v.catalog_answered(ASK_PASSWORD, "hunter2", None, 80, &t)
        else {
            panic!("the check");
        };
        assert_eq!(secret.expose(), "hunter2");
        assert!(!format!("{secret:?}").contains("hunter2"));
        v.unlocked(Ok(()));
        assert!(v.catalog().lock.open());
        v.catalog_mut().lock.shut();
        assert!(!v.catalog().lock.open());
        v.unlocked(Err(ronoctua_pam::Refusal::Wrong));
        assert!(!v.catalog().lock.open());
    }

    /// Every field the rows hold, joined.
    fn said(rows: &[crate::player_rows::Row]) -> String {
        rows.iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join("|")
    }

    /// A source's status, as a state says it.
    fn status(id: &str, kind: SourceKind) -> SourceStatus {
        SourceStatus {
            id: SourceId(id.into()),
            kind,
            title: None,
            shown: "x.example/…".into(),
            adult: false,
            posters: false,
            plain: false,
            items: 1,
            busy: false,
            trouble: None,
        }
    }

    /// FAILS IF: the sources' page says "not read yet" of a source that was
    /// read because the ring is on another one — it read the ring's row only
    /// — or asks only the ring's source to be read again when every source
    /// on it is stale; or a date is said in a zone it does not name.
    #[test]
    fn the_sources_page_reads_every_source_s_standing() {
        let dir = scratch("standing");
        write_fixture(&dir).expect("the fixture");
        let t = texts();
        let mut s = State::default();
        s.catalog.sources = vec![
            status("archive", SourceKind::Archive),
            status("p1790000000", SourceKind::Playlist),
        ];
        let st = Some(&s);
        let mut v = View::blank(None).plugging(&PAGE);
        v.catalogue_at(dir.clone());
        // THE RING ON THE PERSON'S SOURCE, and the archive's hosts let: its
        // standing is its reading, not the row that lets them.
        v.catalog_mut().source = Some("p1790000000".into());
        v.catalog_mut().let_hosts = DefaultSource::of("archive")
            .expect("the archive")
            .hosts
            .iter()
            .map(|h| (*h).to_owned())
            .collect();
        v.push(PageId::Sources, None);
        v.settle_in(st, 100, &t);
        let page = said(&v.rows(st, 100, &t));
        assert!(!page.contains(&t.t(key::PLAYER_CATALOG_NEVER)), "{page}");
        // BOTH DATED, in UTC — this view knows no zone — and saying so.
        let date =
            crate::chrome::when_text(1_790_000_000, &ronoctua_files::when::Zone::utc(), true, &t);
        assert_eq!(page.matches(&date).count(), 2, "{date} in {page}");
        // AND EACH IS OWED A READING, not the ring's alone.
        let owed: Vec<&str> = v
            .catalog()
            .owed
            .iter()
            .filter_map(|a| match a {
                Ask::Ingest { source, .. } => Some(source.0.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            owed.contains(&"archive") && owed.contains(&"p1790000000"),
            "{owed:?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: the whole archive's search draws "nothing matches" while it
    /// is under way or after it failed — a refusal shown as an absence — or a
    /// failed one has no row that asks it again; or its listing is narrowed
    /// again by the words the archive already matched; or its posters are not
    /// asked by the archive's switch.
    #[test]
    fn a_search_of_the_archive_says_it_is_under_way_or_why_it_failed() {
        use ronoctua_broadcast::catalog::{Harvest, NewItem, NewSource, Writer};
        let dir = scratch("searching");
        write_fixture(&dir).expect("the fixture");
        // WHAT THE DAEMON WROTE of the search, a poster named and not yet
        // fetched.
        let key_of = format!("{SEARCH_PREFIX}robots");
        let mut h = Harvest::default();
        h.category(&key_of, "robots", None, false);
        h.items.push(NewItem {
            ext_id: "Robot_Monster".into(),
            kind: ItemKind::Video,
            title: "Robot Monster".into(),
            also: Vec::new(),
            description: None,
            country: None,
            language: None,
            height: None,
            duration_s: None,
            year: Some(1953),
            licence: None,
            popularity: 1,
            published_s: None,
            poster_url: Some("https://archive.org/services/img/Robot_Monster".into()),
            stream: ronoctua_player::own::Stream::typed(
                "https://archive.org/details/Robot_Monster",
            ),
            categories: vec![key_of.clone()],
        });
        let mut w = Writer::open(&dir, 256 << 20, 1_790_000_000).expect("the writer");
        w.replace(
            &NewSource {
                id: SEARCHED.into(),
                kind: SourceKind::Archive,
                title: "robots".into(),
                shown: "archive.org".into(),
                adult: false,
                posters_on: true,
            },
            &h,
            1_790_000_000,
        )
        .expect("the search's rows");
        drop(w);
        let t = texts();
        let searched = |busy, trouble| SourceStatus {
            title: Some("robots".into()),
            busy,
            trouble,
            ..status(SEARCHED, SourceKind::Archive)
        };
        let mut s = State::default();
        s.catalog.sources = vec![
            status("archive", SourceKind::Archive),
            searched(false, None),
        ];
        let mut v = View::blank(None).plugging(&PAGE);
        v.catalogue_at(dir.clone());
        v.catalog_mut().source = Some("archive".into());
        v.catalog_mut().listing = Some(Listing {
            words: Some("robots".into()),
            ..Listing::of(Store::Main, "archive", Scope::AllCategories, None)
        });
        v.push(PageId::Catalog, None);
        v.settle_in(Some(&s), 100, &t);
        let rows = v.rows(Some(&s), 100, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::CatalogSearchArchive)
            .expect("the whole archive's row");
        v.grabbed(at);
        let Errand::TellSaying(Ask::Ingest { source, category }, _) =
            v.key(Press::Enter, Some(&s), 100, &t)
        else {
            panic!("the search asked of the daemon");
        };
        assert_eq!(
            (source.0.as_str(), category.as_deref()),
            ("archive", Some("search:robots"))
        );
        let l = v.catalog().listing.clone().expect("the search's listing");
        assert_eq!((l.source.as_str(), l.words.as_deref()), (SEARCHED, None));
        let nothing = [
            t.t(key::PLAYER_CATALOG_NOTHING_FOUND),
            t.t(key::PLAYER_CATALOG_EMPTY),
        ];
        // UNDER WAY: said, and no "nothing" — with its rows not written yet.
        if let Some(l) = v.catalog_mut().listing.as_mut() {
            l.scope = Scope::Category(CategoryId::of(SEARCHED, "search:trains"));
        }
        s.catalog.sources[1] = searched(true, None);
        v.settle_in(Some(&s), 100, &t);
        let page = said(&v.rows(Some(&s), 100, &t));
        let busy = t.t_with(key::PLAYER_CATALOG_SEARCHING, &[("words", "robots")]);
        assert!(page.contains(&busy), "{page}");
        assert!(!nothing.iter().any(|n| page.contains(n)), "{page}");
        // REFUSED: why, and the row that asks again — no "nothing".
        s.catalog.sources[1] = searched(
            false,
            Some(ronoctua_player::wire::Trouble::Source {
                why: ronoctua_broadcast::sources::Refused::Unreachable,
            }),
        );
        v.settle_in(Some(&s), 100, &t);
        let rows = v.rows(Some(&s), 100, &t);
        let page = said(&rows);
        assert!(
            page.contains(&t.t(key::PLAYER_TROUBLE_SOURCE_UNREACHABLE)),
            "{page}"
        );
        assert!(!nothing.iter().any(|n| page.contains(n)), "{page}");
        let again = rows
            .iter()
            .position(|r| r.target == Target::CatalogSearchArchive)
            .expect("a row that asks again");
        v.grabbed(again);
        let Errand::TellSaying(Ask::Ingest { category, .. }, _) =
            v.key(Press::Enter, Some(&s), 100, &t)
        else {
            panic!("asked again");
        };
        assert_eq!(category.as_deref(), Some("search:robots"));
        // ITS POSTERS, by the archive's switch: none while it is off...
        s.catalog.sources[1] = searched(false, None);
        if let Some(l) = v.catalog_mut().listing.as_mut() {
            l.layout = Layout::Posters;
        }
        v.catalog_mut().owed.clear();
        v.settle_in(Some(&s), 100, &t);
        let posters = |v: &View| {
            v.catalog()
                .owed
                .iter()
                .any(|a| matches!(a, Ask::Posters { items } if !items.is_empty()))
        };
        assert!(!posters(&v), "{:?}", v.catalog().owed);
        // ...and asked once it is on — not held back as asked already while
        // its switch was off.
        s.catalog.sources[0].posters = true;
        v.settle_in(Some(&s), 100, &t);
        assert!(posters(&v), "{:?}", v.catalog().owed);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: an adult listing handed over to a window opens without the
    /// password — or is never opened once it is taken — or the window that
    /// holds it does not ask, masked; or letting the question go keeps it
    /// waiting; or `F` hands over a ring's source worked out again without
    /// the state (a person's source turned into the first default), or a
    /// window that takes the stage itself hands anything over.
    #[test]
    fn an_adult_listing_handed_over_waits_for_the_password() {
        let t = texts();
        let adult = Listing::of(Store::Adult, "p1790000000", Scope::AllCategories, None);
        let handed = || Handed {
            listing: adult.clone(),
            source: "p1790000000".into(),
        };
        let mut v = View::blank(None).plugging(&PAGE);
        v.open_the_handed_listing(handed());
        assert_eq!(v.page(), PageId::Channels);
        assert!(v.listing_waits() && v.catalog().listing.is_none());
        let Some(Errand::Ask(q)) = v.lock_owed() else {
            panic!("the password asked");
        };
        assert!(q.secret && q.id == ASK_PASSWORD);
        assert!(v.lock_owed().is_none(), "asked once while it is typed");
        assert!(matches!(
            v.catalog_answered(ASK_PASSWORD, "hunter2", None, 80, &t),
            Errand::Unlock(_)
        ));
        // WRONG: still waiting, and asked again.
        v.unlocked(Err(ronoctua_pam::Refusal::Wrong));
        assert!(v.listing_waits());
        assert!(matches!(v.lock_owed(), Some(Errand::Ask(_))));
        v.catalog_answered(ASK_PASSWORD, "hunter3", None, 80, &t);
        v.unlocked(Ok(()));
        assert_eq!(v.page(), PageId::Catalog);
        assert_eq!(v.catalog().listing.as_ref(), Some(&adult));
        assert!(!v.listing_waits());
        // LET GO: the listing goes with its question.
        let mut w = View::blank(None).plugging(&PAGE);
        w.open_the_handed_listing(handed());
        w.lock_owed();
        w.let_go();
        assert!(!w.listing_waits());
        // A LISTING OF THE MAIN FILE opens at once.
        let mut m = View::blank(None).plugging(&PAGE);
        m.open_the_handed_listing(Handed {
            listing: Listing::of(Store::Main, "archive", Scope::AllCategories, None),
            source: "archive".into(),
        });
        assert_eq!(m.page(), PageId::Catalog);
        // `F` HANDS the ring's source as it is — a person's — and a window
        // that takes the stage itself hands nothing.
        let mut f = View::blank(None).plugging(&PAGE);
        f.catalog_mut().hands_over = true;
        f.catalog_mut().source = Some("p1790000000".into());
        f.catalog_mut().listing = Some(adult.clone());
        assert_eq!(f.handed(), Some(handed()));
        f.takes_its_own_stage();
        assert_eq!(f.handed(), None);
    }

    /// FAILS IF: a system with no lock service says so only in an echo that
    /// passes — the page goes on offering a row that "opens with your
    /// password" — or a window keeps asking a password it cannot check; or a
    /// wrong password is kept as a reason the lock cannot open.
    #[test]
    fn a_system_with_no_lock_says_so_on_the_page() {
        let dir = scratch("no-service");
        write_fixture(&dir).expect("the fixture");
        let t = texts();
        let mut s = State::default();
        s.catalog.sources = vec![status("p1790000000", SourceKind::Playlist)];
        let st = Some(&s);
        let mut v = View::blank(None).plugging(&PAGE);
        v.catalogue_at(dir.clone());
        v.catalog_mut().source = Some("p1790000000".into());
        v.push(PageId::Channels, None);
        v.unlocked(Err(ronoctua_pam::Refusal::NoService));
        v.settle_in(st, 100, &t);
        let rows = v.rows(st, 100, &t);
        let page = said(&rows);
        assert!(page.contains(&t.t(key::PLAYER_ADULT_NO_SERVICE)), "{page}");
        assert!(page.contains(&t.t(key::PLAYER_ADULT_SHUT)), "{page}");
        assert!(
            !rows.iter().any(|r| r.target == Target::CatalogUnlock),
            "a row that would ask a lock there is not"
        );
        v.catalog_mut().waiting = Some(Listing::of(
            Store::Adult,
            "p1790000000",
            Scope::AllCategories,
            None,
        ));
        assert!(v.lock_owed().is_none(), "no password asked of no lock");
        v.unlocked(Err(ronoctua_pam::Refusal::Wrong));
        v.settle_in(st, 100, &t);
        let rows = v.rows(st, 100, &t);
        assert!(rows.iter().any(|r| r.target == Target::CatalogUnlock));
        assert!(!said(&rows).contains(&t.t(key::PLAYER_ADULT_NO_SERVICE)));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a poster decoded for the catalogue is not drawn with no
    /// player running — the doors asked the player's state first, and channels
    /// by theme are there with none.
    #[test]
    fn a_poster_is_drawn_with_no_player() {
        let dir = scratch("no-player");
        write_fixture(&dir).expect("the fixture");
        let mut v = View::blank(None).plugging(&PAGE);
        v.catalogue_at(dir.clone());
        let mut readers = Readers::default();
        let listing = Listing::of(Store::Main, "archive", Scope::AllCategories, None);
        let Ok(Got::List(listed)) = answer(&mut readers, &dir, &Want::List(Box::new(listing)))
        else {
            panic!("a listing");
        };
        v.catalog_mut()
            .posters
            .look(&listed.page.rows, 0, None, &|_| true);
        let url = listed.page.rows[0].poster_url.clone().expect("a poster");
        let key = v.catalog().posters.key_of(&url);
        assert!(v.picture(None, &key).is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a page typed past the last is kept past it — the page
    /// before it then seems dead for as many presses — or the page before a
    /// page kept past the last (the listing shrank under it) is not the one
    /// before the last.
    #[test]
    fn a_page_past_the_last_is_the_last() {
        assert_eq!(earlier(999, Some(3)), 2);
        assert_eq!(earlier(3, Some(3)), 2);
        assert_eq!(earlier(1, Some(3)), 1);
        assert_eq!(earlier(5, None), 4);
        let dir = scratch("page-past");
        write_fixture(&dir).expect("the fixture");
        let t = texts();
        let mut v = View::blank(None).plugging(&PAGE);
        v.catalogue_at(dir.clone());
        v.catalog_mut().listing = Some(Listing::of(
            Store::Main,
            "p1790000000",
            Scope::AllCategories,
            None,
        ));
        v.push(PageId::Catalog, None);
        v.settle_in(None, 100, &t);
        let pages = v.pages_known().expect("answered");
        assert!(pages > 1);
        v.typing = Some(Typing::CatalogPage);
        v.catalog_answered(ASK_PAGE, "999", None, 100, &t);
        let page = |v: &View| v.catalog().listing.as_ref().map(|l| l.page);
        assert_eq!(page(&v), Some(pages));
        v.listing_pressed(ListingKey::Earlier, None, 100, &t);
        assert_eq!(page(&v), Some(pages - 1));
        let _ = std::fs::remove_dir_all(dir);
    }
}
