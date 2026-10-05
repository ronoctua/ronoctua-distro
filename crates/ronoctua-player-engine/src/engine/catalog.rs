//! The local catalogue, from the daemon's side: its one writer, a thread per
//! source that ingests it, the person's own sources' file and the posters.
//!
//! # An ingest is an event, never a clock
//!
//! Nothing here wakes on a timer. A door that SHOWS a source whose last
//! ingest is older than its freshness (`assets/channels.nix`, `fresh`) — or
//! that was never ingested — asks for it ([`Ask::Ingest`]), and the daemon
//! reads the source again on that source's own thread: one thread per
//! source, started the first time it is asked and asleep on its mailbox
//! after that, so a slow list of fifty thousand channels never holds the
//! NASA library behind it. An ingest asked while one of the same source is
//! under way is the same ingest.
//!
//! # Replaced whole, in one transaction
//!
//! What a source gave is handed to the catalogue's one [`Writer`], which
//! replaces the source's rows in one transaction per file, committed together
//! (`ronoctua_broadcast::catalog`): a door reads the old rows or the new
//! ones. A source that gave nothing keeps its rows, and the catalogue records
//! why ([`Writer::failed`]) — the door says the source is stale and why.
//!
//! # A source removed while it is read stays removed
//!
//! A list of fifty thousand channels takes a while, and the person may remove
//! its source meanwhile. Removing raises the source's thread's flag before it
//! takes the writer ([`Catalog::withdraw`]), and an ingest looks at the flag
//! with the writer in hand ([`ingest`]): it either wrote before the removal
//! took the writer — and the removal deletes what it wrote — or it writes
//! nothing. Either way no row of a removed source is left behind.
//!
//! # A temporary search
//!
//! The archive's snapshot holds each collection's most downloaded; a search
//! of the words a person typed across the archive's collections is an ingest
//! of its own — the category `search:<words>` of [`SEARCHED`], a source the
//! ring never shows — replaced by the next search, and gone when the daemon
//! starts again. It has a status of its own in the state, so a door says it
//! is under way, or why it gave nothing, and never draws a search that failed
//! as one that found nothing; a search asked while another runs is read after
//! it, and only the newest one's ending is said.
//!
//! # The person's sources
//!
//! `sources.json` (`ronoctua_broadcast::sources::Book`) is written by the
//! daemon alone, `0600` from its first byte: a pasted IPTV address carries the
//! person's account. A pasted link is recognised here — its address alone, or
//! its first bytes ([`sources::sniff`]) — and refused in a sentence when it is
//! none of the shapes read. An `http` list is taken, in the clear, and the
//! state says so ([`SourceStatus::plain`]): the owner's decision.
//!
//! # Posters, for what is on screen
//!
//! A door names the items it shows ([`Ask::Posters`]); the daemon fetches the
//! posters it has not got, of the sources whose switch is on, one host at a
//! time and never faster than [`POSTER_PACE`] to one host, into the
//! catalogue's `posters` folder under `playerPostersMib`. The switch of a
//! default source names its hosts ([`DefaultSource::poster_hosts`]) and turning
//! it on lets them; a person's source's posters come from wherever its list
//! says, over `https`, and its switch is the consent. In the clear, only from
//! the host of the person's own `http` list — the one they pasted and were
//! told is not encrypted ([`poster_allowed`]): a list names any address it
//! likes, and a poster asked in the clear of a machine on the person's own
//! network is a request nobody chose.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ronoctua_broadcast::{
    self as ask, Fetch, Net, Web, archive,
    cache::Cache,
    catalog::{self, Harvest, NewSource, Store, Trouble as CatalogTrouble, Writer},
    commons,
    internet::Internet,
    iptv::{self, Index},
    nasa, peertube,
    sources::{
        self, Address, Book, Kept, Kind, PersonSource, Refused as SourceRefused, Sniffed,
        YouTubeList, defaults,
    },
    table::{self, DefaultSource},
};
use ronoctua_player::wire::{
    CatalogSummary, ItemId, Say, SourceId, SourceStatus, SourceUrl, SourcesTrouble, Trouble,
};

use super::{Engine, Refused, Shared, desk::with_engine};

mod policy {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// One mebibyte.
const MIB: u64 = 1024 * 1024;

/// The source a temporary search of the archive is written as: never in a
/// door's ring, replaced by the next search, and removed when the writer
/// opens.
pub const SEARCHED: &str = "archive-search";

/// The prefix of an ingest's category that is a search of the archive's
/// collections for the words after it ([`Ask::Ingest`]).
pub const SEARCH_PREFIX: &str = "search:";

/// The most items one temporary search keeps.
const SEARCH_MOST: usize = 300;

/// The least time between two requests to one host, within one ingest and
/// among the posters: a list of a thousand pages is not a thousand requests
/// at once.
const PACE: Duration = Duration::from_millis(400);

/// The least time between two posters asked of one host.
pub const POSTER_PACE: Duration = Duration::from_millis(250);

/// The most one poster may weigh.
const POSTER_MOST_BYTES: u64 = 2 * MIB;

/// The most posters one ask names; a door names the ones it shows.
const POSTERS_ASKED_MOST: usize = 512;

/// The catalogue's files, shared by every ingest thread: the folder, and the
/// one writer, opened the first time something is written.
struct Stores {
    /// The catalogue's folder; `None` with no cache root.
    dir: Option<PathBuf>,
    /// The writer, once opened.
    writer: Mutex<Option<Writer>>,
}

impl Stores {
    /// Runs `f` on the writer, opening it the first time — the temporary
    /// search removed then, as a new daemon has none.
    fn with_writer<T>(
        &self,
        f: impl FnOnce(&mut Writer) -> Result<T, CatalogTrouble>,
    ) -> Result<T, CatalogTrouble> {
        let mut held = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        if held.is_none() {
            let dir = self.dir.as_ref().ok_or(CatalogTrouble::NoHome)?;
            let mut w = Writer::open(
                dir,
                u64::from(policy::PLAYER_CATALOG_MIB) * MIB,
                now_s(SystemTime::now()),
            )?;
            if let Err(t) = w.remove(SEARCHED) {
                tracing::info!(trouble = ?t, "the last temporary search was not removed");
            }
            *held = Some(w);
        }
        let w = held.as_mut().ok_or(CatalogTrouble::Unwritable)?;
        f(w)
    }
}

/// What one ingest reads.
#[derive(Clone)]
enum What {
    /// A default source, as the table declares it.
    Default(&'static DefaultSource),
    /// One of the person's.
    Person(PersonSource),
    /// A search of the archive's collections for these words.
    Search(String),
}

/// One ingest, for a source's thread.
struct Job {
    /// What is read.
    what: What,
    /// Whether its posters are on, as the catalogue records it.
    posters: bool,
    /// The bytes already fetched when the source was added (a list, a feed):
    /// not asked twice.
    fetched: Option<Vec<u8>>,
}

impl Job {
    /// The words a search asks — `None` for every other ingest.
    fn searched(&self) -> Option<String> {
        match &self.what {
            What::Search(words) => Some(words.clone()),
            What::Default(_) | What::Person(_) => None,
        }
    }
}

/// How an ingest ended.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    /// Written: how many items, and what the source calls itself.
    Written {
        /// Items, in the two files together.
        items: u64,
        /// Its own name, when it says one.
        title: Option<String>,
    },
    /// The source gave nothing, and why; its rows stay.
    Refused(SourceRefused),
    /// The catalogue did not take it.
    Catalog(CatalogTrouble),
    /// The source was removed while it was read: nothing was written.
    Withdrawn,
}

/// A source's thread, as the catalogue holds it: its mailbox, and the flag
/// that withdraws what it reads.
struct Worker {
    /// Its mailbox.
    tx: mpsc::Sender<Job>,
    /// Raised when the source is removed ([`Catalog::withdraw`]): nothing
    /// its thread read after that is written ([`ingest`]).
    withdrawn: Arc<AtomicBool>,
}

/// The catalogue, as the engine holds it.
pub(super) struct Catalog {
    /// Its files.
    stores: Arc<Stores>,
    /// Where the person's sources are kept; `None` with no config root.
    book: Option<Book>,
    /// The book as last read or written.
    kept: Kept,
    /// Why the book could not be read — nothing is written over it then.
    book_trouble: Option<SourcesTrouble>,
    /// Every source, the defaults first.
    statuses: Vec<SourceStatus>,
    /// Moves with every write.
    generation: u64,
    /// Moves when posters arrived.
    posters_generation: u64,
    /// Each source's thread, once it runs.
    workers: HashMap<String, Worker>,
    /// The words of the newest search of the archive asked: an older one's
    /// ending is not said over it.
    searched: Option<String>,
    /// The posters' thread's mailbox, once it runs.
    posters: Option<mpsc::Sender<Vec<Poster>>>,
    /// Raised when the player closes: nothing found after it is posted.
    stop: Arc<AtomicBool>,
}

impl Catalog {
    /// The catalogue of this instance: the person's sources read, nothing
    /// opened — the writer waits for the first ingest.
    pub(super) fn open() -> Self {
        let book = Book::player().ok();
        let (kept, book_trouble) = match book.as_ref().map(Book::read) {
            Some(Ok(kept)) => (kept, None),
            Some(Err(sources::Unread::Unreadable)) => {
                (Kept::default(), Some(SourcesTrouble::Unreadable))
            }
            Some(Err(sources::Unread::Unparseable)) => {
                (Kept::default(), Some(SourcesTrouble::Unparseable))
            }
            None => (Kept::default(), Some(SourcesTrouble::NoHome)),
        };
        if let Some(t) = book_trouble {
            tracing::warn!(trouble = ?t, "the person's catalogue sources could not be read");
        }
        let statuses = statuses_of(&kept, &[]);
        Self {
            stores: Arc::new(Stores {
                dir: catalog::folder(),
                writer: Mutex::new(None),
            }),
            book,
            kept,
            book_trouble,
            statuses,
            generation: 0,
            posters_generation: 0,
            workers: HashMap::new(),
            searched: None,
            posters: None,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The thread of `id` stopped writing — the source was removed: its flag
    /// raised BEFORE the removal takes the writer, so an ingest of it that
    /// has not written yet never will ([`ingest`]), and its mailbox dropped,
    /// so the thread ends once it is done.
    fn withdraw(&mut self, id: &str) {
        if let Some(w) = self.workers.remove(id) {
            w.withdrawn.store(true, Ordering::SeqCst);
        }
    }

    /// A search of the archive for `words` asked: its status under way, and
    /// whether to read it — not when that very search is under way already.
    fn ask_search(&mut self, words: &str) -> bool {
        let same = self.searched.as_deref() == Some(words);
        let s = self.searching(words);
        if s.busy && same {
            return false;
        }
        s.busy = true;
        s.trouble = None;
        true
    }

    /// Whether an ingest that searched `searched` — `None`: no search — is
    /// one whose ending is said: every ingest but a search another one
    /// asked after it replaced.
    fn is_current(&self, searched: Option<&str>) -> bool {
        searched.is_none() || searched == self.searched.as_deref()
    }

    /// The status of the temporary search for `words`, under way — made the
    /// first time one is asked.
    fn searching(&mut self, words: &str) -> &mut SourceStatus {
        self.searched = Some(words.to_owned());
        let at = if let Some(at) = self.statuses.iter().position(|s| s.id.0 == SEARCHED) {
            at
        } else {
            self.statuses.push(SourceStatus {
                id: SourceId(SEARCHED.to_owned()),
                kind: Kind::Archive,
                title: None,
                shown: archive::HOST.to_owned(),
                adult: false,
                // ITS POSTERS ARE THE ARCHIVE'S, by the archive's switch
                // (`Engine::poster_rules`): this one says nothing.
                posters: false,
                plain: false,
                items: 0,
                busy: false,
                trouble: None,
            });
            self.statuses.len() - 1
        };
        let s = &mut self.statuses[at];
        s.title = Some(words.to_owned());
        s
    }

    /// The player is closing: nothing more is posted.
    pub(super) fn end(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// The summary the state carries.
    pub(super) fn summary(&self) -> CatalogSummary {
        CatalogSummary {
            generation: self.generation,
            posters_generation: self.posters_generation,
            sources: self.statuses.clone(),
            trouble: self.book_trouble.map(|why| Trouble::Sources { why }),
        }
    }

    fn status_mut(&mut self, id: &str) -> Option<&mut SourceStatus> {
        self.statuses.iter_mut().find(|s| s.id.0 == id)
    }

    /// The statuses again after the book changed, what each knew kept.
    fn restate(&mut self) {
        self.statuses = statuses_of(&self.kept, &self.statuses);
    }

    /// What an ingest of `id` reads, and whether its posters are on.
    fn what_of(&self, id: &str) -> Option<(What, bool)> {
        if let Some(d) = DefaultSource::of(id) {
            let on = self
                .kept
                .default_posters
                .get(id)
                .copied()
                .unwrap_or(d.posters_on);
            return Some((What::Default(d), on));
        }
        self.kept
            .sources
            .iter()
            .find(|s| s.id == id)
            .map(|s| (What::Person(s.clone()), s.posters))
    }

    /// Whose posters may be fetched, and how: every source whose switch is on.
    fn poster_rules(&self) -> Arc<BTreeMap<String, Rule>> {
        let mut rules = BTreeMap::new();
        for s in self
            .statuses
            .iter()
            .filter(|s| s.posters && s.id.0 != SEARCHED)
        {
            let rule = match DefaultSource::of(&s.id.0) {
                Some(d) if !d.poster_hosts.is_empty() => Rule::Hosts(d.poster_hosts),
                // A DEFAULT SOURCE'S "any site" is any `https` one: nothing of
                // the distro's own is asked in the clear.
                Some(_) => Rule::Anywhere { clear_from: None },
                // A PERSON'S: any `https` site, and in the clear only the
                // host of their own `http` list.
                None => Rule::Anywhere {
                    clear_from: self
                        .kept
                        .sources
                        .iter()
                        .find(|p| p.id == s.id.0)
                        .and_then(|p| plain_host(p.address.expose())),
                },
            };
            rules.insert(s.id.0.clone(), rule);
        }
        // THE TEMPORARY SEARCH is the archive's.
        if let Some(r) = rules.get("archive").cloned() {
            rules.insert(SEARCHED.to_owned(), r);
        }
        Arc::new(rules)
    }
}

/// Every source's status: the table's defaults, then the person's — each
/// keeping what `before` knew of it (busy, its items, its trouble).
fn statuses_of(kept: &Kept, before: &[SourceStatus]) -> Vec<SourceStatus> {
    let known = |id: &str| before.iter().find(|s| s.id.0 == id);
    let mut out: Vec<SourceStatus> = table::DEFAULT_SOURCES
        .iter()
        .map(|d| SourceStatus {
            id: SourceId(d.id.to_owned()),
            kind: d.kind,
            title: None,
            shown: d.hosts.first().copied().unwrap_or_default().to_owned(),
            adult: false,
            posters: kept
                .default_posters
                .get(d.id)
                .copied()
                .unwrap_or(d.posters_on),
            plain: false,
            items: known(d.id).map_or(0, |s| s.items),
            busy: known(d.id).is_some_and(|s| s.busy),
            trouble: known(d.id).and_then(|s| s.trouble.clone()),
        })
        .collect();
    out.extend(kept.sources.iter().map(|p| SourceStatus {
        id: SourceId(p.id.clone()),
        kind: p.kind,
        title: (!p.title.is_empty()).then(|| p.title.clone()),
        shown: p.address.shown(),
        adult: p.adult,
        posters: p.posters,
        plain: sources::is_plain(p.address.expose()),
        items: known(&p.id).map_or(0, |s| s.items),
        busy: known(&p.id).is_some_and(|s| s.busy),
        trouble: known(&p.id).and_then(|s| s.trouble.clone()),
    }));
    // AND THE TEMPORARY SEARCH, last, as it stood: the book does not hold it.
    out.extend(known(SEARCHED).cloned());
    out
}

/// Seconds since the epoch.
fn now_s(at: SystemTime) -> i64 {
    at.duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// A source's refusal, as the trouble a door says.
const fn trouble_of(why: SourceRefused) -> Trouble {
    Trouble::Source { why }
}

impl Engine {
    /// An ingest of `source` asked (`Ask::Ingest`) — a door showed it stale:
    /// read again on the source's own thread. `category` `search:<words>` on
    /// the archive is a temporary search; any other category reads the whole
    /// source (a replace is whole). Asked while one is under way, it is that
    /// one — a search for OTHER words is read after it.
    pub(super) fn ingest(
        &mut self,
        source: &SourceId,
        category: Option<&str>,
    ) -> Result<(), Refused> {
        let search = category
            .and_then(|c| c.strip_prefix(SEARCH_PREFIX))
            .map(str::trim)
            .filter(|w| !w.is_empty());
        let (what, posters, key) = match search {
            Some(words) if source.0 == "archive" => {
                let posters = self.catalog.what_of("archive").is_some_and(|(_, on)| on);
                (What::Search(words.to_owned()), posters, SEARCHED.to_owned())
            }
            Some(_) => return Err(Trouble::NoSuchSource { id: source.clone() }.into()),
            None => {
                let (what, posters) = self
                    .catalog
                    .what_of(&source.0)
                    .ok_or_else(|| Trouble::NoSuchSource { id: source.clone() })?;
                (what, posters, source.0.clone())
            }
        };
        // A DEFAULT SOURCE THE PERSON DID NOT LET is refused AT ONCE, before
        // anything is read: its hosts are its consent row.
        if let What::Default(d) = &what
            && !d.hosts.iter().all(|h| ronoctua_consent::allows(h))
        {
            return Err(trouble_of(SourceRefused::NotLet).into());
        }
        if let What::Search(words) = &what {
            // THE SEARCH HAS A STATUS OF ITS OWN: under way, and then why it
            // gave nothing — never an empty page that looks like no match.
            if !self.catalog.ask_search(words) {
                return Ok(());
            }
        } else {
            if let Some(s) = self.catalog.status_mut(&key)
                && s.busy
            {
                return Ok(());
            }
            if let Some(s) = self.catalog.status_mut(&key) {
                s.busy = true;
                s.trouble = None;
            }
        }
        let job = Job {
            what,
            posters,
            fetched: None,
        };
        self.send_ingest(&key, job)?;
        self.dirty = true;
        Ok(())
    }

    /// `job` sent to `key`'s thread — started the first time, and again once
    /// when it died.
    fn send_ingest(&mut self, key: &str, job: Job) -> Result<(), Refused> {
        let shared = self.shared()?;
        let stores = self.catalog.stores.clone();
        let stop = self.catalog.stop.clone();
        let worker = self
            .catalog
            .workers
            .entry(key.to_owned())
            .or_insert_with(|| spawn_ingest(key, shared.clone(), stores.clone(), stop.clone()));
        if let Err(mpsc::SendError(job)) = worker.tx.send(job) {
            let worker = spawn_ingest(key, shared, stores, stop);
            let _ = worker.tx.send(job);
            self.catalog.workers.insert(key.to_owned(), worker);
        }
        Ok(())
    }

    /// An ingest of `id` ended — of a search for `searched`, when it was one.
    fn ingest_ended(&mut self, id: &str, searched: Option<&str>, outcome: Outcome) {
        // A SEARCH ANOTHER ONE REPLACED wrote rows nobody shows; the status
        // is the newest one's, still under way.
        if !self.catalog.is_current(searched) {
            self.catalog.generation += 1;
            self.dirty = true;
            return;
        }
        let written = matches!(outcome, Outcome::Written { .. });
        if let Some(s) = self.catalog.status_mut(id) {
            s.busy = false;
            match &outcome {
                Outcome::Written { items, title } => {
                    s.items = *items;
                    s.trouble = None;
                    if title.is_some() {
                        s.title.clone_from(title);
                    }
                }
                Outcome::Refused(why) => s.trouble = Some(trouble_of(*why)),
                Outcome::Catalog(why) => s.trouble = Some(Trouble::Catalog { why: *why }),
                // NEVER HERE: a withdrawn ingest's thread ends without a word
                // (`spawn_ingest`), and its source has no status any more.
                Outcome::Withdrawn => {}
            }
        }
        // WHAT A PERSON'S SOURCE CALLS ITSELF is kept with it, the first
        // time it says: the book is the one place its row is named from.
        if let (true, Outcome::Written { title: Some(t), .. }) = (written, &outcome)
            && let Some(p) = self.catalog.kept.sources.iter_mut().find(|p| p.id == id)
            && p.title != *t
        {
            p.title.clone_from(t);
            self.keep_the_book();
        }
        // EVERY ENDING moves the generation: a refusal is written too
        // (`Writer::failed`), and a door reads why.
        self.catalog.generation += 1;
        self.dirty = true;
    }

    /// The book written as the catalogue holds it; a refusal is the state's.
    fn keep_the_book(&mut self) -> Option<SourcesTrouble> {
        let Some(book) = self.catalog.book.as_ref() else {
            return Some(SourcesTrouble::NoHome);
        };
        match book.write(&self.catalog.kept) {
            Ok(()) => None,
            Err(sources::Unkept::NoHome) => Some(SourcesTrouble::NoHome),
            Err(sources::Unkept::Unwritable) => Some(SourcesTrouble::Unwritable),
        }
    }

    /// The posters of `items` asked (`Ask::Posters`): fetched on the posters'
    /// thread, the ones already kept skipped there.
    pub(super) fn posters(&mut self, items: &[ItemId]) -> Result<(), Refused> {
        let rules = self.catalog.poster_rules();
        let asked: Vec<Poster> = items
            .iter()
            .take(POSTERS_ASKED_MOST)
            .map(|i| Poster {
                item: catalog::ItemId(i.0),
                rules: rules.clone(),
            })
            .collect();
        if asked.is_empty() {
            return Ok(());
        }
        let shared = self.shared()?;
        let dir = self.catalog.stores.dir.clone();
        let stop = self.catalog.stop.clone();
        let worker = self
            .catalog
            .posters
            .get_or_insert_with(|| spawn_posters(shared.clone(), dir.clone(), stop.clone()));
        if let Err(mpsc::SendError(asked)) = worker.send(asked) {
            let worker = spawn_posters(shared, dir, stop);
            let _ = worker.send(asked);
            self.catalog.posters = Some(worker);
        }
        Ok(())
    }

    /// Posters arrived: every door that shows them draws them.
    fn posters_arrived(&mut self) {
        self.catalog.posters_generation += 1;
        self.dirty = true;
    }
}

/// The host of `url` when it is asked in the clear — a person's `http`
/// list's — and `None` for an `https` one, whose posters are never asked in
/// the clear. Pure.
fn plain_host(url: &str) -> Option<String> {
    if sources::is_plain(url) {
        ask::host_of(url)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// THE ASKS THAT WRITE THE BOOK — on the asking connection's thread: a pasted
// link is fetched to be recognised, and the engine's lock is not held across
// it.
// ---------------------------------------------------------------------------

/// A source pasted (`Ask::AddSource`): recognised by its address or its first
/// bytes, kept in the book (`0600`), and ingested.
pub(super) fn add_source(url: &SourceUrl, adult: bool, shared: &Shared) -> Say {
    let address = Address::new(&url.0);
    let recognised = recognise(&Internet, &address);
    let (sniffed, fetched) = match recognised {
        Ok(found) => found,
        Err(why) => {
            tracing::info!(why = why.id(), "a pasted source was not taken");
            return Say::Refused(trouble_of(why));
        }
    };
    let now = now_s(SystemTime::now());
    let Ok(mut e) = shared.0.lock() else {
        return Say::Refused(Trouble::NoEngine {
            why: "the engine's lock was poisoned".to_owned(),
        });
    };
    if let Some(why) = e.catalog.book_trouble {
        return Say::Refused(Trouble::Sources { why });
    }
    // A COLLECTION OF THE ARCHIVE is asked of archive.org, whose consent the
    // pasted link is: the person named the host by pasting it — the rule the
    // channels of the video catalogue follow.
    if matches!(sniffed, Sniffed::Collection { .. })
        && let Err(why) = ronoctua_consent::let_hosts(&[archive::HOST], true)
    {
        tracing::info!(?why, "the archive was not let for a pasted collection");
        return Say::Refused(trouble_of(SourceRefused::NotLet));
    }
    let id = Book::next_id(&e.catalog.kept.sources, now);
    let person = PersonSource {
        id: id.clone(),
        kind: sniffed.kind(),
        address,
        title: String::new(),
        adult,
        // POSTERS ON only where they come from the source's own host: a
        // collection's are the archive's.
        posters: matches!(sniffed, Sniffed::Collection { .. }),
        added_s: now,
    };
    e.catalog.kept.sources.push(person.clone());
    if let Some(why) = e.keep_the_book() {
        e.catalog.kept.sources.retain(|s| s.id != id);
        return Say::Refused(Trouble::Sources { why });
    }
    e.catalog.restate();
    if let Some(s) = e.catalog.status_mut(&id) {
        s.busy = true;
    }
    let posters = person.posters;
    let job = Job {
        what: What::Person(person),
        posters,
        fetched,
    };
    let sent = e.send_ingest(&id, job);
    e.dirty = true;
    e.broadcast();
    drop(e);
    shared.1.notify_all();
    match sent {
        Ok(()) => Say::Done,
        Err(Refused(t)) => Say::Refused(t),
    }
}

/// What a pasted link is, and the bytes it answered when it was fetched to
/// know it. Blocks: the connection's thread.
fn recognise(
    web: &dyn Web,
    address: &Address,
) -> Result<(Sniffed, Option<Vec<u8>>), SourceRefused> {
    if let Some(known) = Sniffed::by_address(address.expose()) {
        // AN ADDRESS THAT IS NO ADDRESS never reaches the network either.
        let sniffed = known?;
        sources::sniff(address.expose(), &[])?;
        return Ok((sniffed, None));
    }
    // THE SHAPE FIRST: a bad address is refused before anything is sent.
    if let Err(SourceRefused::NotAnAddress) = sources::sniff(address.expose(), &[]) {
        return Err(SourceRefused::NotAnAddress);
    }
    let bytes = ask::fetched_own(web, address, table::LIST_MIB * MIB)?;
    let head = &bytes[..bytes.len().min(4096)];
    let sniffed = sources::sniff(address.expose(), head)?;
    Ok((sniffed, Some(bytes)))
}

/// A source forgotten (`Ask::RemoveSource`): out of the book and out of both
/// files. A default source is never removed.
pub(super) fn remove_source(id: &SourceId, shared: &Shared) -> Say {
    let Ok(mut e) = shared.0.lock() else {
        return Say::Refused(Trouble::NoEngine {
            why: "the engine's lock was poisoned".to_owned(),
        });
    };
    let Some(at) = e.catalog.kept.sources.iter().position(|s| s.id == id.0) else {
        return Say::Refused(Trouble::NoSuchSource { id: id.clone() });
    };
    let gone = e.catalog.kept.sources.remove(at);
    if let Some(why) = e.keep_the_book() {
        e.catalog.kept.sources.insert(at, gone);
        return Say::Refused(Trouble::Sources { why });
    }
    e.catalog.restate();
    // AN INGEST OF IT UNDER WAY writes nothing now — the flag is up before
    // the removal below takes the writer (`Catalog::withdraw`).
    e.catalog.withdraw(&id.0);
    let stores = e.catalog.stores.clone();
    e.catalog.generation += 1;
    e.dirty = true;
    e.broadcast();
    drop(e);
    shared.1.notify_all();
    // THE ROWS, off the engine's lock: the writer may be finishing an
    // ingest of another source — or of this one, written before the flag
    // went up, which this deletes.
    match stores.with_writer(|w| w.remove(&id.0)) {
        Ok(()) | Err(CatalogTrouble::Absent) => {
            with_engine(shared, |e| {
                e.catalog.generation += 1;
                e.dirty = true;
            });
            Say::Done
        }
        Err(why) => Say::Refused(Trouble::Catalog { why }),
    }
}

/// A source's switches turned (`Ask::SetSource`).
pub(super) fn set_source(
    id: &SourceId,
    posters: Option<bool>,
    adult: Option<bool>,
    shared: &Shared,
) -> Say {
    let Ok(mut e) = shared.0.lock() else {
        return Say::Refused(Trouble::NoEngine {
            why: "the engine's lock was poisoned".to_owned(),
        });
    };
    let default = DefaultSource::of(&id.0);
    let person = e.catalog.kept.sources.iter().position(|s| s.id == id.0);
    if default.is_none() && person.is_none() {
        return Say::Refused(Trouble::NoSuchSource { id: id.clone() });
    }
    // A DEFAULT SOURCE IS NEVER ADULT: nothing adult is declared, and its
    // rows are the table's.
    if default.is_some() && adult == Some(true) {
        return Say::Refused(Trouble::NoSuchSource { id: id.clone() });
    }
    let before = e.catalog.kept.clone();
    let mut again = false;
    if let Some(d) = default
        && let Some(on) = posters
    {
        // THE SWITCH NAMES ITS HOSTS: on, it lets them (the person pressed a
        // row that names them); off, it only stops asking — the hosts may be
        // another source's too.
        if on
            && !d.poster_hosts.is_empty()
            && let Err(why) = ronoctua_consent::let_hosts(d.poster_hosts, true)
        {
            tracing::info!(?why, "a source's poster hosts were not let");
            return Say::Refused(trouble_of(SourceRefused::NotLet));
        }
        if on == d.posters_on {
            e.catalog.kept.default_posters.remove(d.id);
        } else {
            e.catalog.kept.default_posters.insert(d.id.to_owned(), on);
        }
    }
    if let Some(at) = person {
        let p = &mut e.catalog.kept.sources[at];
        if let Some(on) = posters {
            p.posters = on;
        }
        if let Some(on) = adult
            && p.adult != on
        {
            p.adult = on;
            again = true;
        }
    }
    if let Some(why) = e.keep_the_book() {
        e.catalog.kept = before;
        return Say::Refused(Trouble::Sources { why });
    }
    e.catalog.restate();
    let stores = e.catalog.stores.clone();
    // ITS ROWS MOVE between the two files with a new ingest: the replace
    // writes the adult file or the main one by the source's switch.
    if again && let Some((what, on)) = e.catalog.what_of(&id.0) {
        if let Some(s) = e.catalog.status_mut(&id.0) {
            s.busy = true;
        }
        let job = Job {
            what,
            posters: on,
            fetched: None,
        };
        if let Err(Refused(t)) = e.send_ingest(&id.0, job) {
            return Say::Refused(t);
        }
    }
    e.dirty = true;
    e.broadcast();
    drop(e);
    shared.1.notify_all();
    if let Some(on) = posters {
        match stores.with_writer(|w| w.set_posters(&id.0, on)) {
            Ok(()) | Err(CatalogTrouble::Absent) => {}
            Err(why) => return Say::Refused(Trouble::Catalog { why }),
        }
        with_engine(shared, |e| {
            e.catalog.generation += 1;
            e.dirty = true;
        });
    }
    Say::Done
}

// ---------------------------------------------------------------------------
// THE INGESTS
// ---------------------------------------------------------------------------

/// A source's thread: its ingests one at a time — the newest asked when
/// several wait, as it reads the book as it is now — and nothing more once
/// the source is withdrawn.
fn spawn_ingest(key: &str, shared: Shared, stores: Arc<Stores>, stop: Arc<AtomicBool>) -> Worker {
    let (tx, rx) = mpsc::channel::<Job>();
    let withdrawn = Arc::new(AtomicBool::new(false));
    let flag = withdrawn.clone();
    let key = key.to_owned();
    let born = std::thread::Builder::new()
        .name("player-ingest".into())
        .spawn(move || {
            while let Ok(mut job) = rx.recv() {
                while let Ok(newer) = rx.try_recv() {
                    job = newer;
                }
                if stop.load(Ordering::SeqCst) || flag.load(Ordering::SeqCst) {
                    return;
                }
                let tools = Tools::machine();
                let outcome = ingest(&job, &stores, &tools, &flag);
                // REMOVED WHILE IT WAS READ: nothing was written, and there
                // is no status to say anything in.
                if outcome == Outcome::Withdrawn {
                    return;
                }
                if let Outcome::Refused(why) = &outcome {
                    // THE KIND ONLY: a person's source's words name its
                    // address.
                    tracing::info!(
                        source = key,
                        why = why.id(),
                        "a catalogue source gave nothing"
                    );
                }
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let searched = job.searched();
                with_engine(&shared, |e| {
                    e.ingest_ended(&key, searched.as_deref(), outcome);
                });
            }
        });
    if let Err(e) = born {
        tracing::warn!(error = %e, "could not start a catalogue source's thread");
    }
    Worker { tx, withdrawn }
}

/// The person's consent for a host, as a seam.
type PassOf<'a> = dyn Fn(&str) -> Result<ronoctua_consent::Pass, SourceRefused> + 'a;

/// yt-dlp's search of what is live, as a seam.
type SearchLives<'a> =
    dyn Fn(&str, usize) -> Result<Vec<ronoctua_ytdlp::Live>, ronoctua_ytdlp::Trouble> + 'a;

/// yt-dlp's listing of a YouTube channel or playlist, as a seam.
type ListYouTube<'a> = dyn Fn(YouTubeList, &str) -> Result<(String, Harvest), SourceRefused> + 'a;

/// What an ingest works with — the network, the consent, yt-dlp and the
/// player's cache — each a seam a test hands its own.
struct Tools<'a> {
    web: &'a dyn Web,
    net: &'a dyn Net,
    /// The person's consent for a host.
    pass: &'a PassOf<'a>,
    /// yt-dlp's search of what is live.
    lives: &'a SearchLives<'a>,
    /// yt-dlp's listing of a YouTube channel or playlist.
    listed: &'a ListYouTube<'a>,
    /// The cache the TV desk keeps iptv-org's index in.
    cache: Option<Cache>,
    /// Waited between two requests to one host.
    pace: Duration,
    now: SystemTime,
}

impl Tools<'static> {
    /// The machine's.
    fn machine() -> Self {
        Self {
            web: &Internet,
            net: &Internet,
            pass: &|host| ronoctua_consent::pass(host).map_err(|_| SourceRefused::NotLet),
            lives: &ronoctua_ytdlp::search_lives,
            listed: &sources::youtube::listed,
            cache: Cache::player(u64::from(policy::PLAYER_MIB) * MIB).ok(),
            pace: PACE,
            now: SystemTime::now(),
        }
    }
}

/// One ingest, whole: read, then written — unless `withdrawn` is up by the
/// time the writer is in hand: the source was removed meanwhile, and what was
/// read is dropped. Looked at WITH THE WRITER HELD, the flag a removal raises
/// before it takes the writer itself: this writes before the removal deletes,
/// or not at all. Blocks.
fn ingest(job: &Job, stores: &Stores, tools: &Tools<'_>, withdrawn: &AtomicBool) -> Outcome {
    let now = now_s(tools.now);
    let source = new_source(job);
    let gone = || withdrawn.load(Ordering::SeqCst);
    match harvest(job, tools) {
        Ok((title, h)) => {
            let source = NewSource {
                title: title.clone().unwrap_or_else(|| source.title.clone()),
                ..source
            };
            let written = stores.with_writer(|w| {
                if gone() {
                    return Ok(None);
                }
                w.replace(&source, &h, now).map(Some)
            });
            match written {
                Ok(Some(done)) => Outcome::Written {
                    items: done.main.saturating_add(done.adult),
                    title,
                },
                Ok(None) => Outcome::Withdrawn,
                Err(why) => Outcome::Catalog(why),
            }
        }
        Err(why) => {
            let written = stores.with_writer(|w| {
                if gone() {
                    return Ok(false);
                }
                w.failed(&source, why.id()).map(|()| true)
            });
            match written {
                Ok(true) => Outcome::Refused(why),
                Ok(false) => Outcome::Withdrawn,
                Err(t) => Outcome::Catalog(t),
            }
        }
    }
}

/// The source's row, as the writer records it.
fn new_source(job: &Job) -> NewSource {
    match &job.what {
        What::Default(d) => NewSource {
            id: d.id.to_owned(),
            kind: d.kind,
            title: String::new(),
            shown: d.hosts.first().copied().unwrap_or_default().to_owned(),
            adult: false,
            posters_on: job.posters,
        },
        What::Person(p) => NewSource {
            id: p.id.clone(),
            kind: p.kind,
            title: p.title.clone(),
            shown: p.address.shown(),
            adult: p.adult,
            posters_on: job.posters,
        },
        What::Search(words) => NewSource {
            id: SEARCHED.to_owned(),
            kind: Kind::Archive,
            title: words.clone(),
            shown: archive::HOST.to_owned(),
            adult: false,
            posters_on: job.posters,
        },
    }
}

/// What a source gave: its own name, when it says one, and its harvest.
///
/// # Errors
///
/// The source's refusal: nothing of it answered.
fn harvest(job: &Job, tools: &Tools<'_>) -> Result<(Option<String>, Harvest), SourceRefused> {
    match &job.what {
        What::Default(d) => default_harvest(d, tools).map(|h| (None, h)),
        What::Person(p) => person_harvest(p, job.fetched.as_deref(), tools),
        What::Search(words) => search_harvest(words, tools).map(|h| (None, h)),
    }
}

/// The pass for every host of `d`, the first one's returned: refused when any
/// is not let.
fn passes(d: &DefaultSource, tools: &Tools<'_>) -> Result<ronoctua_consent::Pass, SourceRefused> {
    let mut first = None;
    for host in d.hosts {
        let pass = (tools.pass)(host)?;
        first.get_or_insert(pass);
    }
    first.ok_or(SourceRefused::NotLet)
}

/// What each of `d`'s natives gave, in turn — `ask` asked of each until it
/// says it is done — the ones that failed left out and the harvest said to
/// be cut; nothing at all is the first refusal.
fn natives_harvest(
    d: &DefaultSource,
    tools: &Tools<'_>,
    mut ask: impl FnMut(&table::Native, &mut Harvest) -> Result<(), SourceRefused>,
) -> Result<Harvest, SourceRefused> {
    let mut h = Harvest::default();
    let mut first_refusal = None;
    for (n, native) in d.natives.iter().enumerate() {
        if n > 0 {
            std::thread::sleep(tools.pace);
        }
        if let Err(why) = ask(native, &mut h) {
            h.cut = true;
            first_refusal.get_or_insert(why);
            // A HOST THAT ASKED TO BE ASKED LESS is not asked again this
            // time, nor one not let.
            if matches!(why, SourceRefused::TooManyAsks | SourceRefused::NotLet) {
                break;
            }
        }
    }
    if h.items.is_empty()
        && let Some(why) = first_refusal
    {
        return Err(why);
    }
    if h.items.is_empty() {
        return Err(SourceRefused::NothingFound);
    }
    Ok(h)
}

/// Pages of one native asked with `page` until `done` says so or the table's
/// most is reached; a page that fails after the first leaves the rest cut.
fn paged(
    tools: &Tools<'_>,
    h: &mut Harvest,
    mut page: impl FnMut(u32, &mut Harvest) -> Result<bool, SourceRefused>,
) -> Result<(), SourceRefused> {
    let before = h.items.len();
    for number in 1..=table::NATIVE_ITEMS_MOST
        .div_ceil(table::PAGE_ROWS as usize)
        .max(1)
    {
        let number = u32::try_from(number).unwrap_or(u32::MAX);
        if number > 1 {
            std::thread::sleep(tools.pace);
        }
        match page(number, h) {
            Ok(true) => {}
            Ok(false) => return Ok(()),
            Err(why) if h.items.len() == before => return Err(why),
            Err(_) => {
                h.cut = true;
                return Ok(());
            }
        }
        if h.items.len() - before >= table::NATIVE_ITEMS_MOST {
            h.cut = true;
            return Ok(());
        }
    }
    Ok(())
}

/// A default source, whole.
fn default_harvest(d: &'static DefaultSource, tools: &Tools<'_>) -> Result<Harvest, SourceRefused> {
    match d.kind {
        Kind::IptvOrg => {
            // ITS CONSENT: the directory's host, the same as the world TV's.
            passes(d, tools)?;
            let index = tv_index(tools)?;
            Ok(defaults::iptv_org(&index, d))
        }
        Kind::Archive => {
            let pass = passes(d, tools)?;
            natives_harvest(d, tools, |n, h| {
                paged(tools, h, |page, h| {
                    let c = archive::Collection {
                        identifier: n.ask,
                        media: if n.audio {
                            archive::Media::Audio
                        } else {
                            archive::Media::Movies
                        },
                        subjects: n.narrow,
                        page,
                        rows: table::PAGE_ROWS,
                    };
                    let found = archive::collection(tools.web, &pass, &c)?;
                    defaults::archive_native(h, n, &found);
                    Ok(found.more)
                })
            })
        }
        Kind::Lives => {
            passes(d, tools)?;
            natives_harvest(d, tools, |n, h| {
                let found = (tools.lives)(n.ask, table::LIVES_MOST).map_err(SourceRefused::from)?;
                defaults::lives(h, n, &found);
                Ok(())
            })
        }
        Kind::Nasa => {
            let pass = (tools.pass)(nasa::HOST)?;
            passes(d, tools)?;
            natives_harvest(d, tools, |n, h| {
                paged(tools, h, |page, h| {
                    let found = nasa::search(tools.web, &pass, n.ask, page)?;
                    nasa::harvest(h, n, &found);
                    Ok(found.more && !found.videos.is_empty())
                })
            })
        }
        Kind::Commons => {
            let pass = (tools.pass)(commons::HOST)?;
            passes(d, tools)?;
            natives_harvest(d, tools, |n, h| {
                let mut next: Option<String> = None;
                paged(tools, h, |_, h| {
                    let found = commons::members(tools.web, &pass, n.ask, next.as_deref())?;
                    commons::harvest(h, n, &found);
                    next = found.next;
                    Ok(next.is_some())
                })
            })
        }
        Kind::PeerTube => {
            let pass = (tools.pass)(peertube::HOST)?;
            passes(d, tools)?;
            natives_harvest(d, tools, |n, h| {
                let category: u16 = n.ask.parse().map_err(|_| SourceRefused::Unknown)?;
                paged(tools, h, |page, h| {
                    let start = (page - 1).saturating_mul(peertube::PAGE_MOST);
                    let found = peertube::search(tools.web, &pass, category, start)?;
                    peertube::harvest(h, n, &found);
                    let seen = u64::from(page).saturating_mul(u64::from(peertube::PAGE_MOST));
                    Ok(!found.videos.is_empty() && seen < found.total)
                })
            })
        }
        Kind::Playlist | Kind::Feed | Kind::YouTube | Kind::Collection => {
            Err(SourceRefused::Unknown)
        }
    }
}

/// iptv-org's index: the TV desk's, from the player's cache while it is
/// fresh; else asked, and kept for both.
fn tv_index(tools: &Tools<'_>) -> Result<Index, SourceRefused> {
    const NAME: &str = "iptv-index.json";
    let fresh_for = Duration::from_secs(table::FRESH_IPTV_MINUTES.saturating_mul(60));
    if let Some(cache) = tools.cache.as_ref()
        && let Ok(Some(kept)) = cache.read(NAME, fresh_for, tools.now)
        && kept.fresh
        && let Ok(index) = Index::from_bytes(&kept.bytes)
    {
        return Ok(index);
    }
    let index = iptv::refresh(tools.net, table::EXCLUDED, now_s(tools.now))
        .map_err(|r| SourceRefused::from(r.why))?;
    if let Some(cache) = tools.cache.as_ref()
        && let Err(why) = cache.write(NAME, &index.to_bytes(), None)
    {
        tracing::info!(?why, "the TV index was not kept");
    }
    Ok(index)
}

/// One of the person's sources, whole — `fetched` the bytes it answered when
/// it was added.
fn person_harvest(
    p: &PersonSource,
    fetched: Option<&[u8]>,
    tools: &Tools<'_>,
) -> Result<(Option<String>, Harvest), SourceRefused> {
    let url = p.address.expose();
    let named = |title: String| (!title.trim().is_empty()).then_some(title);
    match p.kind {
        Kind::Playlist => {
            let most = usize::try_from(table::LIST_MIB * MIB).unwrap_or(usize::MAX);
            let bytes = match fetched {
                Some(b) => b.to_vec(),
                None => ask::fetched_own(tools.web, &p.address, table::LIST_MIB * MIB)?,
            };
            let text = String::from_utf8_lossy(&bytes);
            let h = sources::playlist(&text, url, most, table::SOURCE_ITEMS_MOST)?;
            Ok((None, h))
        }
        Kind::Feed => {
            let bytes = match fetched {
                Some(b) => b.to_vec(),
                None => ask::fetched_own(tools.web, &p.address, table::FEED_MIB * MIB)?,
            };
            let fed = sources::feed(&String::from_utf8_lossy(&bytes), table::SOURCE_ITEMS_MOST)?;
            Ok((named(fed.title), fed.harvest))
        }
        Kind::YouTube => {
            let Some(Ok(Sniffed::YouTube { list })) = Sniffed::by_address(url) else {
                return Err(SourceRefused::Unknown);
            };
            let (title, h) = (tools.listed)(list, url)?;
            Ok((named(title), h))
        }
        Kind::Collection => {
            let Some(Ok(Sniffed::Collection { identifier })) = Sniffed::by_address(url) else {
                return Err(SourceRefused::Unknown);
            };
            let pass = (tools.pass)(archive::HOST)?;
            let mut h = Harvest::default();
            let theme = sources::theme_of(&identifier);
            let adult = sources::is_adult(&identifier);
            let most = table::SOURCE_ITEMS_MOST;
            let rows = archive::COLLECTION_ROWS_CEILING;
            let mut page = 1;
            loop {
                let c = archive::Collection {
                    identifier: &identifier,
                    media: archive::Media::Either,
                    subjects: &[],
                    page,
                    rows,
                };
                match archive::collection(tools.web, &pass, &c) {
                    Ok(found) => {
                        defaults::holdings(&mut h, &identifier, &identifier, theme, adult, &found);
                        if !found.more || h.items.len() >= most {
                            h.cut |= found.more;
                            break;
                        }
                    }
                    Err(why) if h.items.is_empty() => return Err(why),
                    Err(_) => {
                        h.cut = true;
                        break;
                    }
                }
                page += 1;
                std::thread::sleep(tools.pace);
            }
            if h.items.is_empty() {
                return Err(SourceRefused::NothingPlayable);
            }
            Ok((None, h))
        }
        Kind::IptvOrg
        | Kind::Archive
        | Kind::Lives
        | Kind::Nasa
        | Kind::Commons
        | Kind::PeerTube => Err(SourceRefused::Unknown),
    }
}

/// A search of the archive's collections — the default source's, every one —
/// for `words`, as the one category `search:<words>`.
fn search_harvest(words: &str, tools: &Tools<'_>) -> Result<Harvest, SourceRefused> {
    let archive_source = DefaultSource::of("archive").ok_or(SourceRefused::Unknown)?;
    passes(archive_source, tools)?;
    let mut collections: Vec<&str> = archive_source.natives.iter().map(|n| n.ask).collect();
    collections.sort_unstable();
    collections.dedup();
    let key = format!("{SEARCH_PREFIX}{words}");
    let mut h = Harvest::default();
    h.category(&key, words, None, false);
    for page in 1.. {
        let search = archive::Search {
            collections: &collections,
            subjects: &[],
            words: Some(words),
            page,
            rows: table::PAGE_ROWS,
        };
        let found = match archive::search(tools.net, &search) {
            Ok(f) => f,
            Err(r) if h.items.is_empty() => return Err(SourceRefused::from(r.why)),
            Err(_) => {
                h.cut = true;
                break;
            }
        };
        for item in &found.items {
            let Some(stream) = item.stream() else {
                continue;
            };
            h.items.push(catalog::NewItem {
                ext_id: item.identifier.clone(),
                kind: catalog::ItemKind::Video,
                title: item.title.clone(),
                also: Vec::new(),
                description: None,
                country: None,
                language: None,
                height: None,
                duration_s: None,
                year: item.year,
                licence: item.license_url.clone(),
                popularity: item.downloads,
                published_s: None,
                poster_url: archive::poster_address(&item.identifier),
                stream,
                categories: vec![key.clone()],
            });
        }
        if !found.more || h.items.len() >= SEARCH_MOST {
            h.cut |= found.more;
            break;
        }
        std::thread::sleep(tools.pace);
    }
    Ok(h)
}

// ---------------------------------------------------------------------------
// THE POSTERS
// ---------------------------------------------------------------------------

/// Whose posters are fetched, and from where.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Rule {
    /// From these hosts only, each on the person's consent: a default source
    /// whose pictures come from hosts it names.
    Hosts(&'static [&'static str]),
    /// From any `https` site the list says: a person's source, or a default
    /// one whose pictures come from any site — the switch is the consent.
    /// `clear_from`: the one host an `http` poster is asked of too, in the
    /// clear — a person's `http` list's own, the host they pasted and were
    /// told is not encrypted. Never another: a list names any address it
    /// likes, its neighbours on the person's network among them.
    Anywhere {
        /// The host asked in the clear, when one is.
        clear_from: Option<String>,
    },
}

/// One poster asked, and the sources' rules as they were when it was.
#[derive(Debug, Clone)]
struct Poster {
    item: catalog::ItemId,
    rules: Arc<BTreeMap<String, Rule>>,
}

/// The posters' thread: one poster at a time, never faster than
/// [`POSTER_PACE`] to one host.
fn spawn_posters(
    shared: Shared,
    dir: Option<PathBuf>,
    stop: Arc<AtomicBool>,
) -> mpsc::Sender<Vec<Poster>> {
    let (tx, rx) = mpsc::channel::<Vec<Poster>>();
    let born = std::thread::Builder::new()
        .name("player-posters".into())
        .spawn(move || {
            let Some(dir) = dir else {
                return;
            };
            let cache = Cache::at(
                dir.join(catalog::POSTERS),
                u64::from(policy::PLAYER_POSTERS_MIB) * MIB,
            );
            let mut last: HashMap<String, Instant> = HashMap::new();
            let mut readers: Vec<catalog::Reader> = Vec::new();
            while let Ok(mut asked) = rx.recv() {
                // THE NEWEST ASK WINS: a door that scrolled on names other
                // items, and the ones it scrolled past wait for no one.
                while let Ok(newer) = rx.try_recv() {
                    asked = newer;
                }
                let mut arrived = 0usize;
                for poster in asked {
                    if stop.load(Ordering::SeqCst) {
                        return;
                    }
                    if readers.is_empty() || readers.iter().any(catalog::Reader::replaced) {
                        readers = Store::ALL
                            .iter()
                            .filter_map(|s| catalog::Reader::open(&dir, *s).ok())
                            .collect();
                    }
                    if fetched_poster(&poster, &readers, &cache, &mut last) {
                        arrived += 1;
                    }
                }
                if arrived > 0 {
                    with_engine(&shared, Engine::posters_arrived);
                }
            }
        });
    if let Err(e) = born {
        tracing::warn!(error = %e, "could not start the posters' thread");
    }
    tx
}

/// One poster, fetched into the cache when its source's rule lets it and it
/// is not there yet; whether one arrived.
fn fetched_poster(
    poster: &Poster,
    readers: &[catalog::Reader],
    cache: &Cache,
    last: &mut HashMap<String, Instant>,
) -> bool {
    let Some(item) = readers
        .iter()
        .find_map(|r| r.item(poster.item).ok().flatten())
    else {
        return false;
    };
    let Some(url) = item.row.poster_url.as_deref() else {
        return false;
    };
    let Some(rule) = poster.rules.get(&item.row.source) else {
        return false;
    };
    let name = catalog::poster_file(url);
    if cache.dir().join(&name).exists() {
        return false;
    }
    let Some(host) = ask::host_of(url) else {
        return false;
    };
    if !poster_allowed(rule, url, &host) {
        return false;
    }
    // THE PACE, per host.
    if let Some(at) = last.get(&host) {
        let waited = at.elapsed();
        if waited < POSTER_PACE {
            std::thread::sleep(POSTER_PACE - waited);
        }
    }
    last.insert(host.clone(), Instant::now());
    let got = match rule {
        Rule::Hosts(_) => {
            let Ok(pass) = ronoctua_consent::pass(&host) else {
                return false;
            };
            let fetch = Fetch {
                url: url.to_owned(),
                accept: "image/*",
                gzip: false,
                most_bytes: POSTER_MOST_BYTES,
            };
            ask::fetched(&Internet, &pass, &fetch)
        }
        Rule::Anywhere { .. } => ask::fetched_own(&Internet, &Address::new(url), POSTER_MOST_BYTES),
    };
    match got {
        Ok(bytes) if looks_like_a_picture(&bytes) => match cache.write(&name, &bytes, None) {
            Ok(()) => true,
            Err(why) => {
                tracing::info!(?why, "a poster was not kept");
                false
            }
        },
        Ok(_) => false,
        Err(why) => {
            tracing::debug!(why = why.id(), "a poster did not come");
            false
        }
    }
}

/// Whether the poster at `url`, on `host`, may be asked under `rule`: a
/// source's named hosts (and their subdomains) only; and in the clear only
/// from the host of the person's own `http` list. Pure.
fn poster_allowed(rule: &Rule, url: &str, host: &str) -> bool {
    match rule {
        Rule::Hosts(hosts) => hosts
            .iter()
            .any(|h| host == *h || host.ends_with(&format!(".{h}"))),
        Rule::Anywhere { clear_from } => {
            !sources::is_plain(url) || clear_from.as_deref() == Some(host)
        }
    }
}

/// Whether `bytes` begin as a picture a door decodes: PNG, JPEG, GIF or WebP
/// — anything else (an error page answered 200) is never kept as a poster.
/// Pure.
fn looks_like_a_picture(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(&[0xff, 0xd8, 0xff])
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || (bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
}

#[cfg(test)]
mod tests;
