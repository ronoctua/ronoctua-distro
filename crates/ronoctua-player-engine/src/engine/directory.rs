//! The directories: the world's radio (radio-browser), the world's TV
//! (iptv-org), a theme's films (the Internet Archive) and what is live on
//! YouTube now — listed for every door, with what the globe draws of them.
//!
//! # Five desks, the lookups' shape
//!
//! Each of radio, television, channels (a theme's live TV), archive and lives
//! is a desk ([`DeskKind`]) with a thread of its own, started the first time
//! it is asked and asleep on its mailbox after that. A listing is numbered
//! when it is asked, and what it finds is posted only while its number is
//! still the desk's: a new one REPLACES the one under way. The state carries
//! each desk's summary; a door asks for the page when its generation moves
//! (`Ask::Desk`). A theme's page is three desks — its TV, its films, its
//! lives — so each section is asked, renewed and refused on its own, and the
//! world's TV page is not replaced by a theme's.
//!
//! # A list is shown at once, and renewed behind it
//!
//! What a directory answered is kept in the player's cache
//! (`$XDG_CACHE_HOME/ronoctua/player`, bounded by `playerMib` in
//! `assets/cache.nix`, the file read longest ago leaving first) for as long
//! as `assets/channels.nix` says it stays fresh. A kept list is posted at
//! once; one older than that is posted MARKED STALE and asked again behind
//! it, and when the directory does not answer the stale one stays, with the
//! refusal. What a person typed is never kept: a search is theirs. The TV is
//! one index (`iptv-org`'s files, gzipped), renewed once a day under a lock
//! of this process, so the two TV desks never download it twice at once.
//!
//! # The globe is placed here
//!
//! A page carries its marks ([`Globe`]): a country at the point its name goes
//! on a map (the atlas's anchor), a station that says where it is at its own
//! coordinate, the stations that do not on their country's anchor, and a
//! channel whose feed names a place the map can find there — else on its
//! country's anchor. A door groups and draws them; it never places one.
//!
//! # What a stream of a directory is owed once it starts
//!
//! When a stream's sound has STARTED (mpv's `FILE_LOADED`), and not before:
//! radio-browser's click (`/json/url/<uuid>`, which the service asks for
//! every time a station starts playing, and counts once a day), while the
//! radio is let; and its place at the front of the recently played. Once
//! each time it becomes the one playing — a relay tried after a failed one
//! is the same play — and again after a stop.
//!
//! # Consent
//!
//! Each directory is let apart, by a press on a row that names its host,
//! and born not let; the lives need yt-dlp, which only the media profile
//! installs. A listing of one not let — or of the lives without yt-dlp — is
//! refused AT ONCE, before anything is read or asked.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock, PoisonError,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ronoctua_broadcast::{
    Code, Net, Refusal, Source, Spot, Why, archive,
    cache::Cache,
    country,
    internet::Internet,
    iptv::{self, Channel, Index},
    picks::{Change, Kind, Pick, Shelf},
    radio::{self, By, CountryCount, Named, Radio, Station},
    table::{self, Category},
};
use ronoctua_player::{
    find::{
        ArchiveRow, CHANNELS_MOST, ChannelRow, CountryRow, Desk, Directory, Globe, Listed, LiveRow,
        Mark, MarkKind, NamedRow, Page, Query, RadioList, StationRow, TvList,
    },
    own::{Origin, Stream},
    prefs::{self, Preferences},
    queue::EntryId,
    tongues,
    wire::{DeskKind, DeskSummary, Say, Trouble},
};
use ronoctua_ytdlp::Live;
use serde::{Serialize, de::DeserializeOwned};

use super::{Engine, Fetch, Refused, Shared, desk::with_engine};

mod policy {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// One mebibyte.
const MIB: u64 = 1024 * 1024;

/// The most genres or languages one list carries: radio-browser has
/// thousands of tags, most of them one station's.
const NAMED_MOST: u32 = 300;

/// The desks the directories keep, in the order the state lists them.
pub(super) const KINDS: [DeskKind; 5] = [
    DeskKind::Radio,
    DeskKind::Television,
    DeskKind::Channels,
    DeskKind::Archive,
    DeskKind::Lives,
];

/// The cache every directory keeps its lists in; `None` with no home, and
/// then nothing is kept and every list is asked.
fn player_cache() -> Option<Cache> {
    Cache::player(u64::from(policy::PLAYER_MIB) * MIB).ok()
}

/// One desk, and the listing it has.
struct Slot {
    desk: Desk,
    /// The number of the listing whose findings count.
    search: u64,
    /// Its thread's mailbox, once it runs.
    worker: Option<mpsc::Sender<Job>>,
}

impl Slot {
    fn new(kind: DeskKind) -> Self {
        Self {
            desk: Desk::empty(kind),
            search: 0,
            worker: None,
        }
    }

    /// Something changed: the page's generation moves.
    fn moved(&mut self, busy: bool) {
        self.desk.generation += 1;
        self.desk.busy = busy;
    }
}

/// What a desk's thread shares with the engine without its lock.
#[derive(Clone)]
struct Context {
    /// Raised when the player closes: nothing found after it is posted.
    stop: Arc<AtomicBool>,
    /// The radio-browser server this session asks, once found.
    radio: Arc<Mutex<Option<Radio>>>,
    /// Held while the TV index is renewed.
    index: Arc<Mutex<()>>,
}

/// The desks of the directories.
pub(super) struct Directories {
    slots: Vec<Slot>,
    context: Context,
    /// The entry whose start was already counted — its click and its place
    /// among the recently played.
    started: Option<EntryId>,
}

/// A listing for a desk's thread.
struct Job {
    serial: u64,
    query: Query,
    prefs: Preferences,
}

impl Directories {
    /// Empty desks.
    pub(super) fn open() -> Self {
        Self {
            slots: KINDS.iter().map(|k| Slot::new(*k)).collect(),
            context: Context {
                stop: Arc::new(AtomicBool::new(false)),
                radio: Arc::new(Mutex::new(None)),
                index: Arc::new(Mutex::new(())),
            },
            started: None,
        }
    }

    /// The player is closing: nothing more is posted.
    pub(super) fn end(&self) {
        self.context.stop.store(true, Ordering::SeqCst);
    }

    /// Each desk's summary, for the state.
    pub(super) fn summaries(&self) -> Vec<DeskSummary> {
        self.slots
            .iter()
            .map(|s| DeskSummary {
                kind: s.desk.kind,
                generation: s.desk.generation,
                busy: s.desk.busy,
                trouble: s.desk.trouble.clone(),
            })
            .collect()
    }

    /// A desk's page, when it is one of the directories'.
    pub(super) fn page(&self, kind: DeskKind) -> Option<Desk> {
        self.slots
            .iter()
            .find(|s| s.desk.kind == kind)
            .map(|s| s.desk.clone())
    }

    fn slot(&mut self, kind: DeskKind) -> Option<&mut Slot> {
        self.slots.iter_mut().find(|s| s.desk.kind == kind)
    }

    fn is_current(&self, kind: DeskKind, serial: u64) -> bool {
        !self.context.stop.load(Ordering::SeqCst)
            && self
                .slots
                .iter()
                .any(|s| s.desk.kind == kind && s.search == serial)
    }

    /// The row of any page that is `(kind, id)`: its title, its country and
    /// what plays it.
    fn row(&self, kind: Kind, id: &str) -> Option<(String, Option<Code>, Stream)> {
        self.slots.iter().find_map(|s| s.desk.page.pick(kind, id))
    }

    /// The server a click goes to: the one found this session, else the
    /// entry — never a discovery made for a click, and never a wait on the
    /// radio's thread while it looks for one.
    fn server(&self) -> Radio {
        self.context
            .radio
            .try_lock()
            .ok()
            .and_then(|r| r.clone())
            .unwrap_or_else(Radio::entry)
    }

    /// A stream began to play (`FILE_LOADED`): what it is owed, once each
    /// time it becomes the one playing — radio-browser's click while the
    /// radio is let, and its place among the recently played.
    pub(super) fn started(
        &mut self,
        current: EntryId,
        stream: &Stream,
        prefs: &Preferences,
        now_unix_seconds: i64,
    ) -> Vec<Fetch> {
        if self.started == Some(current) {
            return Vec::new();
        }
        self.started = Some(current);
        let mut owed = Vec::new();
        if let Origin::RadioBrowser { uuid } = &stream.origin
            && prefs.allows_directory(Directory::Radio)
        {
            owed.push(Fetch::Click {
                server: self.server(),
                uuid: uuid.clone(),
            });
        }
        if let Some(pick) = self.recent_of(stream, now_unix_seconds) {
            owed.push(Fetch::Recent {
                pick: Box::new(pick),
            });
        }
        owed
    }

    /// What plays stopped, or another entry is the one playing: the next
    /// start is a start again.
    pub(super) fn forget_start(&mut self) {
        self.started = None;
    }

    /// What the recently played keeps of a stream of a directory; `None` for
    /// an address a person typed.
    fn recent_of(&self, stream: &Stream, now_unix_seconds: i64) -> Option<Pick> {
        let (kind, id) = match &stream.origin {
            Origin::RadioBrowser { uuid } => (Kind::Radio, uuid.clone()),
            Origin::Iptv { channel } => (Kind::Tv, channel.clone()),
            Origin::Archive { identifier } => (Kind::Archive, identifier.clone()),
            Origin::YouTube { id } => (Kind::Live, id.clone()),
            Origin::Typed => return None,
        };
        let (title, country, kept) = self.row(kind, &id).unwrap_or_else(|| {
            // NOT ON A PAGE ANY MORE: what the stream says of itself — and a
            // film of the archive is kept WHOLE, as its page, so playing it
            // again opens every part of it and not the one that played.
            let kept = match kind {
                Kind::Archive => archive::details_address(&id).map_or_else(
                    |_| stream.clone(),
                    |url| Stream {
                        url,
                        alternates: Vec::new(),
                        ..stream.clone()
                    },
                ),
                Kind::Radio | Kind::Tv | Kind::Live => stream.clone(),
            };
            let title = stream.title.clone().unwrap_or_else(|| stream.url.clone());
            (title, None, kept)
        });
        Some(Pick {
            kind,
            id,
            title,
            country,
            stream: kept,
            at_unix_seconds: now_unix_seconds,
        })
    }
}

impl Engine {
    /// A directory's listing asked (`Ask::Find`): its consent checked now,
    /// then listed on its desk's thread, replacing the one under way.
    pub(super) fn look(&mut self, query: Query) -> Result<(), Refused> {
        let prefs = prefs::store().read();
        admitted(&prefs, &query, ronoctua_ytdlp::installed())?;
        let shared = self.shared()?;
        let kind = query.desk();
        let context = self.directories.context.clone();
        let Some(slot) = self.directories.slot(kind) else {
            return Err(Trouble::NotUnderstood {
                why: format!("{kind:?} is not a directory's desk"),
            }
            .into());
        };
        slot.search += 1;
        let serial = slot.search;
        slot.desk.trouble = None;
        slot.desk.query = Some(query.clone());
        slot.desk.refused.clear();
        slot.desk.page = Page::Nothing;
        slot.moved(true);
        let job = Job {
            serial,
            query,
            prefs,
        };
        let worker = slot
            .worker
            .get_or_insert_with(|| spawn_worker(kind, shared.clone(), context.clone()));
        if let Err(mpsc::SendError(job)) = worker.send(job) {
            // A THREAD THAT DIED is started again, once.
            let worker = spawn_worker(kind, shared, context);
            let _ = worker.send(job);
            slot.worker = Some(worker);
        }
        self.dirty = true;
        Ok(())
    }

    /// A listing's page, posted by its thread — kept only while it is still
    /// the desk's listing.
    fn post_listing(&mut self, kind: DeskKind, serial: u64, page: Page, refused: Vec<Trouble>) {
        if !self.directories.is_current(kind, serial) {
            return;
        }
        if let Some(slot) = self.directories.slot(kind) {
            slot.desk.page = page;
            slot.desk.refused = refused;
            slot.moved(true);
        }
        self.dirty = true;
    }

    /// A listing ended: the page stays as it was posted — a stale one
    /// included — and the trouble, when there is one, is said.
    fn listing_ended(&mut self, kind: DeskKind, serial: u64, trouble: Option<Trouble>) {
        if !self.directories.is_current(kind, serial) {
            return;
        }
        if let Some(slot) = self.directories.slot(kind) {
            slot.desk.trouble = trouble;
            slot.moved(false);
        }
        self.dirty = true;
    }

    /// The entry that plays STARTED (mpv's `FILE_LOADED`): when it is a
    /// stream of a directory, what it is owed is queued for the fetch thread.
    pub(super) fn stream_started(&mut self) {
        let Some(current) = self.state.current else {
            return;
        };
        let Some(ronoctua_player::queue::Body::Stream(stream)) =
            self.queue.find(current).map(|e| e.body.clone())
        else {
            return;
        };
        let prefs = prefs::store().read();
        let owed = self
            .directories
            .started(current, &stream, &prefs, unix(SystemTime::now()));
        self.fetches.extend(owed);
    }

    /// The pick `(kind, id)` as a page shows it now, for a favourite.
    fn favourite_of(&self, kind: Kind, id: &str) -> Option<Pick> {
        let (title, country, stream) = self.directories.row(kind, id)?;
        Some(Pick {
            kind,
            id: id.to_owned(),
            title,
            country,
            stream,
            at_unix_seconds: unix(SystemTime::now()),
        })
    }
}

/// A star pressed (`Ask::Favourite`) — on the asking connection's thread,
/// the file written outside the lock. What is kept comes from a directory's
/// page, else the recently played, else the favourites themselves.
pub(super) fn favourite(kind: Kind, id: &str, on: bool, shared: &Shared) -> Say {
    let shelf = Shelf::favourites(table::FAVOURITES_MOST);
    let change = if on {
        let listed = shared.0.lock().ok().and_then(|e| e.favourite_of(kind, id));
        let found = listed.or_else(|| {
            let same = |p: &Pick| p.kind == kind && p.id == id;
            Shelf::recent(table::RECENT_MOST)
                .read()
                .into_iter()
                .find(same)
                .or_else(|| shelf.read().into_iter().find(same))
                .map(|p| Pick {
                    at_unix_seconds: unix(SystemTime::now()),
                    ..p
                })
        });
        match found {
            Some(pick) => Change::Add(pick),
            None => {
                return refuse(
                    Trouble::NoSuchPick {
                        kind,
                        id: id.to_owned(),
                    },
                    shared,
                );
            }
        }
    } else {
        Change::Remove(kind, id.to_owned())
    };
    match shelf.change(&change) {
        Ok(_) => Say::Done,
        Err(why) => refuse(Trouble::Picks { why }, shared),
    }
}

/// A refusal, said to the one who asked and in the state's troubles: most
/// doors tell and do not wait for the answer.
fn refuse(trouble: Trouble, shared: &Shared) -> Say {
    with_engine(shared, |e| {
        e.state.troubles = vec![trouble.clone()];
        e.dirty = true;
    });
    Say::Refused(trouble)
}

/// Seconds since the epoch.
fn unix(at: SystemTime) -> i64 {
    at.duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Whether this listing may be asked now — refused AT ONCE otherwise,
/// before anything is read or asked: a directory the person did not let
/// (naming its host), the lives without yt-dlp, a theme that has none of
/// what is asked, a search with no words.
pub(super) fn admitted(prefs: &Preferences, query: &Query, yt_dlp: bool) -> Result<(), Trouble> {
    let Some(directory) = query.directory() else {
        return Ok(());
    };
    if !prefs.allows_directory(directory) {
        return Err(Trouble::Off {
            host: directory.host().to_owned(),
        });
    }
    let no_words = |source: Source| Trouble::Directory(Refusal::new(source, Why::NothingToAsk));
    let theme = |id: &str, has: fn(&Category) -> bool| {
        Category::of(id)
            .filter(|c| has(c))
            .map(drop)
            .ok_or_else(|| Trouble::NoSuchCategory {
                category: id.to_owned(),
            })
    };
    match query {
        Query::Radio(
            RadioList::Search { words: w, .. }
            | RadioList::Tag { tag: w, .. }
            | RadioList::Language { language: w, .. },
        ) if w.trim().is_empty() => Err(no_words(Source::Radio)),
        Query::Tv(TvList::Search { words }) if words.trim().is_empty() => {
            Err(no_words(Source::Iptv))
        }
        Query::Tv(TvList::Category { category }) => theme(category, |c| !c.iptv.is_empty()),
        Query::Archive { category, .. } => theme(category, |c| !c.archive.is_empty()),
        Query::Lives { category } => {
            theme(category, |c| !c.lives.is_empty())?;
            if yt_dlp {
                Ok(())
            } else {
                Err(Trouble::Listing {
                    what: category.clone(),
                    why: ronoctua_ytdlp::Trouble::Absent,
                })
            }
        }
        _ => Ok(()),
    }
}

/// A desk's thread: its listings one at a time, in the order asked.
fn spawn_worker(kind: DeskKind, shared: Shared, context: Context) -> mpsc::Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    let name = match kind {
        DeskKind::Radio => "player-radio",
        DeskKind::Television => "player-television",
        DeskKind::Channels => "player-channels",
        DeskKind::Archive => "player-archive",
        _ => "player-lives",
    };
    let _ = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                run(kind, &job, &shared, &context);
            }
        });
    tx
}

/// Whether listing `serial` is still the desk's, and the player not closing.
fn still(shared: &Shared, kind: DeskKind, serial: u64) -> bool {
    shared
        .0
        .lock()
        .is_ok_and(|e| e.directories.is_current(kind, serial))
}

/// A seed for choosing among radio-browser's servers, so two machines do not
/// all ask the first.
fn seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::from(d.subsec_nanos()));
    nanos ^ u64::from(std::process::id())
}

/// One listing, on its desk's thread.
fn run(kind: DeskKind, job: &Job, shared: &Shared, context: &Context) {
    if !still(shared, kind, job.serial) {
        return;
    }
    let cache = player_cache();
    let tools = Tools {
        net: &Internet,
        cache: cache.as_ref(),
        map: atlas_map(),
        radio: &context.radio,
        index: &context.index,
        seed: seed(),
        now: SystemTime::now(),
        yt_dlp: ronoctua_ytdlp::installed(),
        lives: &ronoctua_ytdlp::search_lives,
    };
    let mut post = |page: Page, refused: Vec<Trouble>| {
        with_engine(shared, |e| e.post_listing(kind, job.serial, page, refused));
    };
    let outcome = look(&job.query, &job.prefs, &tools, &mut post);
    if let Err(t) = &outcome {
        tracing::info!(desk = ?kind, trouble = ?t, "a directory gave nothing");
    }
    with_engine(shared, |e| e.listing_ended(kind, job.serial, outcome.err()));
}

/// Where the world is, as a directory's page needs it.
pub(super) trait Map: Sync {
    /// Where a country's name goes on a map.
    fn anchor(&self, country: Code) -> Option<Spot>;
    /// Where a place a TV feed names is, in its country.
    fn place(&self, country: Code, name: &str) -> Option<Spot>;
}

/// The atlas the doors draw the globe with (`RONOCTUA_ATLAS`), read ONCE and
/// kept as what the pages need of it: the few hundred anchors. Without it
/// every row is "no place on the map", and says so.
struct AtlasMap {
    anchors: HashMap<Code, Spot>,
}

impl Map for AtlasMap {
    fn anchor(&self, country: Code) -> Option<Spot> {
        self.anchors.get(&country).copied()
    }

    /// NOT YET: the atlas finds the place NEAR a coordinate, and has no
    /// finder by name — its places carry no country to search within
    /// (`ronoctua_atlas::place_near`). Until it has one, a channel's city is
    /// said on its row and the channel is drawn on its country's anchor.
    fn place(&self, _country: Code, _name: &str) -> Option<Spot> {
        None
    }
}

fn atlas_map() -> &'static AtlasMap {
    static MAP: OnceLock<AtlasMap> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut anchors = HashMap::new();
        // FROM THE ENVIRONMENT, as the shell reads it (`globe::world`): the
        // unit sets it where the atlas is installed, and a build with none
        // runs with none. A file named and not read is said: a silent
        // fallback would be a globe with every station in no place.
        let Some(path) = std::env::var_os("RONOCTUA_ATLAS") else {
            tracing::info!("no atlas: the directories' rows have no place on the map");
            return AtlasMap { anchors };
        };
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(error = %e, "the atlas could not be read: the directories' rows have no place");
                return AtlasMap { anchors };
            }
        };
        let Ok(atlas) = ronoctua_atlas::Atlas::open(&bytes) else {
            tracing::warn!("the atlas would not open: the directories' rows have no place");
            return AtlasMap { anchors };
        };
        for a in b'A'..=b'Z' {
            for b in b'A'..=b'Z' {
                let letters = [a, b];
                let Ok(code) = country::normal(std::str::from_utf8(&letters).unwrap_or("")) else {
                    continue;
                };
                if code.0 == letters
                    && let Some(at) = ronoctua_atlas::anchor_of(&atlas, letters)
                {
                    anchors.insert(
                        code,
                        Spot {
                            lat: at.lat,
                            lon: at.lon,
                        },
                    );
                }
            }
        }
        AtlasMap { anchors }
    })
}

/// yt-dlp's search of what is live (`ronoctua_ytdlp::search_lives`), as a
/// seam: a test hands its own, and never runs a program.
pub(super) type SearchLives<'a> =
    dyn Fn(&str, usize) -> Result<Vec<Live>, ronoctua_ytdlp::Trouble> + 'a;

/// What a listing works with — the network, the cache, the map and yt-dlp,
/// each a seam a test hands its own.
pub(super) struct Tools<'a> {
    pub(super) net: &'a dyn Net,
    pub(super) cache: Option<&'a Cache>,
    pub(super) map: &'a dyn Map,
    pub(super) radio: &'a Mutex<Option<Radio>>,
    pub(super) index: &'a Mutex<()>,
    pub(super) seed: u64,
    pub(super) now: SystemTime,
    pub(super) yt_dlp: bool,
    pub(super) lives: &'a SearchLives<'a>,
}

/// Where a listing's pages go: a stale one first when there is one, then the
/// fresh one — each with what gave nothing while the rest answered.
pub(super) type Post<'p> = &'p mut dyn FnMut(Page, Vec<Trouble>);

/// One listing: consent first, then the cache, then the directory. Blocks
/// for as long as the directory takes: call it on a desk's thread.
///
/// # Errors
///
/// Its refusal — posted pages stay, a stale one included.
pub(super) fn look(
    query: &Query,
    prefs: &Preferences,
    tools: &Tools<'_>,
    post: Post<'_>,
) -> Result<(), Trouble> {
    // CHECKED AGAIN here, not only when asked: this is the one function that
    // reaches a directory, and nothing reaches one that is not let.
    admitted(prefs, query, tools.yt_dlp)?;
    match query {
        Query::Radio(list) => radio_list(list, tools, post),
        Query::Tv(list) => tv_list(list, tools, post),
        Query::Archive {
            category,
            words,
            page,
        } => archive_page(category, words.as_deref(), *page, tools, post),
        Query::Lives { category } => lives(category, tools, post),
        Query::Suggest { .. } | Query::Subtitles { .. } | Query::Lyrics { .. } => {
            Err(Trouble::NotUnderstood {
                why: "a lookup about a file is not a directory's listing".to_owned(),
            })
        }
    }
}

/// A list kept in the cache under `name` — none for what a person typed —
/// shown at once when it is there; asked of the directory when it is older
/// than `fresh_minutes` or not there, kept, and shown again.
fn listed<T: Serialize + DeserializeOwned>(
    tools: &Tools<'_>,
    name: Option<&str>,
    fresh_minutes: u64,
    ask: impl FnOnce() -> Result<(T, Vec<Trouble>), Trouble>,
    mut show: impl FnMut(T, Listed, Vec<Trouble>),
) -> Result<(), Trouble> {
    let fresh_for = Duration::from_secs(fresh_minutes.saturating_mul(60));
    let kept = name.and_then(|n| tools.cache?.read(n, fresh_for, tools.now).ok().flatten());
    if let Some(kept) = kept
        && let Ok(value) = serde_json::from_slice::<T>(&kept.bytes)
    {
        let listed = Listed {
            fetched_unix_seconds: Some(unix(kept.written)),
            stale: !kept.fresh,
        };
        show(value, listed, Vec::new());
        if kept.fresh {
            return Ok(());
        }
    }
    let (value, refused) = ask()?;
    if let (Some(n), Some(cache)) = (name, tools.cache)
        && let Ok(bytes) = serde_json::to_vec(&value)
        && let Err(why) = cache.write(n, &bytes, None)
    {
        tracing::info!(name = n, ?why, "a directory's list was not kept");
    }
    let listed = Listed {
        fetched_unix_seconds: Some(unix(tools.now)),
        stale: false,
    };
    show(value, listed, refused);
    Ok(())
}

/// A directory's refusal, as the trouble a page says.
const fn refused(r: Refusal) -> Trouble {
    Trouble::Directory(r)
}

/// A cache name for a value a person did not type but a list gave (a genre,
/// a language): its FNV-1a hash, since the cache takes only plain names.
fn hashed(value: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in value.trim().to_lowercase().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The radio-browser server this session asks: the one found, else one
/// found now — kept only when it is a mirror, so an entry used because the
/// list could not be had is asked for again next time.
fn server(tools: &Tools<'_>) -> Radio {
    if let Some(found) = tools
        .radio
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
    {
        return found;
    }
    let found = Radio::discover(tools.net, tools.seed);
    if found.host() != radio::ENTRY {
        *tools.radio.lock().unwrap_or_else(PoisonError::into_inner) = Some(found.clone());
    }
    found
}

fn radio_list(list: &RadioList, tools: &Tools<'_>, post: Post<'_>) -> Result<(), Trouble> {
    let fresh = table::FRESH_RADIO_MINUTES;
    match list {
        RadioList::Countries => listed(
            tools,
            Some("radio-countries.json"),
            fresh,
            || {
                Ok((
                    server(tools).countries(tools.net).map_err(refused)?,
                    Vec::new(),
                ))
            },
            |rows: Vec<CountryCount>, listed, refused| {
                let (countries, globe) =
                    countries_globe(rows.iter().map(|c| (c.country, c.stations)), tools.map);
                post(
                    Page::RadioCountries {
                        countries,
                        globe,
                        listed,
                    },
                    refused,
                );
            },
        ),
        RadioList::Country { country, page } => stations(
            tools,
            Some(paged(format!("radio-country-{country}"), *page)),
            &By::Country(*country),
            *page,
            post,
        ),
        RadioList::Tag { tag, page } => stations(
            tools,
            Some(paged(format!("radio-tag-{}", hashed(tag)), *page)),
            &By::Tag(tag.clone()),
            *page,
            post,
        ),
        RadioList::Language { language, page } => stations(
            tools,
            Some(paged(format!("radio-language-{}", hashed(language)), *page)),
            &By::Language(language.clone()),
            *page,
            post,
        ),
        RadioList::Search { words, page } => {
            stations(tools, None, &By::Name(words.clone()), *page, post)
        }
        RadioList::Tags => named(tools, "radio-tags.json", false, post),
        RadioList::Languages => named(tools, "radio-languages.json", true, post),
    }
}

/// The name a page of a list is kept under in the cache: the list's own for
/// its first page — what was kept before pages were — and `-p<n>` after it.
fn paged(stem: String, page: u32) -> String {
    if page <= 1 {
        format!("{stem}.json")
    } else {
        format!("{stem}-p{page}.json")
    }
}

/// A PAGE of a list of stations — by a name, what a person typed: the name
/// first, then the genre (`Radio::search`) — the `page`th run of
/// `table::STATIONS_MOST`, the most played first.
fn stations(
    tools: &Tools<'_>,
    name: Option<String>,
    by: &By,
    page: u32,
    post: Post<'_>,
) -> Result<(), Trouble> {
    let skip = page
        .max(1)
        .saturating_sub(1)
        .saturating_mul(table::STATIONS_MOST);
    listed(
        tools,
        name.as_deref(),
        table::FRESH_RADIO_MINUTES,
        || {
            let server = server(tools);
            let found = match by {
                By::Name(words) => server.search(tools.net, words, table::STATIONS_MOST, skip),
                other => server.stations(tools.net, other, table::STATIONS_MOST, skip),
            };
            Ok((found.map_err(refused)?, Vec::new()))
        },
        |found: Vec<Station>, listed, refused| {
            let (stations, globe) = stations_globe(found, tools.map);
            post(
                Page::Stations {
                    stations,
                    globe,
                    listed,
                },
                refused,
            );
        },
    )
}

/// The genres, or the languages — each language named by the table of
/// languages when its code is there.
fn named(tools: &Tools<'_>, name: &str, languages: bool, post: Post<'_>) -> Result<(), Trouble> {
    listed(
        tools,
        Some(name),
        table::FRESH_RADIO_MINUTES,
        || {
            let server = server(tools);
            let found = if languages {
                server.languages(tools.net, NAMED_MOST)
            } else {
                server.tags(tools.net, NAMED_MOST)
            };
            Ok((found.map_err(refused)?, Vec::new()))
        },
        |found: Vec<Named>, listed, refused| {
            let rows: Vec<NamedRow> = found
                .into_iter()
                .map(|n| NamedRow {
                    tongue: languages
                        .then(|| tongue_of(&n.name, n.code.as_deref()))
                        .flatten(),
                    name: n.name,
                    stations: n.stations,
                })
                .collect();
            let page = if languages {
                Page::Languages {
                    languages: rows,
                    listed,
                }
            } else {
                Page::Genres {
                    genres: rows,
                    listed,
                }
            };
            post(page, refused);
        },
    )
}

/// The language of the table radio-browser's `name` and two-letter `code`
/// name, as the code a door names it by (`tongues::name_key`): the row its
/// words say when they say a region (`brazilian portuguese` is `pt-BR`,
/// `radio::language_region`); the one row that has the code; and a code two
/// rows share (`pt`, Brazil's and Portugal's) as that bare code — the door
/// names it as the language, with no region, which is what radio-browser's
/// `portuguese` says. `None` for a code no row has. Pure.
fn tongue_of(name: &str, code: Option<&str>) -> Option<String> {
    if let Some(tag) = ronoctua_broadcast::radio::language_region(name) {
        return tongues::of(tag).map(|t| t.code.to_owned());
    }
    let code = code?.trim().to_ascii_lowercase();
    let mut rows = tongues::TONGUES.iter().filter(|t| t.two_letter == code);
    let one = rows.next()?;
    Some(if rows.next().is_none() {
        one.code.to_owned()
    } else {
        code
    })
}

/// The TV index: the one kept while it is fresh; else renewed — under a lock
/// of this process, so the two TV desks never download it at once — with the
/// stale one shown meanwhile (`stale`).
fn index(
    tools: &Tools<'_>,
    mut stale: impl FnMut(&Index, Listed),
) -> Result<(Index, Listed), Trouble> {
    const NAME: &str = "iptv-index.json";
    let fresh_for = Duration::from_secs(table::FRESH_IPTV_MINUTES.saturating_mul(60));
    let read = || {
        let kept = tools
            .cache?
            .read(NAME, fresh_for, tools.now)
            .ok()
            .flatten()?;
        let index = Index::from_bytes(&kept.bytes).ok()?;
        let listed = Listed {
            fetched_unix_seconds: Some(index.fetched_unix_seconds),
            stale: !kept.fresh,
        };
        Some((index, listed))
    };
    if let Some((index, listed)) = read() {
        if !listed.stale {
            return Ok((index, listed));
        }
        stale(&index, listed);
    }
    let _renewing = tools.index.lock().unwrap_or_else(PoisonError::into_inner);
    // THE OTHER TV DESK may have renewed it while this one waited.
    if let Some((index, listed)) = read()
        && !listed.stale
    {
        return Ok((index, listed));
    }
    let index = iptv::refresh(tools.net, table::EXCLUDED, unix(tools.now)).map_err(refused)?;
    if let Some(cache) = tools.cache
        && let Err(why) = cache.write(NAME, &index.to_bytes(), None)
    {
        tracing::info!(?why, "the TV index was not kept");
    }
    let listed = Listed {
        fetched_unix_seconds: Some(index.fetched_unix_seconds),
        stale: false,
    };
    Ok((index, listed))
}

fn tv_list(list: &TvList, tools: &Tools<'_>, post: Post<'_>) -> Result<(), Trouble> {
    let (index, listed) = index(tools, |index, listed| {
        post(tv_page(list, index, tools.map, listed), Vec::new());
    })?;
    post(tv_page(list, &index, tools.map, listed), Vec::new());
    Ok(())
}

/// A TV page out of the index. Pure.
fn tv_page(list: &TvList, index: &Index, map: &dyn Map, listed: Listed) -> Page {
    let channels = match list {
        TvList::Countries => {
            let (countries, mut globe) = countries_globe(index.countries(), map);
            globe.placeless = globe
                .placeless
                .saturating_add(u32::try_from(index.placeless()).unwrap_or(u32::MAX));
            return Page::TvCountries {
                countries,
                globe,
                listed,
            };
        }
        TvList::Country { country } => index.in_country(*country),
        TvList::Category { category } => {
            Category::of(category).map_or_else(Vec::new, |c| index.in_categories(c.iptv))
        }
        TvList::Search { words } => index.search(words, index.channels.len()),
        // NO PLACE ON THE MAP: no country, or one the map has no point for —
        // the world page's count, open.
        TvList::Placeless => index
            .channels
            .iter()
            .filter(|c| c.country.is_none_or(|code| map.anchor(code).is_none()))
            .collect(),
    };
    let total = u32::try_from(channels.len()).unwrap_or(u32::MAX);
    let (channels, globe) = channels_globe(&channels, index, map);
    Page::Channels {
        channels,
        total,
        globe,
        listed,
    }
}

fn archive_page(
    category: &str,
    words: Option<&str>,
    page: u32,
    tools: &Tools<'_>,
    post: Post<'_>,
) -> Result<(), Trouble> {
    let c = Category::of(category).ok_or_else(|| Trouble::NoSuchCategory {
        category: category.to_owned(),
    })?;
    let words = words.map(str::trim).filter(|w| !w.is_empty());
    let page = page.max(1);
    // A PAGE OF THE THEME ITSELF is kept; one a person's words made is not.
    let name = words
        .is_none()
        .then(|| format!("archive-{}-{page}.json", c.id));
    listed(
        tools,
        name.as_deref(),
        table::FRESH_ARCHIVE_MINUTES,
        || {
            let search = archive::Search {
                collections: c.archive,
                subjects: c.subjects,
                words,
                page,
                rows: table::ARCHIVE_ROWS,
            };
            Ok((
                archive::search(tools.net, &search).map_err(refused)?,
                Vec::new(),
            ))
        },
        |found: archive::Found, listed, refused| {
            let items = found
                .items
                .into_iter()
                .filter_map(|item| {
                    Some(ArchiveRow {
                        stream: item.stream()?,
                        item,
                    })
                })
                .collect();
            post(
                Page::Archive {
                    category: c.id.to_owned(),
                    items,
                    total: found.total,
                    number: found.page,
                    more: found.more,
                    listed,
                },
                refused,
            );
        },
    )
}

/// A live found on YouTube, as the queue plays it.
fn live_stream(live: &Live) -> Stream {
    Stream {
        url: live.url.clone(),
        title: Some(live.title.trim().to_owned()).filter(|t| !t.is_empty()),
        referrer: None,
        user_agent: None,
        alternates: Vec::new(),
        origin: Origin::YouTube {
            id: live.id.clone(),
        },
        live: true,
    }
}

fn lives(category: &str, tools: &Tools<'_>, post: Post<'_>) -> Result<(), Trouble> {
    let c = Category::of(category).ok_or_else(|| Trouble::NoSuchCategory {
        category: category.to_owned(),
    })?;
    listed(
        tools,
        Some(&format!("lives-{}.json", c.id)),
        table::FRESH_LIVES_MINUTES,
        || {
            let mut rows: Vec<LiveRow> = Vec::new();
            let mut gave_nothing: Vec<Trouble> = Vec::new();
            for words in c.lives {
                match (tools.lives)(words, table::LIVES_MOST) {
                    Ok(found) => {
                        for live in found {
                            if !rows.iter().any(|r| r.live.id == live.id) {
                                rows.push(LiveRow {
                                    stream: live_stream(&live),
                                    asked: (*words).to_owned(),
                                    live,
                                });
                            }
                        }
                    }
                    // SAID BY THE THEME, never by the words it searched
                    // with: those are the table's, in English, and the door
                    // names the theme in the person's language — once, however
                    // many of its searches failed the same way.
                    Err(why) => {
                        let t = Trouble::Listing {
                            what: c.id.to_owned(),
                            why,
                        };
                        if !gave_nothing.contains(&t) {
                            gave_nothing.push(t);
                        }
                    }
                }
            }
            // NOTHING AT ALL, and a search that failed: the failure is the
            // page's. Every search answering with nothing live is a page of
            // nothing, which is true.
            if rows.is_empty()
                && let Some(first) = gave_nothing.first()
            {
                return Err(first.clone());
            }
            Ok((rows, gave_nothing))
        },
        |rows: Vec<LiveRow>, listed, refused| {
            post(
                Page::Lives {
                    category: c.id.to_owned(),
                    lives: rows,
                    listed,
                },
                refused,
            );
        },
    )
}

/// The marks of a page, made as its rows are.
#[derive(Default)]
struct Marks {
    marks: Vec<Mark>,
    placeless: u32,
    by_key: HashMap<String, u32>,
}

impl Marks {
    /// The mark under `key` — made when this is its first row — with one
    /// more row on it.
    fn on(
        &mut self,
        key: String,
        at: Spot,
        kind: MarkKind,
        label: &str,
        country: Option<Code>,
    ) -> u32 {
        if let Some(&id) = self.by_key.get(&key) {
            if let Some(m) = self.marks.get_mut(id as usize) {
                m.weight = m.weight.saturating_add(1);
            }
            return id;
        }
        let id = u32::try_from(self.marks.len()).unwrap_or(u32::MAX);
        self.marks.push(Mark {
            id,
            at,
            weight: 1,
            kind,
            label: label.to_owned(),
            country,
        });
        self.by_key.insert(key, id);
        id
    }

    /// A row with no place on the map.
    fn nowhere(&mut self) -> Option<u32> {
        self.placeless = self.placeless.saturating_add(1);
        None
    }

    /// Its country's anchor, for a row that says no more than its country.
    fn anchored(&mut self, country: Option<Code>, map: &dyn Map) -> Option<u32> {
        match country.and_then(|c| map.anchor(c).map(|at| (c, at))) {
            Some((c, at)) => Some(self.on(
                format!("country:{c}"),
                at,
                MarkKind::Country,
                c.as_str(),
                Some(c),
            )),
            None => self.nowhere(),
        }
    }

    fn globe(self) -> Globe {
        Globe {
            marks: self.marks,
            placeless: self.placeless,
        }
    }
}

/// Countries, each a mark at its anchor weighing what it has. Pure.
fn countries_globe(
    rows: impl IntoIterator<Item = (Code, u32)>,
    map: &dyn Map,
) -> (Vec<CountryRow>, Globe) {
    let mut marks = Marks::default();
    let mut out = Vec::new();
    for (country, count) in rows {
        let mark = match map.anchor(country) {
            Some(at) => {
                let id = marks.on(
                    format!("country:{country}"),
                    at,
                    MarkKind::Country,
                    country.as_str(),
                    Some(country),
                );
                if let Some(m) = marks.marks.get_mut(id as usize) {
                    m.weight = count;
                }
                Some(id)
            }
            None => {
                marks.placeless = marks.placeless.saturating_add(count);
                None
            }
        };
        out.push(CountryRow {
            country,
            count,
            mark,
        });
    }
    (out, marks.globe())
}

/// Stations, each at its own coordinate when it says one, else on its
/// country's anchor with the others of that country that do not. Pure.
fn stations_globe(stations: Vec<Station>, map: &dyn Map) -> (Vec<StationRow>, Globe) {
    let mut marks = Marks::default();
    let rows = stations
        .into_iter()
        .map(|station| {
            let mark = match station.at {
                Some(at) => Some(marks.on(
                    format!("station:{}", station.uuid),
                    at,
                    MarkKind::Station,
                    &station.name,
                    station.country,
                )),
                None => marks.anchored(station.country, map),
            };
            StationRow {
                stream: station.stream(),
                station,
                mark,
            }
        })
        .collect();
    (rows, marks.globe())
}

/// Channels, at most [`CHANNELS_MOST`], each where the first place its feeds
/// name is when the map finds it, else on its country's anchor. Pure.
fn channels_globe(channels: &[&Channel], index: &Index, map: &dyn Map) -> (Vec<ChannelRow>, Globe) {
    let mut marks = Marks::default();
    let mut rows = Vec::new();
    for c in channels.iter().take(CHANNELS_MOST) {
        let Some(stream) = c.stream() else { continue };
        let places: Vec<(&iptv::Area, &iptv::PlaceName)> = c
            .areas
            .iter()
            .filter_map(|a| index.place(a).map(|p| (a, p)))
            .collect();
        let found = places.iter().find_map(|(area, p)| {
            let country = p.country.or(c.country)?;
            map.place(country, &p.name)
                .map(|at| (*area, *p, country, at))
        });
        let mark = match found {
            Some((area, p, country, at)) => Some(marks.on(
                format!("place:{area:?}"),
                at,
                MarkKind::Place,
                &p.name,
                Some(country),
            )),
            None => marks.anchored(c.country, map),
        };
        rows.push(ChannelRow {
            id: c.id.clone(),
            name: c.name.clone(),
            also: c.also.clone(),
            country: c.country,
            categories: c.categories.clone(),
            lines: c.streams.first().and_then(|s| s.lines),
            geo_blocked: c.streams.iter().all(|s| s.geo_blocked),
            sometimes_off: c.sometimes_off(),
            areas: places.iter().map(|(_, p)| p.name.clone()).collect(),
            stream,
            mark,
        });
    }
    (rows, marks.globe())
}

#[cfg(test)]
mod tests;
