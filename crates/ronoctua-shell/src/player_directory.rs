//! THE DIRECTORIES, as a door sees them (plan P9): world radio, world TV,
//! channels by theme — live TV, the Internet Archive's films, what is live
//! on YouTube — and the favourites and the played lately.
//!
//! # The daemon lists, the door shows
//!
//! Every list is the daemon's, on a DESK of its own — radio, television, a
//! theme's channels, the archive, the lives (`ronoctua_player::find`) — one
//! page per desk for the whole machine. So a page of a door ASKS FOR ITS
//! LIST as it opens, and again when it is come back to ([`View::owing`]):
//! the desk may hold what another door, or another page of this one, asked
//! for. What the desk said of each list on this door's stack is kept
//! (`player_find::Found::listing`), so a page whose desk another door took
//! still shows its own. The daemon's cache answers a list asked again
//! within its freshness without a word to the network.
//!
//! # A press on a row that names the host is the consent
//!
//! No directory is born allowed. Until one is, its page says whom it would
//! ask and a row that asks it ([`Target::Asks`]); pressing it allows that
//! directory and asks — nothing reaches the network before. A refusal is its
//! sentence, and a row that tries again.
//!
//! # A row that plays, and its star
//!
//! A station, a channel, a film, a live: `RET` plays it now — after what
//! plays, the queue kept — `a` adds it to the queue, and `b` or the star at
//! the head of its row keeps it among the favourites, or takes it off
//! (`Ask::Favourite`: the daemon writes the file, and every door reads it —
//! [`Picks`]).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::player::{Errand, PageId, Target, Typing, View};
use crate::player_rows::Builder;
use ronoctua_broadcast::picks::{Kind, Pick};
use ronoctua_broadcast::table::Category;
use ronoctua_i18n::{Arg, key};
use ronoctua_player::find::{Desk, Directory, Listed, Page, Query, RadioList, TvList};
use ronoctua_player::own::Stream;
use ronoctua_player::wire::{Ask, DeskKind, State, Thing};

/// What a page lists as it opens, when that is always the same list — the
/// world's countries, the genres, the languages, a theme's first page of
/// films. A list of stations or of channels is the level's own
/// ([`View::push_listing`]).
#[must_use]
pub fn listed_on(page: PageId) -> Option<Query> {
    match page {
        PageId::Radio => Some(Query::Radio(RadioList::Countries)),
        PageId::RadioGenres => Some(Query::Radio(RadioList::Tags)),
        PageId::RadioLanguages => Some(Query::Radio(RadioList::Languages)),
        PageId::Tv => Some(Query::Tv(TvList::Countries)),
        PageId::Theme(c) => (!c.archive.is_empty()).then(|| Query::Archive {
            category: c.id.to_owned(),
            words: None,
            page: 1,
        }),
        _ => None,
    }
}

/// Every list a page shows — one, and a theme's three: its live TV, its page
/// of films (the level's, which holds the words and the page number) and its
/// lives, each only where the theme has that source.
#[must_use]
pub fn queries_of(page: PageId, query: Option<&Query>) -> Vec<Query> {
    match page {
        PageId::Theme(c) => {
            let mut all = Vec::new();
            if !c.iptv.is_empty() {
                all.push(Query::Tv(TvList::Category {
                    category: c.id.to_owned(),
                }));
            }
            all.extend(query.cloned());
            if !c.lives.is_empty() {
                all.push(Query::Lives {
                    category: c.id.to_owned(),
                });
            }
            all
        }
        _ => query.cloned().into_iter().collect(),
    }
}

/// The directory's word in a row's spot.
const fn spot_word(d: Directory) -> &'static str {
    d.id()
}

/// What the view keeps of the directories: the lists owed to the player,
/// the favourites and the played lately, and the zone a list's date is said
/// in.
#[derive(Debug, Default)]
pub struct Directories {
    /// The lists a page that just came up asks for, sent with the door's next
    /// errand ([`View::owing`]).
    owed: Vec<Query>,
    /// The lists this door asked for and has not heard yet: a page says it
    /// is asking from the press on, and not only once the daemon's state
    /// says its desk is busy ([`Asked`]).
    asked: Vec<Asked>,
    /// The lists the player REFUSED at once, and how — never reaching their
    /// desk, so no page of theirs will say it: each page says it in place of
    /// "asking", with a row that tries again, until it is asked again.
    refused: Vec<(Query, crate::player::Told)>,
    /// The favourites and the played lately, as last read.
    pub(crate) picks: Picks,
    /// The machine's zone, a list's date is said in — `None` for a
    /// photograph's view, which says every date in UTC and is the same
    /// picture on every machine.
    zone: Option<ronoctua_files::when::Zone>,
}

/// A list this door asked for, until a page of it is here.
#[derive(Debug, Clone, PartialEq)]
struct Asked {
    /// The list.
    query: Query,
    /// The player took the ask: its desk works on it — unless another door
    /// asked that desk for another list meanwhile.
    taken: bool,
    /// The desk's generation when this door first saw the ask taken: a desk
    /// that has moved past it, is idle and holds another list replaced it.
    seen_at: Option<u64>,
}

impl Directories {
    /// The directories of a door on this machine: its zone read.
    #[must_use]
    pub fn of_this_machine() -> Self {
        Self {
            zone: Some(ronoctua_files::when::Zone::read_from(std::path::Path::new(
                "/etc/localtime",
            ))),
            ..Self::default()
        }
    }

    /// `query` asked from now: said "asking" until a page of it comes, and
    /// what the player refused of it before forgotten.
    fn asking(&mut self, query: &Query) {
        self.refused.retain(|(q, _)| q != query);
        self.asked.retain(|a| a.query != *query);
        self.asked.push(Asked {
            query: query.clone(),
            taken: false,
            seen_at: None,
        });
    }

    /// Every list asked of a player that is gone, and what it refused: the
    /// one that comes back has none of them under way — each page says it
    /// lists nothing yet, with a row that asks again.
    pub(crate) fn forget_the_asks(&mut self) {
        self.asked.clear();
        self.refused.clear();
    }

    /// Whether this door asked for `query` and has no page of it yet.
    fn asked(&self, query: &Query) -> bool {
        self.asked.iter().any(|a| a.query == *query)
    }

    /// How the player refused `query` at once, when it did.
    fn refusal(&self, query: &Query) -> Option<&crate::player::Told> {
        self.refused
            .iter()
            .find(|(q, _)| q == query)
            .map(|(_, t)| t)
    }
}

/// THE FAVOURITES AND THE PLAYED LATELY, as this door last read them.
///
/// Read by the door and WRITTEN BY THE DAEMON ONLY (`Ask::Favourite`, and
/// every start of a stream): a door never writes its copy, so two doors
/// never undo each other. A door with a loop watches both files and reads
/// them again when they move ([`Picks::look`]); a door with none — a
/// photograph, a test — is handed them ([`Picks::given`]) and reads none of
/// this machine's.
#[derive(Debug, Default)]
pub struct Picks {
    favourites: Vec<Pick>,
    recent: Vec<Pick>,
    /// Read at least once.
    read: bool,
    /// Read again at the next look: a page that shows them came up.
    stirred: Arc<AtomicBool>,
    /// The two files, watched while a door with a loop shows them — each
    /// from the look its folder is there, a folder that is not there yet
    /// looked for again on the next (`player_internet::Watched`).
    watched: crate::player_internet::Watched,
}

impl Picks {
    /// The two lists HANDED to the door — a photograph's, a test's.
    pub fn given(&mut self, favourites: Vec<Pick>, recent: Vec<Pick>) {
        self.favourites = favourites;
        self.recent = recent;
        self.read = true;
    }

    /// Read again on the next look — a page that shows them came up.
    pub fn stir(&self) {
        self.stirred.store(true, Ordering::SeqCst);
    }

    /// What a page that shows them takes on each settle: the two files read
    /// when they moved, and watched from the first look on, waking the door.
    /// With no `wake` — no loop — nothing of this machine's is read.
    pub fn look(&mut self, wake: Option<&crate::player_subtitles::Wake>) {
        let Some(wake) = wake else {
            return;
        };
        let places = [
            (
                ronoctua_xdg::config_dir(),
                ronoctua_broadcast::picks::FAVOURITES_FILE,
            ),
            (
                ronoctua_xdg::state_dir(),
                ronoctua_broadcast::picks::RECENT_FILE,
            ),
        ];
        let moved = self.watched.look(&places, wake);
        if moved || !self.read || self.stirred.swap(false, Ordering::SeqCst) {
            use ronoctua_broadcast::picks::Shelf;
            use ronoctua_broadcast::table::{FAVOURITES_MOST, RECENT_MOST};
            self.favourites = Shelf::favourites(FAVOURITES_MOST).read();
            self.recent = Shelf::recent(RECENT_MOST).read();
            self.read = true;
        }
    }

    /// Whether `(kind, id)` is kept among the favourites.
    #[must_use]
    pub fn starred(&self, kind: Kind, id: &str) -> bool {
        self.favourites.iter().any(|p| p.kind == kind && p.id == id)
    }

    /// The favourites, newest first.
    #[must_use]
    pub fn favourites(&self) -> &[Pick] {
        &self.favourites
    }

    /// The played lately, newest first.
    #[must_use]
    pub fn recent(&self) -> &[Pick] {
        &self.recent
    }
}

/// The ids of the questions this module asks — a search typed on a
/// directory's page — which the footer's minibuffer hands back by id.
#[must_use]
pub fn asks(id: &str) -> bool {
    [
        crate::player_question::SEARCH_RADIO,
        crate::player_question::SEARCH_TV,
        crate::player_question::SEARCH_ARCHIVE,
        crate::player_question::SEARCH_GENRE,
        crate::player_question::SEARCH_LANGUAGE,
    ]
    .contains(&id)
}

/// The question a directory's desk is searched with, typed.
const fn question_of(kind: DeskKind) -> Option<&'static str> {
    match kind {
        DeskKind::Radio => Some(crate::player_question::SEARCH_RADIO),
        DeskKind::Television => Some(crate::player_question::SEARCH_TV),
        DeskKind::Archive => Some(crate::player_question::SEARCH_ARCHIVE),
        _ => None,
    }
}

/// The desk a search question is answered on.
fn desk_of_question(id: &str) -> Option<DeskKind> {
    [DeskKind::Radio, DeskKind::Television, DeskKind::Archive]
        .into_iter()
        .find(|k| question_of(*k) == Some(id))
}

/// What playing `stream` asks: now, after what plays and the queue kept —
/// or at the queue's end — and the sentence it is said with.
fn put(stream: Stream, name: &str, play: bool, texts: &ronoctua_i18n::Texts) -> Errand {
    Errand::TellSaying(
        Ask::Put {
            what: vec![Thing::Stream { stream }],
            after: None,
            into: None,
            play,
            replace: false,
        },
        texts.t_with(
            if play {
                key::PLAYER_PLAYING_NOW
            } else {
                key::PLAYER_ADDED
            },
            &[("name", name)],
        ),
    )
}

/// Whether `errand` reaches the player, and a list asked may ride after it —
/// an errand of the door's (a question, closing) keeps the lists for the
/// next.
const fn rides(errand: &Errand) -> bool {
    matches!(
        errand,
        Errand::Nothing
            | Errand::Said(_)
            | Errand::Tell(_)
            | Errand::TellSaying(..)
            | Errand::Then(..)
    )
}

/// `errand`, and after it `asks`, in order — the lists owed to the player.
fn then_asking(errand: Errand, asks: Vec<Ask>) -> Errand {
    asks.into_iter()
        .map(Errand::Tell)
        .fold(errand, |so_far, next| {
            if so_far == Errand::Nothing {
                next
            } else {
                Errand::Then(Box::new(so_far), Box::new(next))
            }
        })
}

impl View {
    /// THE PAGE ON TOP CAME UP — opened, or come back to: its lists are owed
    /// to the player, and the favourites read again where it shows them.
    pub(crate) fn listing_follows(&mut self) {
        let page = self.page();
        let owed = queries_of(page, self.top().query.as_ref());
        self.directories.owed = owed;
        if matches!(
            page,
            PageId::Favourites | PageId::Recent | PageId::Radio | PageId::Tv
        ) || !self.directories.owed.is_empty()
        {
            self.directories.picks.stir();
            // AND THE PERSON'S CONSENTS AS THEY ARE ON DISK, for a door on
            // this machine: read again on the next look, which watches the
            // file from then on. A photograph's view keeps its defaults.
            self.prefs_on_disk.stir();
        }
    }

    /// `errand`, and the lists a page that came up owes the player — asked
    /// only of a directory the person allowed: a page of one not allowed
    /// says whom it would ask, and waits for the press.
    pub(crate) fn owing(&mut self, errand: Errand) -> Errand {
        let owed = std::mem::take(&mut self.directories.owed);
        let asks: Vec<Ask> = owed
            .iter()
            .filter(|q| {
                q.directory()
                    .is_some_and(|d| self.prefs.allows_directory(d))
            })
            .map(|q| Ask::Find { query: q.clone() })
            .collect();
        if asks.is_empty() {
            return errand;
        }
        if !rides(&errand) {
            self.directories.owed = owed;
            return errand;
        }
        for ask in &asks {
            if let Ask::Find { query } = ask {
                self.directories.asking(query);
            }
        }
        then_asking(errand, asks)
    }

    /// How an ask for a list ended ([`crate::player::Told::Listing`]): taken,
    /// its desk works on it; anything else — a directory not allowed, no
    /// yt-dlp for the lives, a theme that has none of it, no player — is the
    /// page's to say, in place of "asking".
    pub(crate) fn listing_told(&mut self, query: &Query, answer: &crate::player::Told) {
        let d = &mut self.directories;
        if *answer == crate::player::Told::Taken || *answer == crate::player::Told::Working {
            if let Some(a) = d.asked.iter_mut().find(|a| a.query == *query) {
                a.taken = true;
            }
            return;
        }
        d.asked.retain(|a| a.query != *query);
        d.refused.retain(|(q, _)| q != query);
        d.refused.push((query.clone(), answer.clone()));
    }

    /// The desks a page of the directories reads, while it is on screen.
    #[must_use]
    pub(crate) fn directory_desks(&self) -> Vec<DeskKind> {
        let top = self.top();
        queries_of(top.page, top.query.as_ref())
            .iter()
            .map(Query::desk)
            .collect()
    }

    /// The lists asked that have been heard of — a page of each is here —
    /// forgotten: from now on the daemon's state says whether it is busy. And
    /// one another door's ask REPLACED on its desk — taken, the desk moved
    /// past it, idle, holding another list — forgotten too: its page says
    /// nothing is listed, with a row that asks again, and never "asking"
    /// forever.
    pub(crate) fn heard_the_lists(&mut self, state: Option<&State>) {
        let found = &self.found;
        for a in &mut self.directories.asked {
            if a.taken && a.seen_at.is_none() {
                a.seen_at = state
                    .and_then(|s| crate::player_find::summary(s, a.query.desk()))
                    .map(|d| d.generation);
            }
        }
        self.directories.asked.retain(|a| {
            if found.listing(&a.query).is_some() {
                return false;
            }
            let summary = state.and_then(|s| crate::player_find::summary(s, a.query.desk()));
            let replaced = a.taken
                && summary.is_some_and(|now| {
                    !now.busy
                        && a.seen_at.is_some_and(|then| now.generation > then)
                        && found.desk(a.query.desk()).is_some_and(|held| {
                            held.generation == now.generation
                                && held.query.as_ref() != Some(&a.query)
                        })
                });
            !replaced
        });
    }

    /// Every list on this door's stack: what the kept pages are kept for.
    #[must_use]
    pub(crate) fn listed_on_the_stack(&self) -> Vec<Query> {
        self.levels
            .iter()
            .flat_map(|l| queries_of(l.page, l.query.as_ref()))
            .collect()
    }

    /// A page of the directories opened on `query` — a country's stations, a
    /// genre's, a language's, a country's channels — from the row under the
    /// cursor.
    pub(crate) fn open_listing(
        &mut self,
        query: Query,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let page = match query {
            Query::Tv(_) => PageId::TvChannels,
            _ => PageId::Stations,
        };
        let from = self.spot_here(state, columns, texts);
        self.push_listing(page, query, from);
        self.settle_in(state, columns, texts);
    }

    /// `/` — the page's search: stations on the world radio and a list of
    /// them, channels on the world TV and a list of them, films on a theme
    /// with films. Asked on the door's line.
    pub(crate) fn search_here(&mut self) -> Errand {
        // THE GENRES AND THE LANGUAGES are searched by a name of their own:
        // the list holds the ones with the most stations, and any other is
        // reached by typing it.
        let named = match self.page() {
            PageId::RadioGenres => Some(crate::player_question::SEARCH_GENRE),
            PageId::RadioLanguages => Some(crate::player_question::SEARCH_LANGUAGE),
            _ => None,
        };
        if let Some(id) = named {
            self.typing = Some(Typing::Search(DeskKind::Radio));
            return Errand::Ask(crate::player_question::Question::plain(id, false));
        }
        let kind = match self.page() {
            PageId::Radio | PageId::Stations | PageId::RadioGenres | PageId::RadioLanguages => {
                DeskKind::Radio
            }
            PageId::Tv | PageId::TvChannels => DeskKind::Television,
            PageId::Theme(c) if !c.archive.is_empty() => DeskKind::Archive,
            _ => return Errand::Nothing,
        };
        self.search_asked(kind)
    }

    /// The search of `kind`'s desk, asked on the door's line.
    fn search_asked(&mut self, kind: DeskKind) -> Errand {
        let Some(id) = question_of(kind) else {
            return Errand::Nothing;
        };
        self.typing = Some(Typing::Search(kind));
        Errand::Ask(crate::player_question::Question::plain(id, false))
    }

    /// What was typed for a directory's search: the list of what it names —
    /// stations or channels on a page of their own, films on the theme's
    /// page, from its first — and asked. Empty: never mind.
    pub(crate) fn searched(
        &mut self,
        id: &str,
        said: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        self.typing = None;
        let words = said.trim();
        if words.is_empty() {
            return Errand::Nothing;
        }
        // A GENRE, A LANGUAGE, by its name: its stations, on their page.
        // radio-browser writes both in lowercase.
        let named = match id {
            crate::player_question::SEARCH_GENRE => Some(RadioList::Tag {
                tag: words.to_lowercase(),
                page: 1,
            }),
            crate::player_question::SEARCH_LANGUAGE => Some(RadioList::Language {
                language: words.to_lowercase(),
                page: 1,
            }),
            _ => None,
        };
        if let Some(list) = named {
            self.open_listing(Query::Radio(list), state, columns, texts);
            return Errand::Nothing;
        }
        let Some(kind) = desk_of_question(id) else {
            return Errand::Nothing;
        };
        match kind {
            DeskKind::Radio => self.open_listing(
                Query::Radio(RadioList::Search {
                    words: words.to_owned(),
                    page: 1,
                }),
                state,
                columns,
                texts,
            ),
            DeskKind::Television => self.open_listing(
                Query::Tv(TvList::Search {
                    words: words.to_owned(),
                }),
                state,
                columns,
                texts,
            ),
            _ => {
                let PageId::Theme(c) = self.page() else {
                    return Errand::Nothing;
                };
                let films = Query::Archive {
                    category: c.id.to_owned(),
                    words: Some(words.to_owned()),
                    page: 1,
                };
                self.top_mut().query = Some(films.clone());
                // ONLY THE FILMS are asked again: the theme's live TV and
                // lives are the same lists they were.
                self.directories.owed = vec![films];
            }
        }
        Errand::Nothing
    }

    /// What a press does on a row of the directories — `None` for a target
    /// that is none of theirs.
    pub(crate) fn directory_turned(
        &mut self,
        target: &Target,
        press: crate::player::Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        use crate::player::Press;
        match (target, press) {
            // A LIST ASKED: the press allows its directory, then asks — the
            // row names the host. "Try again" after a refusal is the same.
            (Target::Asks(query), Press::Enter) => Some(self.asking_list(query, texts)),
            // A LIST OPENED on its own page.
            (Target::Opens(query), Press::Enter) => {
                self.open_listing(query.clone(), state, columns, texts);
                Some(Errand::Nothing)
            }
            (Target::SearchDirectory(kind), Press::Enter) => Some(self.search_asked(*kind)),
            // EVERY ROW BACK, where a group of the globe narrowed them.
            (Target::EveryRow, Press::Enter) => {
                self.top_mut().only = None;
                Some(Errand::Nothing)
            }
            // A LIST OF STATIONS' NEXT OR EARLIER PAGE: the level's query
            // at that page, asked.
            (Target::StationsPage(number), Press::Enter) => {
                let top = self.top_mut();
                if let Some(Query::Radio(list)) = top.query.as_mut() {
                    *list = list.at_page(*number);
                }
                top.only = None;
                top.cursor = 0;
                top.moved = false;
                self.directories.owed = top.query.iter().cloned().collect();
                Some(Errand::Nothing)
            }
            // A THEME'S NEXT OR EARLIER PAGE OF FILMS.
            (Target::FilmsPage(number), Press::Enter) => {
                let top = self.top_mut();
                if let Some(Query::Archive { page, .. }) = top.query.as_mut() {
                    *page = (*number).max(1);
                }
                self.directories.owed = top.query.iter().cloned().collect();
                Some(Errand::Nothing)
            }
            // THE STAR, and `b`: kept, or taken off.
            (Target::Star(kind, id), Press::Enter) | (Target::Pick(kind, id), Press::Favourite) => {
                Some(self.starred(*kind, id, texts))
            }
            // `d` ON A FAVOURITE, on its page: taken off.
            (Target::Pick(kind, id), Press::Remove) if self.page() == PageId::Favourites => {
                Some(self.starred(*kind, id, texts))
            }
            (Target::Pick(kind, id), Press::Enter | Press::PlayNow | Press::Add) => {
                let (name, stream) = self.pick_of(state, *kind, id)?;
                Some(put(stream, &name, press != Press::Add, texts))
            }
            _ => None,
        }
    }

    /// A press on a row that names a directory's host: allowed, then asked.
    fn asking_list(&mut self, query: &Query, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(d) = query.directory() else {
            return Errand::Nothing;
        };
        self.directories.asking(query);
        let ask = Errand::TellSaying(
            Ask::Find {
                query: query.clone(),
            },
            texts.t_with(key::PLAYER_DESK_ASKING, &[("hosts", d.host())]),
        );
        if self.prefs.allows_directory(d) {
            return ask;
        }
        self.prefs.let_directory(d);
        Errand::Let {
            services: Vec::new(),
            directories: vec![d],
            on: true,
            then: Box::new(ask),
        }
    }

    /// The star of `(kind, id)` turned: kept among the favourites, or taken
    /// off — the daemon's to write.
    fn starred(&self, kind: Kind, id: &str, texts: &ronoctua_i18n::Texts) -> Errand {
        let on = !self.directories.picks.starred(kind, id);
        Errand::TellSaying(
            Ask::Favourite {
                kind,
                id: id.to_owned(),
                on,
            },
            texts.t(if on {
                key::PLAYER_FAVOURITE_ADDED
            } else {
                key::PLAYER_FAVOURITE_REMOVED
            }),
        )
    }

    /// What `(kind, id)` is called and what plays it: from a list on the
    /// page on screen, else the favourites, else the played lately.
    fn pick_of(&self, state: Option<&State>, kind: Kind, id: &str) -> Option<(String, Stream)> {
        let _ = state;
        let top = self.top();
        let listed = queries_of(top.page, top.query.as_ref())
            .iter()
            .filter_map(|q| self.found.listing(q))
            .find_map(|d| d.page.pick(kind, id))
            .map(|(name, _, stream)| (name, stream));
        listed.or_else(|| {
            let picks = &self.directories.picks;
            picks
                .favourites()
                .iter()
                .chain(picks.recent())
                .find(|p| p.kind == kind && p.id == id)
                .map(|p| (p.title.clone(), p.stream.clone()))
        })
    }

    /// The rows of a page of the directories, `page` on top.
    pub(crate) fn directory_rows(&self, page: PageId, s: &State, b: &mut Builder<'_>) {
        match page {
            PageId::Radio => self.world_page(s, DeskKind::Radio, b),
            PageId::Tv => self.world_page(s, DeskKind::Television, b),
            PageId::RadioGenres | PageId::RadioLanguages => self.named_page(s, b),
            PageId::Stations | PageId::TvChannels => self.list_page(s, b),
            PageId::Channels => themes(b),
            PageId::Theme(c) => self.theme_page(c, s, b),
            PageId::Favourites => self.picks_page(true, b),
            PageId::Recent => self.picks_page(false, b),
            _ => {}
        }
    }

    /// The title of a list's page: what it lists — a country, a genre, a
    /// language, what was searched for. The page's own key while it lists
    /// nothing yet.
    #[must_use]
    pub(crate) fn listing_title(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let top = self.top();
        if !matches!(top.page, PageId::Stations | PageId::TvChannels) {
            return None;
        }
        Some(match top.query.as_ref()? {
            Query::Radio(RadioList::Country { country, .. })
            | Query::Tv(TvList::Country { country }) => {
                crate::player_globe::country_name(*country, texts)
            }
            Query::Radio(RadioList::Tag { tag, .. }) => tag.clone(),
            Query::Radio(RadioList::Language { language, .. }) => self
                .language_named(language, texts)
                .unwrap_or_else(|| language.clone()),
            Query::Radio(RadioList::Search { words, .. }) | Query::Tv(TvList::Search { words }) => {
                texts.t_with(key::PLAYER_SEARCHED_TITLE, &[("words", words)])
            }
            Query::Tv(TvList::Placeless) => texts.t(key::PLAYER_GLOBE_NO_PLACE),
            _ => return None,
        })
    }

    /// A radio language by its name in the person's language, when the
    /// languages' page named it so.
    fn language_named(&self, language: &str, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let desk = self.found.listing(&Query::Radio(RadioList::Languages))?;
        let Page::Languages { languages, .. } = &desk.page else {
            return None;
        };
        let row = languages.iter().find(|r| r.name == language)?;
        let tongue = row.tongue.as_deref()?;
        Some(crate::player_find::tongue_name(tongue, texts))
    }

    /// How many stations the list on screen holds in all, as the page that
    /// opened it counted them — a country's, a genre's, a language's — when
    /// that page is here.
    fn stations_in_all(&self, query: &Query) -> Option<u32> {
        let counted = |list: RadioList| self.found.listing(&Query::Radio(list));
        match query {
            Query::Radio(RadioList::Country { country, .. }) => {
                match &counted(RadioList::Countries)?.page {
                    Page::RadioCountries { countries, .. } => countries
                        .iter()
                        .find(|r| r.country == *country)
                        .map(|r| r.count),
                    _ => None,
                }
            }
            Query::Radio(RadioList::Tag { tag, .. }) => match &counted(RadioList::Tags)?.page {
                Page::Genres { genres, .. } => {
                    genres.iter().find(|r| r.name == *tag).map(|r| r.stations)
                }
                _ => None,
            },
            Query::Radio(RadioList::Language { language, .. }) => {
                match &counted(RadioList::Languages)?.page {
                    Page::Languages { languages, .. } => languages
                        .iter()
                        .find(|r| r.name == *language)
                        .map(|r| r.stations),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// THE STATUS OF A LIST, over its rows — and the list, when the page may
    /// show one: while its directory is not allowed, the row that names the
    /// host and asks; while it is asked, that; a refusal in its sentence and
    /// the row that tries again; and when the list was made.
    fn status<'v>(&'v self, s: &State, query: &Query, b: &mut Builder<'_>) -> Option<&'v Desk> {
        let d = query.directory()?;
        let desk = self.found.listing(query);
        if !self.prefs.allows_directory(d) {
            b.pair_under(
                b.texts
                    .t_with(key::PLAYER_CONSENT_ASK, &[("hosts", d.host())]),
                String::new(),
                &b.t(key::PLAYER_CONSENT_FIRST),
                Target::Asks(query.clone()),
                &format!("ask-{}", spot_word(d)),
            );
            return desk;
        }
        // THE DESK IS ON THIS LIST — the held page is this one — and busy.
        let live = self
            .found
            .desk(query.desk())
            .is_some_and(|h| h.query.as_ref() == Some(query));
        // BUSY by the daemon's word — or by this door's press, while nothing
        // of the list is here yet.
        let busy = (live && crate::player_find::summary(s, query.desk()).is_some_and(|d| d.busy))
            || (desk.is_none() && self.directories.asked(query));
        let listed = desk.and_then(|d| listed_of(&d.page));
        if busy {
            if listed.is_some_and(|l| l.stale) {
                b.note(key::PLAYER_LISTED_UPDATING);
            } else {
                let said = b
                    .texts
                    .t_with(key::PLAYER_DESK_ASKING, &[("hosts", d.host())]);
                crate::player_find::note_said(&said, b);
            }
        }
        // REFUSED AT ONCE — the ask never reached its desk, so no page of it
        // says so: said here, and a row that tries again.
        if !busy && let Some(told) = self.directories.refusal(query) {
            if let Some(said) = crate::player::told_sentence(told, crate::player::older(s), b.texts)
            {
                b.warning(&said);
            }
            let hopeless = matches!(
                told,
                crate::player::Told::Refused(t) if hopeless(t)
            );
            if !hopeless {
                self.again_row(query, d, b);
            }
            desk?;
        }
        let Some(desk) = desk else {
            if !busy {
                b.note(key::PLAYER_LISTING_NOTHING);
                self.again_row(query, d, b);
            }
            return None;
        };
        if let Some(t) = desk.trouble.as_ref().filter(|_| !busy) {
            b.warning(&crate::player::trouble_sentence(t, b.texts));
            if !hopeless(t) {
                self.again_row(query, d, b);
            }
        }
        for t in &desk.refused {
            b.warning(&crate::player::trouble_sentence(t, b.texts));
        }
        if let Some(at) = listed.and_then(|l| l.fetched_unix_seconds) {
            let zone = self
                .directories
                .zone
                .clone()
                .unwrap_or_else(ronoctua_files::when::Zone::utc);
            let date = crate::chrome::when_text(at, &zone, false, b.texts);
            let said = b.texts.t_with(key::PLAYER_LISTED_AT, &[("date", &date)]);
            crate::player_find::note_said(&said, b);
        }
        Some(desk)
    }

    /// The row that asks `query` again — after a refusal, or a list that
    /// never came.
    fn again_row(&self, query: &Query, d: Directory, b: &mut Builder<'_>) {
        let _ = self;
        b.pair(
            b.t(key::PLAYER_LISTING_AGAIN),
            String::new(),
            Target::Asks(query.clone()),
            &format!("again-{}", spot_word(d)),
        );
    }

    /// The favourites' and the played lately's rows on a world page — each
    /// the page's own key, the count its value.
    fn picks_rows(&self, b: &mut Builder<'_>) {
        let picks = &self.directories.picks;
        for (page, n) in [
            (PageId::Favourites, picks.favourites().len()),
            (PageId::Recent, picks.recent().len()),
        ] {
            b.pair(
                b.t(page.key()),
                b.texts.t_args(
                    key::PLAYER_META_COUNT,
                    &[("count", Arg::Number(i64::try_from(n).unwrap_or(i64::MAX)))],
                ),
                Target::Go(page),
                page.spot(),
            );
        }
    }

    /// A row that narrows nothing any more, while a group of the globe's
    /// marks narrows the rows: how many are shown of how many, and every one
    /// back on a press.
    fn every_row(&self, shown: usize, total: usize, b: &mut Builder<'_>) {
        if self.top().only.is_none() {
            return;
        }
        let n = |x: usize| Arg::Number(i64::try_from(x).unwrap_or(i64::MAX));
        b.pair(
            b.texts.t_args(
                key::PLAYER_LISTING_EVERY,
                &[("shown", n(shown)), ("total", n(total))],
            ),
            String::new(),
            Target::EveryRow,
            "every-row",
        );
    }

    /// WORLD RADIO, WORLD TV: the globe, what it is, the directory's consent
    /// or its status, a search by name, the favourites and the played lately
    /// (on the radio's also the genres and the languages), and a row a
    /// country — its name in the person's language, how many — and how many
    /// have no place on the map.
    fn world_page(&self, s: &State, kind: DeskKind, b: &mut Builder<'_>) {
        let radio = kind == DeskKind::Radio;
        crate::player_globe::rows(self, s, b);
        b.note(if radio {
            key::PLAYER_ABOUT_RADIO
        } else {
            key::PLAYER_ABOUT_TV
        });
        let query = if radio {
            Query::Radio(RadioList::Countries)
        } else {
            Query::Tv(TvList::Countries)
        };
        b.pair(
            b.t(if radio {
                key::PLAYER_RADIO_SEARCH
            } else {
                key::PLAYER_TV_SEARCH
            }),
            String::new(),
            Target::SearchDirectory(kind),
            "search",
        );
        self.picks_rows(b);
        if radio {
            for page in [PageId::RadioGenres, PageId::RadioLanguages] {
                b.pair(
                    b.t(page.key()),
                    String::new(),
                    Target::Go(page),
                    page.spot(),
                );
            }
        }
        b.heading(key::PLAYER_HEADING_COUNTRIES);
        let Some(desk) = self.status(s, &query, b) else {
            return;
        };
        let (Page::RadioCountries {
            countries, globe, ..
        }
        | Page::TvCountries {
            countries, globe, ..
        }) = &desk.page
        else {
            return;
        };
        let only = self.top().only.as_deref();
        let shown: Vec<_> = countries
            .iter()
            .filter(|r| crate::player_globe::kept(only, r.mark))
            .collect();
        self.every_row(shown.len(), countries.len(), b);
        let country_row = |r: &ronoctua_player::find::CountryRow, b: &mut Builder<'_>| {
            let opens = if radio {
                Query::Radio(RadioList::Country {
                    country: r.country,
                    page: 1,
                })
            } else {
                Query::Tv(TvList::Country { country: r.country })
            };
            b.pair(
                crate::player_globe::country_name(r.country, b.texts),
                crate::player_globe::counted(kind, r.count, b.texts),
                Target::Opens(opens),
                &format!("country-{}", r.country.as_str()),
            );
        };
        // THE COUNTRIES THE MAP HAS A POINT FOR, then — under a heading of
        // their own, each a row that opens — the ones it has none for: what
        // "no place on the map" holds, open, never a count nobody can reach.
        for r in shown.iter().filter(|r| r.mark.is_some()) {
            country_row(r, b);
        }
        if only.is_some() {
            return;
        }
        let placeless: Vec<_> = shown.iter().filter(|r| r.mark.is_none()).collect();
        let in_countries: u32 = placeless.iter().map(|r| r.count).sum();
        let nowhere = globe.placeless.saturating_sub(in_countries);
        if placeless.is_empty() && nowhere == 0 {
            return;
        }
        b.heading_within(b.texts.t_args(
            key::PLAYER_GLOBE_PLACELESS,
            &[("count", Arg::Number(i64::from(globe.placeless)))],
        ));
        for r in placeless {
            country_row(r, b);
        }
        // CHANNELS OF NO COUNTRY at all, on a list of their own.
        if !radio && nowhere > 0 {
            b.pair(
                b.t(key::PLAYER_GLOBE_NO_COUNTRY),
                crate::player_globe::counted(kind, nowhere, b.texts),
                Target::Opens(Query::Tv(TvList::Placeless)),
                "placeless",
            );
        }
    }

    /// THE GENRES, or THE LANGUAGES, of radio: a row each, how many stations,
    /// opening their list. A language the table knows is named in the
    /// person's language; the others as radio-browser writes them.
    fn named_page(&self, s: &State, b: &mut Builder<'_>) {
        let Some(query) = self.top().query.clone() else {
            return;
        };
        let languages = query == Query::Radio(RadioList::Languages);
        // THE LIST HOLDS THE ONES WITH THE MOST STATIONS; any other is
        // reached by its name.
        b.note(if languages {
            key::PLAYER_ABOUT_RADIO_LANGUAGES
        } else {
            key::PLAYER_ABOUT_RADIO_GENRES
        });
        b.pair(
            b.t(if languages {
                key::PLAYER_RADIO_LANGUAGE_SEARCH
            } else {
                key::PLAYER_RADIO_GENRE_SEARCH
            }),
            String::new(),
            Target::SearchDirectory(DeskKind::Radio),
            "search",
        );
        let Some(desk) = self.status(s, &query, b) else {
            return;
        };
        let (rows, language) = match &desk.page {
            Page::Genres { genres, .. } => (genres, false),
            Page::Languages { languages, .. } => (languages, true),
            _ => return,
        };
        for r in rows {
            let name = r.tongue.as_deref().map_or_else(
                || r.name.clone(),
                |t| crate::player_find::tongue_name(t, b.texts),
            );
            let opens = if language {
                Query::Radio(RadioList::Language {
                    language: r.name.clone(),
                    page: 1,
                })
            } else {
                Query::Radio(RadioList::Tag {
                    tag: r.name.clone(),
                    page: 1,
                })
            };
            b.pair(
                name,
                crate::player_globe::counted(DeskKind::Radio, r.stations, b.texts),
                Target::Opens(opens),
                &format!("named-{}", r.name),
            );
        }
    }

    /// A LIST OF STATIONS OR OF CHANNELS: the globe, the status, and a row
    /// each — its star, its name, what it is — narrowed to a group of the
    /// globe's marks when one was clicked twice.
    fn list_page(&self, s: &State, b: &mut Builder<'_>) {
        let Some(query) = self.top().query.clone() else {
            return;
        };
        crate::player_globe::rows(self, s, b);
        let Some(desk) = self.status(s, &query, b) else {
            return;
        };
        // A COUNTRY'S LIST says no country on each row; another says it.
        let of_a_country = matches!(
            query,
            Query::Radio(RadioList::Country { .. }) | Query::Tv(TvList::Country { .. })
        );
        // WHEN A SEARCH SEARCHED, what it went out with: a search's list of
        // stations has a page of its own and no other title.
        let only = self.top().only.clone();
        match &desk.page {
            Page::Stations { stations, .. } => {
                self.stations_said(&query, stations.len(), b);
                let shown: Vec<_> = stations
                    .iter()
                    .filter(|r| crate::player_globe::kept(only.as_deref(), r.mark))
                    .collect();
                self.every_row(shown.len(), stations.len(), b);
                for r in shown {
                    self.station_row(s, r, of_a_country, b);
                }
                self.stations_pages(&query, stations.len(), b);
            }
            Page::Channels {
                channels, total, ..
            } => {
                let shown: Vec<_> = channels
                    .iter()
                    .filter(|r| crate::player_globe::kept(only.as_deref(), r.mark))
                    .collect();
                self.every_row(shown.len(), channels.len(), b);
                shown_of_total(channels.len(), *total, b);
                for r in shown {
                    self.channel_row(s, r, of_a_country, b);
                }
            }
            _ => {}
        }
    }

    /// A station's row: its star, its name, and under it its codec and
    /// bitrate — and its country, on a list of several. A stream that did
    /// not answer this session says so.
    fn station_row(
        &self,
        s: &State,
        r: &ronoctua_player::find::StationRow,
        of_a_country: bool,
        b: &mut Builder<'_>,
    ) {
        let st = &r.station;
        let mut value: Vec<String> = Vec::new();
        if let Some(codec) = st.codec.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
            value.push(codec.to_owned());
        }
        if st.hls {
            value.push(b.t(key::PLAYER_RADIO_HLS));
        } else if st.bitrate_kbps > 0 {
            value.push(b.texts.t_args(
                key::PLAYER_RADIO_BITRATE,
                &[("kbps", Arg::Number(i64::from(st.bitrate_kbps)))],
            ));
        }
        if !of_a_country && let Some(c) = st.country {
            value.push(crate::player_globe::country_name(c, b.texts));
        }
        let silent = s.unanswered.contains(&r.stream.url);
        if silent {
            value.push(b.t(key::PLAYER_STREAM_SILENT));
        }
        let first = b.len();
        self.pick_row(
            Kind::Radio,
            &st.uuid,
            st.name.clone(),
            &value.join(&b.t(key::PLAYER_META_JOINER)),
            b,
        );
        // A STATION THAT DID NOT ANSWER is dimmed, and still plays on a
        // press — the next try may answer.
        if silent {
            b.dim_from(first);
        }
    }

    /// A channel's row: its star, its name, and under it the height of its
    /// picture, whether it plays only in some countries or not always, the
    /// places its feeds name, and its country on a list of several.
    fn channel_row(
        &self,
        s: &State,
        r: &ronoctua_player::find::ChannelRow,
        of_a_country: bool,
        b: &mut Builder<'_>,
    ) {
        let mut value: Vec<String> = Vec::new();
        if let Some(lines) = r.lines {
            value.push(b.texts.t_args(
                key::PLAYER_TV_LINES,
                &[("lines", Arg::Number(i64::from(lines)))],
            ));
        }
        if r.geo_blocked {
            value.push(b.t(key::PLAYER_TV_GEO_BLOCKED));
        }
        if r.sometimes_off {
            value.push(b.t(key::PLAYER_TV_SOMETIMES_OFF));
        }
        if !r.areas.is_empty() {
            value.push(r.areas.join(&b.t(key::PLAYER_LIST_JOINER)));
        }
        if !of_a_country && let Some(c) = r.country {
            value.push(crate::player_globe::country_name(c, b.texts));
        }
        let silent = s.unanswered.contains(&r.stream.url);
        if silent {
            value.push(b.t(key::PLAYER_STREAM_SILENT));
        }
        let first = b.len();
        self.pick_row(
            Kind::Tv,
            &r.id,
            r.name.clone(),
            &value.join(&b.t(key::PLAYER_META_JOINER)),
            b,
        );
        if silent {
            b.dim_from(first);
        }
    }

    /// Over a list of stations: how many it shows of how many there are, and
    /// which page — radio-browser answers a list a page of
    /// `table::STATIONS_MOST` at a time, the most played first, and a country
    /// has thousands. Said only when there is more than one page.
    fn stations_said(&self, query: &Query, shown: usize, b: &mut Builder<'_>) {
        let Query::Radio(list) = query else {
            return;
        };
        let page = list.page().unwrap_or(1);
        let most = ronoctua_broadcast::table::STATIONS_MOST;
        let full = u32::try_from(shown).unwrap_or(u32::MAX) >= most;
        if page <= 1 && !full {
            return;
        }
        let n = |x: u32| Arg::Number(i64::from(x));
        let said = match self.stations_in_all(query) {
            Some(total) => b.texts.t_args(
                key::PLAYER_STATIONS_PAGE_OF,
                &[
                    ("total", n(total)),
                    ("page", n(page)),
                    ("pages", n(total.div_ceil(most).max(page))),
                ],
            ),
            None => b.texts.t_args(
                key::PLAYER_STATIONS_PAGE,
                &[("most", n(most)), ("page", n(page))],
            ),
        };
        crate::player_find::note_said(&said, b);
    }

    /// Under a list of stations, the rows to the page before it and the one
    /// after — the next only while this one is full.
    fn stations_pages(&self, query: &Query, shown: usize, b: &mut Builder<'_>) {
        let _ = self;
        let Query::Radio(list) = query else {
            return;
        };
        let page = list.page().unwrap_or(1);
        if page > 1 {
            b.pair(
                b.t(key::PLAYER_STATIONS_EARLIER),
                String::new(),
                Target::StationsPage(page - 1),
                "stations-earlier",
            );
        }
        if u32::try_from(shown).unwrap_or(u32::MAX) >= ronoctua_broadcast::table::STATIONS_MOST {
            b.pair(
                b.t(key::PLAYER_STATIONS_MORE),
                String::new(),
                Target::StationsPage(page + 1),
                "stations-more",
            );
        }
    }

    /// A row that PLAYS: the star at its head — kept among the favourites or
    /// not — its name on the whole row, and under it, in the quiet register,
    /// what it is: a name is data as long as it is, and a column of values
    /// squeezed it to a stub in a narrow door, and cut what was said of it
    /// (the search results' shape, `player_find`).
    fn pick_row(&self, kind: Kind, id: &str, name: String, said: &str, b: &mut Builder<'_>) {
        let on = self.directories.picks.starred(kind, id);
        b.pair_with_star(
            (on, Target::Star(kind, id.to_owned())),
            name,
            String::new(),
            said,
            Target::Pick(kind, id.to_owned()),
            &format!("pick-{}-{id}", kind_word(kind)),
        );
    }

    /// A THEME: its live TV, its films on the Internet Archive — a search by
    /// title, the page's rows, its rights, the next page — and what is live on
    /// YouTube for it, each only where the theme has that source, each with
    /// its own consent and status.
    fn theme_page(&self, c: &'static Category, s: &State, b: &mut Builder<'_>) {
        b.note(key::PLAYER_ABOUT_THEME);
        if !c.iptv.is_empty() {
            b.heading(key::PLAYER_HEADING_LIVE_TV);
            let query = Query::Tv(TvList::Category {
                category: c.id.to_owned(),
            });
            if let Some(desk) = self.status(s, &query, b)
                && let Page::Channels {
                    channels, total, ..
                } = &desk.page
            {
                shown_of_total(channels.len(), *total, b);
                for r in channels {
                    self.channel_row(s, r, false, b);
                }
            }
        }
        if let Some(query) = self.top().query.clone() {
            b.heading(key::PLAYER_HEADING_ARCHIVE);
            b.pair(
                b.t(key::PLAYER_ARCHIVE_SEARCH),
                String::new(),
                Target::SearchDirectory(DeskKind::Archive),
                "search-archive",
            );
            if let Some(Query::Archive {
                words: Some(words), ..
            }) = self.top().query.as_ref()
            {
                let said = b
                    .texts
                    .t_with(key::PLAYER_DESK_SEARCHED_FOR, &[("words", words)]);
                crate::player_find::note_said(&said, b);
            }
            if let Some(desk) = self.status(s, &query, b)
                && let Page::Archive {
                    items,
                    total,
                    number,
                    more,
                    ..
                } = &desk.page
            {
                if !items.is_empty() {
                    let said = b.texts.t_args(
                        key::PLAYER_ARCHIVE_PAGE,
                        &[
                            ("number", Arg::Number(i64::from(*number))),
                            (
                                "total",
                                Arg::Number(i64::try_from(*total).unwrap_or(i64::MAX)),
                            ),
                        ],
                    );
                    crate::player_find::note_said(&said, b);
                }
                for r in items {
                    let item = &r.item;
                    let said: Vec<String> = item
                        .year
                        .map(|y| y.to_string())
                        .into_iter()
                        .chain(item.license_url.as_deref().map(|u| {
                            b.texts.t_with(
                                key::PLAYER_ARCHIVE_RIGHTS,
                                &[("licence", &licence_name(u, b.texts))],
                            )
                        }))
                        .collect();
                    self.pick_row(
                        Kind::Archive,
                        &item.identifier,
                        item.title.clone(),
                        &said.join(&b.t(key::PLAYER_META_JOINER)),
                        b,
                    );
                }
                if *number > 1 {
                    b.pair(
                        b.t(key::PLAYER_ARCHIVE_EARLIER),
                        String::new(),
                        Target::FilmsPage(number - 1),
                        "films-earlier",
                    );
                }
                if *more {
                    b.pair(
                        b.t(key::PLAYER_ARCHIVE_MORE),
                        String::new(),
                        Target::FilmsPage(number + 1),
                        "films-more",
                    );
                }
            }
        }
        if !c.lives.is_empty() {
            b.heading(key::PLAYER_HEADING_LIVES);
            let query = Query::Lives {
                category: c.id.to_owned(),
            };
            if let Some(desk) = self.status(s, &query, b)
                && let Page::Lives { lives, .. } = &desk.page
            {
                for r in lives {
                    let live = &r.live;
                    // A LIVE WITH NO TITLE is named by its channel, which is
                    // then not said twice.
                    let untitled = live.title.trim().is_empty();
                    let name = if untitled {
                        live.channel.clone().unwrap_or_else(|| live.id.clone())
                    } else {
                        live.title.clone()
                    };
                    let mut said: Vec<String> =
                        live.channel.iter().filter(|_| !untitled).cloned().collect();
                    if let Some(n) = live.watching {
                        said.push(b.texts.t_args(
                            key::PLAYER_LIVES_WATCHING,
                            &[("count", Arg::Number(i64::try_from(n).unwrap_or(i64::MAX)))],
                        ));
                    }
                    said.push(
                        b.texts
                            .t_with(key::PLAYER_LIVES_FOUND_BY, &[("words", &r.asked)]),
                    );
                    self.pick_row(
                        Kind::Live,
                        &live.id,
                        name,
                        &said.join(&b.t(key::PLAYER_META_JOINER)),
                        b,
                    );
                }
            }
        }
    }

    /// THE FAVOURITES, or THE PLAYED LATELY: by kind, each its star, its
    /// name and its country; nothing kept said so.
    fn picks_page(&self, favourites: bool, b: &mut Builder<'_>) {
        b.note(if favourites {
            key::PLAYER_ABOUT_FAVOURITES
        } else {
            key::PLAYER_ABOUT_RECENT
        });
        let picks = &self.directories.picks;
        let all = if favourites {
            picks.favourites()
        } else {
            picks.recent()
        };
        if all.is_empty() {
            b.note(if favourites {
                key::PLAYER_FAVOURITES_NONE
            } else {
                key::PLAYER_RECENT_NONE
            });
            return;
        }
        for kind in [Kind::Radio, Kind::Tv, Kind::Archive, Kind::Live] {
            let of: Vec<&Pick> = all.iter().filter(|p| p.kind == kind).collect();
            if of.is_empty() {
                continue;
            }
            b.heading(kind_key(kind));
            for p in of {
                let country = p
                    .country
                    .map(|c| crate::player_globe::country_name(c, b.texts))
                    .unwrap_or_default();
                self.pick_row(kind, &p.id, p.title.clone(), &country, b);
            }
        }
    }
}

/// A licence by its name, from the address a film's rights point at: the
/// public domain, or Creative Commons' licence and version (`CC BY-SA 4.0`)
/// — the address itself only for one neither of those. Pure.
fn licence_name(url: &str, texts: &ronoctua_i18n::Texts) -> String {
    let path = url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.");
    let Some(rest) = path.strip_prefix("creativecommons.org/") else {
        return url.trim().to_owned();
    };
    let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
    match parts.as_slice() {
        ["publicdomain", "zero", ..] => texts.t(key::PLAYER_LICENCE_CC0),
        ["publicdomain", "mark", ..] => texts.t(key::PLAYER_LICENCE_PUBLIC_DOMAIN),
        ["licenses", kind, rest @ ..]
            if kind
                .split('-')
                .all(|k| ["by", "sa", "nc", "nd"].contains(&k)) =>
        {
            let kind = kind.to_uppercase();
            match rest
                .first()
                .filter(|v| v.chars().all(|c| c.is_ascii_digit() || c == '.'))
            {
                Some(version) => texts.t_with(
                    key::PLAYER_LICENCE_CC_VERSION,
                    &[("kind", &kind), ("version", version)],
                ),
                None => texts.t_with(key::PLAYER_LICENCE_CC, &[("kind", &kind)]),
            }
        }
        _ => url.trim().to_owned(),
    }
}

/// Whether asking again could not answer `t`: a machine with no yt-dlp has
/// none to list the lives with, however often asked.
const fn hopeless(t: &ronoctua_player::wire::Trouble) -> bool {
    matches!(
        t,
        ronoctua_player::wire::Trouble::Listing {
            why: ronoctua_player::Listing::Absent,
            ..
        }
    )
}

/// When a list was made, on a directory's page.
const fn listed_of(page: &Page) -> Option<Listed> {
    match page {
        Page::RadioCountries { listed, .. }
        | Page::Stations { listed, .. }
        | Page::Genres { listed, .. }
        | Page::Languages { listed, .. }
        | Page::TvCountries { listed, .. }
        | Page::Channels { listed, .. }
        | Page::Archive { listed, .. }
        | Page::Lives { listed, .. } => Some(*listed),
        Page::Nothing | Page::Suggestions { .. } | Page::Subtitles { .. } | Page::Lyrics { .. } => {
            None
        }
    }
}

/// How many of the channels found a page shows, when the ceiling cut some.
fn shown_of_total(shown: usize, total: u32, b: &mut Builder<'_>) {
    let shown = u32::try_from(shown).unwrap_or(u32::MAX);
    if total > shown {
        let said = b.texts.t_args(
            key::PLAYER_TV_SHOWN,
            &[
                ("shown", Arg::Number(i64::from(shown))),
                ("total", Arg::Number(i64::from(total))),
            ],
        );
        crate::player_find::note_said(&said, b);
    }
}

/// CHANNELS BY THEME: a row a theme, each opening its page — in the table's
/// order (`assets/channels.nix`).
fn themes(b: &mut Builder<'_>) {
    b.note(key::PLAYER_ABOUT_CHANNELS);
    for c in ronoctua_broadcast::table::CATEGORIES {
        let page = PageId::Theme(c);
        b.pair(
            b.t(page.key()),
            String::new(),
            Target::Go(page),
            page.spot(),
        );
    }
}

/// A kind of pick's word in a row's spot.
const fn kind_word(kind: Kind) -> &'static str {
    match kind {
        Kind::Radio => "radio",
        Kind::Tv => "tv",
        Kind::Archive => "archive",
        Kind::Live => "live",
    }
}

/// A kind of pick's heading on the favourites' and the played lately's pages.
const fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Radio => key::PLAYER_PICKS_RADIO,
        Kind::Tv => key::PLAYER_PICKS_TV,
        Kind::Archive => key::PLAYER_PICKS_ARCHIVE,
        Kind::Live => key::PLAYER_PICKS_LIVE,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::player::Press;
    use crate::player_globe::tests::{countries_desk, stations_desk, with_radio};
    use ronoctua_player::find::Directory;
    use ronoctua_player::wire::DeskSummary;
    use ronoctua_tokens as tokens;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    fn row_of(rows: &[crate::player_rows::Row], target: &Target) -> Option<usize> {
        rows.iter().position(|r| r.target == *target && r.stops())
    }

    fn said(rows: &[crate::player_rows::Row]) -> String {
        rows.iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// FAILS IF: a directory's page asks the player anything before the
    /// person pressed the row that names its host — or the press does not
    /// allow that directory and then ask; or, once allowed, a page that comes
    /// up (opened, come back to) does not ask for its own list.
    #[test]
    fn nothing_is_asked_until_the_host_is_pressed_and_then_every_page_asks_its_list() {
        let t = texts();
        let s = with_radio(0);
        let mut v = View::blank(None);
        let opened = v.key(Press::Go(PageId::Radio), Some(&s), 80, &t);
        assert_eq!(opened, Errand::Nothing, "asked before any consent");
        let rows = v.rows(Some(&s), 80, &t);
        let countries = Query::Radio(RadioList::Countries);
        let at = row_of(&rows, &Target::Asks(countries.clone())).expect("the row that asks");
        assert!(
            rows[at].fields[0].text.contains("radio-browser.info"),
            "the row does not name the host"
        );
        v.clicked(at, &rows);
        let Errand::Let {
            directories,
            on,
            then,
            ..
        } = v.key(Press::Enter, Some(&s), 80, &t)
        else {
            panic!("the press did not allow the directory");
        };
        assert_eq!((directories, on), (vec![Directory::Radio], true));
        assert!(matches!(*then, Errand::TellSaying(Ask::Find { query }, _) if query == countries));
        // ALLOWED: a country's page asks for its stations as it opens, and
        // the world's page asks for its countries again when it comes back.
        v.found.given(countries_desk());
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let brazil = Query::Radio(RadioList::Country {
            country: ronoctua_broadcast::country::normal("BR").expect("BR"),
            page: 1,
        });
        let at = row_of(&rows, &Target::Opens(brazil.clone())).expect("Brazil's row");
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Find { query: brazil })
        );
        assert_eq!(v.page(), PageId::Stations);
        assert_eq!(
            v.key(Press::Escape, Some(&s), 80, &t),
            Errand::Tell(Ask::Find { query: countries })
        );
    }

    /// FAILS IF: another door that took the desk for another list takes this
    /// door's page with it — a page shows what its own list was, kept — or a
    /// page shows a list that is not its own.
    #[test]
    fn a_page_keeps_its_own_list_when_the_desk_moves_on() {
        let t = texts();
        let s = with_radio(3);
        let mut v = View::blank(None);
        v.prefs.let_directory(Directory::Radio);
        v.key(Press::Go(PageId::Radio), Some(&s), 80, &t);
        v.found.given(countries_desk());
        v.settle_in(Some(&s), 80, &t);
        assert!(
            said(&v.rows(Some(&s), 80, &t)).contains("Brazil")
                || said(&v.rows(Some(&s), 80, &t)).contains("BR")
        );
        // ANOTHER DOOR ASKED FOR BRAZIL'S STATIONS: the desk holds them now.
        v.found.given(stations_desk());
        v.settle_in(Some(&s), 80, &t);
        let now = said(&v.rows(Some(&s), 80, &t));
        assert!(
            now.contains("40 stations"),
            "the world page lost its countries:\n{now}"
        );
        assert!(
            !now.contains("Frevo FM"),
            "the world page shows a country's stations"
        );
    }

    /// FAILS IF: a station's row is not its star, its name and what it is;
    /// `RET` does not play it now and `a` does not add it; `b` and its star do
    /// not keep it among the favourites — or take it off when it is kept; or a
    /// stream that did not answer is not said.
    #[test]
    fn a_station_plays_is_queued_and_is_starred() {
        let t = texts();
        let mut s = with_radio(7);
        s.unanswered = vec!["https://radio.example/b".into()];
        let mut v = View::blank(None);
        v.prefs.let_directory(Directory::Radio);
        v.push_listing(
            PageId::Stations,
            Query::Radio(RadioList::Country {
                country: ronoctua_broadcast::country::normal("BR").expect("BR"),
                page: 1,
            }),
            None,
        );
        // THE LIST IT ASKS AS IT OPENS, sent (a door's key would carry it).
        let _ = v.owing(Errand::Nothing);
        v.found.given(stations_desk());
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let pick = Target::Pick(Kind::Radio, "a".into());
        let at = row_of(&rows, &pick).expect("the station's row");
        let row = &rows[at];
        assert_eq!(
            row.fields[0].text,
            tokens::icon::PLAYER_FAVOURITE_OFF.to_string()
        );
        assert!(row.fields.iter().any(|f| f.text == "Rádio Paulista"));
        // WHAT IT IS, under its name.
        assert!(
            rows[at + 1].continues
                && rows[at + 1]
                    .fields
                    .iter()
                    .any(|f| f.text.contains("MP3 · 128 kbps"))
        );
        assert!(
            said(&rows).contains("did not answer"),
            "the silent stream is not said"
        );
        // AND ITS ROW IS DIMMED — the cursor elsewhere.
        let silent = row_of(&rows, &Target::Pick(Kind::Radio, "b".into())).expect("the silent one");
        assert_ne!(v.cursor(), silent);
        assert!(
            rows[silent]
                .fields
                .iter()
                .all(|f| f.color == tokens::color::TEXT_DIM),
            "the station that did not answer is not dimmed"
        );
        assert!(
            rows[at]
                .fields
                .iter()
                .any(|f| f.color != tokens::color::TEXT_DIM),
            "a station that answers is dimmed"
        );
        // THE NAME CHOSEN, not the star at its head: a click there is the
        // row's.
        v.clicked_at(at, row.fields[1].at, &rows);
        let played = v.key(Press::Enter, Some(&s), 80, &t);
        let Errand::TellSaying(
            Ask::Put {
                play: true, what, ..
            },
            _,
        ) = played
        else {
            panic!("RET did not play it: {played:?}");
        };
        assert!(
            matches!(what.as_slice(), [Thing::Stream { stream }] if stream.url == "https://radio.example/a")
        );
        assert!(matches!(
            v.key(Press::Add, Some(&s), 80, &t),
            Errand::TellSaying(Ask::Put { play: false, .. }, _)
        ));
        assert!(matches!(
            v.key(Press::Favourite, Some(&s), 80, &t),
            Errand::TellSaying(Ask::Favourite { kind: Kind::Radio, ref id, on: true }, _) if id == "a"
        ));
        // THE STAR, clicked: a control, acting at once — and taking off one
        // kept.
        let pick = |p: &str| Pick {
            kind: Kind::Radio,
            id: p.into(),
            title: "Rádio Paulista".into(),
            country: None,
            stream: Stream::typed("https://radio.example/a"),
            at_unix_seconds: 1,
        };
        v.directories.picks.given(vec![pick("a")], Vec::new());
        let rows = v.rows(Some(&s), 80, &t);
        let at = row_of(&rows, &Target::Pick(Kind::Radio, "a".into())).expect("the row");
        assert_eq!(
            rows[at].fields[0].text,
            tokens::icon::PLAYER_FAVOURITE.to_string()
        );
        assert!(
            v.clicked_at(at, rows[at].cells[0].from, &rows),
            "the star is no control"
        );
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::TellSaying(Ask::Favourite { on: false, .. }, _)
        ));
    }

    /// FAILS IF: `/` does not ask the page's search on the door's line, or
    /// what is typed does not open the list of what it names — and ask it.
    #[test]
    fn a_search_is_typed_and_opens_its_list() {
        let t = texts();
        let s = with_radio(3);
        let mut v = View::blank(None);
        v.prefs.let_directory(Directory::Radio);
        v.key(Press::Go(PageId::Radio), Some(&s), 80, &t);
        let Errand::Ask(q) = v.key(Press::Search, Some(&s), 80, &t) else {
            panic!("`/` asked nothing");
        };
        assert_eq!(q.id, crate::player_question::SEARCH_RADIO);
        let mut pairing = crate::player_machines::Pairing::default();
        let answered = v.answered(q.id, "bossa", &mut pairing, Some(&s), 80, &t);
        let words = Query::Radio(RadioList::Search {
            words: "bossa".into(),
            page: 1,
        });
        assert_eq!(
            answered,
            Errand::Tell(Ask::Find {
                query: words.clone()
            })
        );
        assert_eq!(v.page(), PageId::Stations);
        assert_eq!(v.top().query, Some(words));
        assert_eq!(v.listing_title(&t).as_deref(), Some("Named “bossa”"));
    }

    /// FAILS IF: a list the player refused AT ONCE — never reaching its
    /// desk, so no page of it will come — leaves its page saying "asking"
    /// for ever: the refusal is said there, with a row that tries again
    /// (none for the lives without yt-dlp, which no try can answer, said by
    /// the theme's name and never its id); or an ask another door's replaced
    /// on the desk stays "asking" once the desk is idle on another list.
    #[test]
    fn a_list_refused_at_once_is_said_on_its_page_and_tried_again() {
        use crate::player::Told;
        use ronoctua_player::wire::Trouble;
        let t = texts();
        let s = with_radio(0);
        let mut v = View::blank(None);
        v.prefs.let_directory(Directory::Radio);
        let countries = Query::Radio(RadioList::Countries);
        assert_eq!(
            v.key(Press::Go(PageId::Radio), Some(&s), 80, &t),
            Errand::Tell(Ask::Find {
                query: countries.clone()
            })
        );
        let asking = t.t_with(
            key::PLAYER_DESK_ASKING,
            &[("hosts", Directory::Radio.host())],
        );
        assert!(said(&v.rows(Some(&s), 80, &t)).contains(&asking));
        // REFUSED: said on the page, and tried again from a row.
        let off = Trouble::Off {
            host: Directory::Radio.host().to_owned(),
        };
        assert!(v.told(&Told::Listing {
            query: countries.clone(),
            answer: Box::new(Told::Refused(off.clone())),
        }));
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let now = said(&rows);
        assert!(!now.contains(&asking), "still asking:\n{now}");
        assert!(
            now.contains(&crate::player::trouble_sentence(&off, &t)),
            "the refusal is not said:\n{now}"
        );
        let again =
            row_of(&rows, &Target::Asks(countries.clone())).expect("a row that tries again");
        v.grabbed(again);
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::TellSaying(Ask::Find { .. }, _)
        ));
        assert!(
            said(&v.rows(Some(&s), 80, &t)).contains(&asking),
            "asked again"
        );
        // TAKEN — and ANOTHER DOOR'S ASK REPLACED IT on the desk: once the desk
        // moved on and is idle with another list, the page is not "asking".
        assert!(v.told(&Told::Listing {
            query: countries.clone(),
            answer: Box::new(Told::Taken),
        }));
        v.settle_in(Some(&s), 80, &t);
        assert!(
            said(&v.rows(Some(&s), 80, &t)).contains(&asking),
            "taken is still asking"
        );
        let moved = with_radio(7);
        v.found.given(stations_desk());
        v.settle_in(Some(&moved), 80, &t);
        let rows = v.rows(Some(&moved), 80, &t);
        let now = said(&rows);
        assert!(
            !now.contains(&asking),
            "a replaced ask is asking for ever:\n{now}"
        );
        assert!(
            row_of(&rows, &Target::Asks(countries)).is_some(),
            "no row asks again"
        );
        // THE LIVES WITHOUT YT-DLP: said by the theme's name, never tried again.
        let news = ronoctua_broadcast::table::Category::of("news").expect("news");
        v.prefs.let_directory(Directory::Lives);
        v.key(Press::Go(PageId::Theme(news)), Some(&s), 80, &t);
        let lives = Query::Lives {
            category: "news".into(),
        };
        v.told(&Told::Listing {
            query: lives.clone(),
            answer: Box::new(Told::Refused(Trouble::Listing {
                what: "news".into(),
                why: ronoctua_player::Listing::Absent,
            })),
        });
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let now = said(&rows);
        assert!(
            now.contains(&t.t_with(
                key::PLAYER_TROUBLE_LISTING_ABSENT,
                &[("what", &t.t(news.key))]
            )),
            "the theme is not named:\n{now}"
        );
        assert!(!now.contains("news:"), "the theme's id leaks:\n{now}");
        assert!(
            row_of(&rows, &Target::Asks(lives)).is_none(),
            "tried again without yt-dlp"
        );
    }

    /// FAILS IF: a list of stations that holds a page of them — the most
    /// played — does not say so, and how many there are in all, or offers no
    /// way to the next page and back.
    #[test]
    fn a_long_list_of_stations_is_walked_a_page_at_a_time() {
        let t = texts();
        let s = with_radio(7);
        let mut v = View::blank(None);
        v.prefs.let_directory(Directory::Radio);
        let mut world = crate::player_globe::tests::countries_desk();
        if let Page::RadioCountries { countries, .. } = &mut world.page {
            countries[0].count = 2380;
        }
        // THE WORLD'S LIST COMES while its page is open, as it does.
        v.key(Press::Go(PageId::Radio), Some(&s), 80, &t);
        v.found.given(world);
        v.settle_in(Some(&s), 80, &t);
        let brazil = Query::Radio(RadioList::Country {
            country: ronoctua_broadcast::country::normal("BR").expect("BR"),
            page: 1,
        });
        let rows = v.rows(Some(&s), 80, &t);
        let at = row_of(&rows, &Target::Opens(brazil)).expect("Brazil's row");
        v.grabbed(at);
        v.key(Press::Enter, Some(&s), 80, &t);
        let mut desk = stations_desk();
        if let Page::Stations { stations, .. } = &mut desk.page {
            let one = stations[0].clone();
            let most = ronoctua_broadcast::table::STATIONS_MOST as usize;
            *stations = (0..most)
                .map(|n| {
                    let mut r = one.clone();
                    r.station.uuid = format!("{n}");
                    r.station.name = format!("Station {n}");
                    r
                })
                .collect();
        }
        v.found.given(desk);
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let now = said(&rows);
        let pages = i64::from(2380_u32.div_ceil(ronoctua_broadcast::table::STATIONS_MOST));
        let expected = t.t_args(
            key::PLAYER_STATIONS_PAGE_OF,
            &[
                ("total", Arg::Number(2380)),
                ("page", Arg::Number(1)),
                ("pages", Arg::Number(pages)),
            ],
        );
        assert!(now.contains(&expected), "not said:\n{expected}\nin\n{now}");
        let more = row_of(&rows, &Target::StationsPage(2)).expect("the next page");
        assert!(row_of(&rows, &Target::StationsPage(0)).is_none());
        v.grabbed(more);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Find {
                query: Query::Radio(RadioList::Country {
                    country: ronoctua_broadcast::country::normal("BR").expect("BR"),
                    page: 2,
                })
            })
        );
    }

    /// FAILS IF: what the world page counts as having no place on the map
    /// cannot be opened — the countries the map has no point for are rows
    /// under a heading of their own, and TV channels of no country a list.
    #[test]
    fn what_has_no_place_on_the_map_is_open() {
        let t = texts();
        let s = with_radio(3);
        let mut v = View::blank(None);
        v.prefs.let_directory(Directory::Radio);
        let mut world = crate::player_globe::tests::countries_desk();
        if let Page::RadioCountries {
            countries, globe, ..
        } = &mut world.page
        {
            countries.push(ronoctua_player::find::CountryRow {
                country: ronoctua_broadcast::country::normal("AD").expect("AD"),
                count: 2,
                mark: None,
            });
            globe.placeless = 2;
        }
        v.found.given(world);
        v.key(Press::Go(PageId::Radio), Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let andorra = Query::Radio(RadioList::Country {
            country: ronoctua_broadcast::country::normal("AD").expect("AD"),
            page: 1,
        });
        let heading = t.t_args(key::PLAYER_GLOBE_PLACELESS, &[("count", Arg::Number(2))]);
        let at_heading = rows
            .iter()
            .position(|r| r.heading && r.fields.iter().any(|f| f.text == heading))
            .expect("the heading of what has no place");
        let at = row_of(&rows, &Target::Opens(andorra)).expect("a country with no place opens");
        assert!(at > at_heading, "it is not under its heading");
        // TV: the channels of no country, a list of their own.
        let mut tv = crate::player_globe::tests::countries_desk();
        tv.kind = DeskKind::Television;
        tv.query = Some(Query::Tv(TvList::Countries));
        if let Page::RadioCountries {
            countries,
            globe,
            listed,
        } = tv.page
        {
            tv.page = Page::TvCountries {
                countries,
                globe: ronoctua_player::find::Globe {
                    placeless: 212,
                    ..globe
                },
                listed,
            };
        }
        v.prefs.let_directory(Directory::Iptv);
        v.found.given(tv);
        v.key(Press::Go(PageId::Tv), Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let at = row_of(&rows, &Target::Opens(Query::Tv(TvList::Placeless)))
            .expect("the channels of no country open");
        v.grabbed(at);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Find {
                query: Query::Tv(TvList::Placeless)
            })
        );
        assert_eq!(v.page(), PageId::TvChannels);
        assert_eq!(
            v.listing_title(&t).as_deref(),
            Some(t.t(key::PLAYER_GLOBE_NO_PLACE).as_str())
        );
        // AND NO GLOBE over what has no place on it.
        assert!(
            !v.rows(Some(&s), 80, &t).iter().any(|r| r.globe.is_some()),
            "a globe over the channels of no place"
        );
    }

    /// FAILS IF: a genre or a language past the list's most cannot be
    /// reached — its name typed opens its stations.
    #[test]
    fn any_genre_is_reached_by_its_name() {
        let t = texts();
        let s = with_radio(3);
        let mut v = View::blank(None);
        v.prefs.let_directory(Directory::Radio);
        v.key(Press::Go(PageId::RadioGenres), Some(&s), 80, &t);
        let Errand::Ask(q) = v.key(Press::Search, Some(&s), 80, &t) else {
            panic!("`/` asked nothing");
        };
        assert_eq!(q.id, crate::player_question::SEARCH_GENRE);
        let mut pairing = crate::player_machines::Pairing::default();
        let answered = v.answered(q.id, "Bossa Nova", &mut pairing, Some(&s), 80, &t);
        assert_eq!(
            answered,
            Errand::Tell(Ask::Find {
                query: Query::Radio(RadioList::Tag {
                    tag: "bossa nova".into(),
                    page: 1,
                })
            })
        );
        assert_eq!(v.page(), PageId::Stations);
    }

    /// FAILS IF: a film's rights are an address, not the licence by its name.
    #[test]
    fn a_licence_is_named() {
        let t = texts();
        assert_eq!(
            licence_name("https://creativecommons.org/licenses/by-sa/4.0/", &t),
            "CC BY-SA 4.0"
        );
        assert_eq!(
            licence_name("http://creativecommons.org/licenses/by-nc/3.0", &t),
            "CC BY-NC 3.0"
        );
        assert_eq!(
            licence_name("http://creativecommons.org/publicdomain/mark/1.0/", &t),
            t.t(key::PLAYER_LICENCE_PUBLIC_DOMAIN)
        );
        assert_eq!(
            licence_name("https://creativecommons.org/publicdomain/zero/1.0/", &t),
            t.t(key::PLAYER_LICENCE_CC0)
        );
        assert_eq!(
            licence_name("https://example.org/terms", &t),
            "https://example.org/terms"
        );
    }

    /// FAILS IF: a theme's page is not its three sources, each where the theme
    /// has it, each asked on its own — or its films' next page is not a row.
    #[test]
    fn a_theme_is_three_lists_each_asked_on_its_own() {
        let t = texts();
        let mut s = with_radio(0);
        s.desks.push(DeskSummary {
            kind: DeskKind::Archive,
            generation: 2,
            busy: false,
            trouble: None,
        });
        let news = Category::of("news").expect("news");
        let mut v = View::blank(None);
        for d in Directory::ALL {
            v.prefs.let_directory(d);
        }
        let opened = v.key(Press::Go(PageId::Theme(news)), Some(&s), 80, &t);
        let mut asked = Vec::new();
        let mut e = opened;
        loop {
            match e {
                Errand::Then(a, b) => {
                    asked.push(*b);
                    e = *a;
                }
                other => {
                    asked.push(other);
                    break;
                }
            }
        }
        let kinds: Vec<DeskKind> = asked
            .iter()
            .filter_map(|e| match e {
                Errand::Tell(Ask::Find { query }) => Some(query.desk()),
                _ => None,
            })
            .collect();
        for k in [DeskKind::Channels, DeskKind::Archive, DeskKind::Lives] {
            assert!(kinds.contains(&k), "{k:?} was not asked: {kinds:?}");
        }
        let archive = Desk {
            kind: DeskKind::Archive,
            generation: 2,
            busy: false,
            trouble: None,
            query: Some(Query::Archive {
                category: "news".into(),
                words: None,
                page: 1,
            }),
            asked: None,
            refused: Vec::new(),
            page: Page::Archive {
                category: "news".into(),
                items: vec![ronoctua_player::find::ArchiveRow {
                    item: ronoctua_broadcast::archive::Item {
                        identifier: "night".into(),
                        title: "The Night".into(),
                        year: Some(1950),
                        downloads: 9,
                        license_url: Some(
                            "https://creativecommons.org/publicdomain/mark/1.0/".into(),
                        ),
                    },
                    stream: Stream::typed("https://archive.org/details/night"),
                }],
                total: 60,
                number: 1,
                more: true,
                listed: Listed::default(),
            },
            taken: None,
        };
        v.found.given(archive);
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let text = said(&rows);
        assert!(text.contains("The Night"), "{text}");
        assert!(
            text.contains(&t.t(key::PLAYER_LICENCE_PUBLIC_DOMAIN)),
            "the rights are not named: {text}"
        );
        assert!(
            !text.contains("creativecommons.org"),
            "the rights are an address: {text}"
        );
        let more = row_of(&rows, &Target::FilmsPage(2)).expect("the next page");
        v.clicked(more, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Find {
                query: Query::Archive {
                    category: "news".into(),
                    words: None,
                    page: 2
                }
            })
        );
    }
}
