//! Looking things up on the network, as a door asks it and the daemon keeps
//! the answer: tags suggested for a file, subtitles for a film, lyrics for a
//! song.
//!
//! # The daemon asks, and keeps the page
//!
//! Every lookup is the daemon's ([`crate::wire::Ask::Find`]): MusicBrainz
//! takes one request a second from an ADDRESS, and three doors would be three
//! limiters, so the daemon is the one process that asks (one pace per host,
//! `ronoctua_lookup::pace`). What it found is a [`Desk`] — one per
//! [`DeskKind`] — that outlives the door that asked: a person who asked in the
//! footer and swapped to the window finds the same page. The state carries
//! only each desk's summary ([`crate::wire::DeskSummary`]); a door asks for
//! the page ([`crate::wire::Ask::Desk`]) when its generation moves.
//!
//! A new search REPLACES the one under way of its kind: what the old one
//! finds after that is dropped.
//!
//! # Consent is a press on a row that names the host
//!
//! Nothing here asks on its own. A service is asked only when the person
//! let it be ([`crate::prefs::Preferences::allows`]): the row that searches
//! names its hosts, and pressing it is the consent — the door records it
//! ([`crate::prefs::Preferences::consent`]) and then asks. A daemon asked
//! for a service the person did not let it ask refuses, naming the host
//! ([`crate::wire::Trouble::Off`]).
//!
//! # Taking one
//!
//! [`crate::wire::Ask::Take`] names a candidate by its [`CandidateId`] —
//! never reused, so a take aimed at a page a newer search replaced is
//! refused, not carried out on whatever now sits at that place.
//!
//! # And the directories
//!
//! The world's radio (radio-browser), the world's TV (iptv-org), the
//! Internet Archive's films and YouTube's lives are listed the same way:
//! a [`Query`] the daemon asks on a desk's thread, the page kept for every
//! door. A directory's page carries its rows AND what the globe draws of
//! them ([`Globe`]): a door draws, and never places a thing on the map
//! itself. Each row that plays carries the [`Stream`] the door puts in the
//! queue, its relays and its origin decided by the daemon.
//!
//! Each directory is let by a press on a row that names its host
//! ([`Directory::host`], [`crate::prefs::Preferences::let_directory`]), and
//! nothing is asked of one that is not.

use ronoctua_broadcast::{Code, Spot, archive, radio::Station};
use ronoctua_lookup::{Service, found::Found, lrclib::Record, suggestion::Suggestion};
use ronoctua_playlist::own::Stream;
use serde::{Deserialize, Serialize};

use crate::wire::{DeskKind, Trouble};

/// Who suggests a file's tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Suggester {
    /// A song: MusicBrainz, and its release's cover on the Cover Art
    /// Archive.
    #[serde(rename = "musicbrainz")]
    MusicBrainz,
    /// A film or a series, keyless: title, date, director, genre, and the
    /// poster when Commons has one.
    #[serde(rename = "wikidata")]
    Wikidata,
    /// A film or an episode, with the person's token: the synopsis and the
    /// poster too.
    #[serde(rename = "tmdb")]
    Tmdb,
}

impl Suggester {
    /// Every one, in the order the door offers them.
    pub const ALL: [Self; 3] = [Self::MusicBrainz, Self::Wikidata, Self::Tmdb];

    /// The services it asks — each named on its row, and each consented to
    /// by the press on it. MusicBrainz's covers are the Cover Art
    /// Archive's.
    #[must_use]
    pub const fn services(self) -> &'static [Service] {
        match self {
            Self::MusicBrainz => &[Service::MusicBrainz, Service::CoverArtArchive],
            Self::Wikidata => &[Service::Wikidata],
            Self::Tmdb => &[Service::Tmdb],
        }
    }
}

/// The services a subtitle search asks, each with the person's key; the
/// one without a key, or not let, is left out and said.
pub const SUBTITLE_SERVICES: [Service; 2] = [Service::OpenSubtitles, Service::SubDl];

/// What a door asks the daemon to look up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "what")]
pub enum Query {
    /// Tags for a file: its own tags (or `words`) asked of `source`.
    Suggest {
        /// The file (or the address that plays).
        location: String,
        /// Who is asked.
        source: Suggester,
        /// Words to ask with instead of the file's own title.
        #[serde(default)]
        words: Option<String>,
        /// The languages names are asked in (Wikidata's labels, TMDB's
        /// titles and synopsis), the interface's first; none is English.
        #[serde(default)]
        tongues: Vec<String>,
    },
    /// Subtitles for a film, in these languages
    /// ([`crate::tongues`] codes; none is English): by the file's
    /// fingerprint first, then by its name, or by `words`.
    Subtitles {
        /// The film (or the address that plays).
        location: String,
        /// The languages.
        #[serde(default)]
        tongues: Vec<String>,
        /// Words to ask with instead of the file's name and tags.
        #[serde(default)]
        words: Option<String>,
    },
    /// Lyrics for a song: by its title and artist, or by `words`.
    Lyrics {
        /// The song (or the address that plays).
        location: String,
        /// Words to ask with instead of the song's own tags.
        #[serde(default)]
        words: Option<String>,
    },
    /// The world's radio, from radio-browser ([`Directory::Radio`]).
    Radio(RadioList),
    /// The world's TV, from iptv-org's index ([`Directory::Iptv`]).
    Tv(TvList),
    /// A category's films on the Internet Archive ([`Directory::Archive`]),
    /// a page at a time, the most downloaded first.
    Archive {
        /// The category (`ronoctua_broadcast::table::CATEGORIES`).
        category: String,
        /// Words in the title, when the person typed some.
        #[serde(default)]
        words: Option<String>,
        /// Which page, from 1.
        #[serde(default = "first_page")]
        page: u32,
    },
    /// What is live on YouTube now for a category, through yt-dlp
    /// ([`Directory::Lives`]).
    Lives {
        /// The category.
        category: String,
    },
}

/// A search's first page, when a door does not say.
const fn first_page() -> u32 {
    1
}

/// What of the world's radio is listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "list")]
pub enum RadioList {
    /// Every country with stations, and how many — the globe's world view.
    Countries,
    /// A country's stations, the most played first.
    Country {
        /// The country (`UK` is read as `GB`).
        country: Code,
        /// Which page of them, from 1: a page holds the table's
        /// `STATIONS_MOST`, and a country has thousands.
        #[serde(default = "first_page")]
        page: u32,
    },
    /// The genres with the most stations ([`Page::Genres`]).
    Tags,
    /// The stations of a genre, exactly as the genre list names it.
    Tag {
        /// The genre.
        tag: String,
        /// Which page of them, from 1.
        #[serde(default = "first_page")]
        page: u32,
    },
    /// The languages with the most stations ([`Page::Languages`]).
    Languages,
    /// The stations in a language, exactly as the language list names it.
    Language {
        /// The language, radio-browser's word for it.
        language: String,
        /// Which page of them, from 1.
        #[serde(default = "first_page")]
        page: u32,
    },
    /// Stations whose name has these words — else stations of that genre.
    Search {
        /// What the person typed.
        words: String,
        /// Which page of them, from 1.
        #[serde(default = "first_page")]
        page: u32,
    },
}

impl RadioList {
    /// Which page of a list of stations this is, from 1 — `None` for a list
    /// that is not of stations (the countries, the genres, the languages).
    #[must_use]
    pub const fn page(&self) -> Option<u32> {
        match self {
            Self::Country { page, .. }
            | Self::Tag { page, .. }
            | Self::Language { page, .. }
            | Self::Search { page, .. } => Some(*page),
            Self::Countries | Self::Tags | Self::Languages => None,
        }
    }

    /// The same list of stations at page `to` (from 1); a list that is not
    /// of stations stays as it is.
    #[must_use]
    pub fn at_page(&self, to: u32) -> Self {
        let mut out = self.clone();
        if let Self::Country { page, .. }
        | Self::Tag { page, .. }
        | Self::Language { page, .. }
        | Self::Search { page, .. } = &mut out
        {
            *page = to.max(1);
        }
        out
    }
}

/// What of the world's TV is listed. Every answer comes from the index kept
/// in the cache: iptv-org is asked only when that is older than a day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "list")]
pub enum TvList {
    /// Every country with channels, and how many.
    Countries,
    /// A country's channels.
    Country {
        /// The country (`UK` is read as `GB`).
        country: Code,
    },
    /// A category's live channels — the TV of "channels by theme", on a desk
    /// of its own ([`DeskKind::Channels`]) so the world's TV stays where it
    /// was.
    Category {
        /// The category (`ronoctua_broadcast::table::CATEGORIES`).
        category: String,
    },
    /// Channels whose name has these words.
    Search {
        /// What the person typed.
        words: String,
    },
    /// The channels no place on the map holds: whose country the index does
    /// not name, or names one the map has no point for — the world page's
    /// "no place on the map".
    Placeless,
}

impl Query {
    /// The desk its answer goes to.
    #[must_use]
    pub const fn desk(&self) -> DeskKind {
        match self {
            Self::Suggest { .. } => DeskKind::Suggestions,
            Self::Subtitles { .. } => DeskKind::Subtitles,
            Self::Lyrics { .. } => DeskKind::Lyrics,
            Self::Radio(_) => DeskKind::Radio,
            Self::Tv(TvList::Category { .. }) => DeskKind::Channels,
            Self::Tv(_) => DeskKind::Television,
            Self::Archive { .. } => DeskKind::Archive,
            Self::Lives { .. } => DeskKind::Lives,
        }
    }

    /// What it is about: the file a lookup is for; empty for a directory's
    /// listing, which is about no file.
    #[must_use]
    pub fn location(&self) -> &str {
        match self {
            Self::Suggest { location, .. }
            | Self::Subtitles { location, .. }
            | Self::Lyrics { location, .. } => location,
            Self::Radio(_) | Self::Tv(_) | Self::Archive { .. } | Self::Lives { .. } => "",
        }
    }

    /// The services it may ask — none for a directory, whose consent is its
    /// own ([`Query::directory`]).
    #[must_use]
    pub fn services(&self) -> &'static [Service] {
        match self {
            Self::Suggest { source, .. } => source.services(),
            Self::Subtitles { .. } => &SUBTITLE_SERVICES,
            Self::Lyrics { .. } => &[Service::Lrclib],
            Self::Radio(_) | Self::Tv(_) | Self::Archive { .. } | Self::Lives { .. } => &[],
        }
    }

    /// The directory it lists from; `None` for a lookup about a file.
    #[must_use]
    pub const fn directory(&self) -> Option<Directory> {
        match self {
            Self::Suggest { .. } | Self::Subtitles { .. } | Self::Lyrics { .. } => None,
            Self::Radio(_) => Some(Directory::Radio),
            Self::Tv(_) => Some(Directory::Iptv),
            Self::Archive { .. } => Some(Directory::Archive),
            Self::Lives { .. } => Some(Directory::Lives),
        }
    }
}

/// A public directory the player lists from. Each is let apart, by a press on
/// a row that names its host, and born not let.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Directory {
    /// radio-browser: the world's radio stations.
    Radio,
    /// iptv-org: the world's TV channels.
    Iptv,
    /// The Internet Archive: films.
    Archive,
    /// YouTube, asked through yt-dlp for what is live now.
    Lives,
}

impl Directory {
    /// Every directory, in the order the Internet page lists them.
    pub const ALL: [Self; 4] = [Self::Radio, Self::Iptv, Self::Archive, Self::Lives];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Radio => "radio",
            Self::Iptv => "iptv",
            Self::Archive => "archive",
            Self::Lives => "lives",
        }
    }

    /// The host its row names, and a refusal of it ([`Trouble::Off`]).
    #[must_use]
    pub const fn host(self) -> &'static str {
        match self {
            Self::Radio => ronoctua_broadcast::Source::Radio.host(),
            Self::Iptv => ronoctua_broadcast::Source::Iptv.host(),
            Self::Archive => ronoctua_broadcast::Source::Archive.host(),
            Self::Lives => ronoctua_ytdlp::LIVES_HOST,
        }
    }

    /// The directory a refusal's host names.
    #[must_use]
    pub fn of_host(host: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|d| d.host().eq_ignore_ascii_case(host.trim()))
    }
}

/// A candidate of a desk, by a number the daemon never gives twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CandidateId(pub u64);

/// A page of results the daemon keeps ([`crate::wire::Ask::Desk`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Desk {
    /// Which desk.
    pub kind: DeskKind,
    /// Moves every time anything below changes.
    pub generation: u64,
    /// A search or a take is under way.
    pub busy: bool,
    /// Why the last search gave nothing, or the last take failed.
    pub trouble: Option<Trouble>,
    /// What the last search was asked, as the door asked it.
    pub query: Option<Query>,
    /// What the daemon asked the services with: the door's words, else what
    /// it made of the file's tags and name.
    pub asked: Option<Asked>,
    /// Each service that gave nothing while another answered, and why —
    /// a service without a key, one not let, one that did not answer.
    pub refused: Vec<Trouble>,
    /// What was found.
    pub page: Page,
    /// How the last take went.
    pub taken: Option<Taken>,
}

impl Desk {
    /// An empty desk of `kind`.
    #[must_use]
    pub const fn empty(kind: DeskKind) -> Self {
        Self {
            kind,
            generation: 0,
            busy: false,
            trouble: None,
            query: None,
            asked: None,
            refused: Vec::new(),
            page: Page::Nothing,
            taken: None,
        }
    }
}

/// The words a search went out with.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Asked {
    /// The title, or the person's words.
    pub words: String,
    /// The artist, for a song.
    pub artist: Option<String>,
    /// The album, for a song.
    pub album: Option<String>,
    /// The year, for a film.
    pub year: Option<u16>,
    /// The season, for an episode.
    pub season: Option<u16>,
    /// The episode.
    pub episode: Option<u16>,
    /// The file's fingerprint went with it (OpenSubtitles' moviehash): a
    /// match is timed for exactly this file.
    pub fingerprint: bool,
    /// It was looked up by an id the file carries (MusicBrainz's), not
    /// searched.
    pub by_id: bool,
}

/// What a desk holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "page")]
pub enum Page {
    /// Nothing asked yet (or nothing found).
    Nothing,
    /// Tags suggested for a file.
    Suggestions {
        /// The file.
        location: String,
        /// The candidates, the service's order.
        candidates: Vec<SuggestionCandidate>,
    },
    /// Subtitles found for a film.
    Subtitles {
        /// The film.
        location: String,
        /// The languages asked in ([`crate::tongues`] codes).
        tongues: Vec<String>,
        /// The candidates: by language, those matched by the fingerprint
        /// first, then the most downloaded.
        candidates: Vec<SubtitleCandidate>,
        /// OpenSubtitles' downloads left today, as it last said (after a
        /// login, after each download).
        remaining_downloads: Option<i64>,
    },
    /// Lyrics found for a song.
    Lyrics {
        /// The song.
        location: String,
        /// Its length, which each record's is measured against.
        duration_seconds: Option<f64>,
        /// The candidates, the service's order.
        candidates: Vec<LyricsCandidate>,
    },
    /// The countries with radio stations, the most first
    /// ([`RadioList::Countries`]).
    RadioCountries {
        /// One row per country.
        countries: Vec<CountryRow>,
        /// One mark per country, at its anchor.
        globe: Globe,
        /// When the list was made.
        listed: Listed,
    },
    /// Radio stations: a country's, a genre's, a language's, a search's.
    Stations {
        /// The most played first.
        stations: Vec<StationRow>,
        /// A station that says where it is is a mark of its own; the others
        /// share their country's anchor.
        globe: Globe,
        /// When the list was made.
        listed: Listed,
    },
    /// The genres of radio, the most stations first ([`RadioList::Tags`]).
    Genres {
        /// One row per genre.
        genres: Vec<NamedRow>,
        /// When the list was made.
        listed: Listed,
    },
    /// The languages of radio, the most stations first
    /// ([`RadioList::Languages`]).
    Languages {
        /// One row per language.
        languages: Vec<NamedRow>,
        /// When the list was made.
        listed: Listed,
    },
    /// The countries with TV channels, the most first ([`TvList::Countries`]).
    TvCountries {
        /// One row per country.
        countries: Vec<CountryRow>,
        /// One mark per country, at its anchor.
        globe: Globe,
        /// When the index was made.
        listed: Listed,
    },
    /// TV channels: a country's, a category's, a search's.
    Channels {
        /// The channels, by name — at most [`CHANNELS_MOST`].
        channels: Vec<ChannelRow>,
        /// How many there were before the ceiling.
        total: u32,
        /// A channel whose feed names a city the map knows is a mark there;
        /// the others share their country's anchor.
        globe: Globe,
        /// When the index was made.
        listed: Listed,
    },
    /// A page of a category's films on the Internet Archive.
    Archive {
        /// The category.
        category: String,
        /// The items of this page, the most downloaded first.
        items: Vec<ArchiveRow>,
        /// How many the search found in all.
        total: u64,
        /// Which page this is, from 1.
        number: u32,
        /// Whether there is a page after it.
        more: bool,
        /// When the page was asked.
        listed: Listed,
    },
    /// What is live on YouTube now for a category.
    Lives {
        /// The category.
        category: String,
        /// The lives, each once, in the order found.
        lives: Vec<LiveRow>,
        /// When they were asked.
        listed: Listed,
    },
}

/// The most channels one TV page carries: a country as large as the United
/// States has a couple of thousand, and a page is sent whole to a door.
pub const CHANNELS_MOST: usize = 1000;

/// When a directory's list was made, and whether a fresher one is being
/// asked: an old list is shown while the new one comes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Listed {
    /// When the directory answered it, in seconds since the epoch.
    pub fetched_unix_seconds: Option<i64>,
    /// It is older than its directory's freshness: a fresher one is asked,
    /// and this one stays on the page until it comes — or with the refusal
    /// when it does not.
    pub stale: bool,
}

/// What the globe draws of a page: every mark placed by the daemon, and how
/// many rows the map has no place for.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Globe {
    /// The marks, each once; the rows name theirs by id.
    pub marks: Vec<Mark>,
    /// The rows with no place on the map: no coordinate, and a country the
    /// map has no anchor for (or none at all).
    pub placeless: u32,
}

/// One mark on the globe. Marks closer on screen than the merge distance are
/// grouped when drawn — the door's, with the atlas — and a group's count is
/// the sum of its marks' weights.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    /// Its number on this page; the rows it stands for carry it.
    pub id: u32,
    /// Where it is.
    pub at: Spot,
    /// How many rows it stands for.
    pub weight: u32,
    /// What it is.
    pub kind: MarkKind,
    /// What the hover says: a station's or a place's name (data, like any
    /// name a directory gives); for a country, its code, which the door names
    /// in the person's language.
    pub label: String,
    /// Its country, when known.
    pub country: Option<Code>,
}

/// What a mark stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarkKind {
    /// A country, at the point its name goes on a map: a country of a world
    /// view, or the rows of a country that do not say where they are.
    Country,
    /// One station, where it says it is.
    Station,
    /// A city or a region a TV channel's feed names.
    Place,
}

/// A country and how many it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountryRow {
    /// The country.
    pub country: Code,
    /// How many stations, or channels.
    pub count: u32,
    /// Its mark; `None` when the map has no anchor for it.
    pub mark: Option<u32>,
}

/// A radio station, and what plays it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StationRow {
    /// What radio-browser says of it.
    pub station: Station,
    /// What the door puts in the queue: live, its other address as the next
    /// tried, and where it was found (for the click owed once it plays).
    pub stream: Stream,
    /// Its mark.
    pub mark: Option<u32>,
}

/// A genre or a language of radio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedRow {
    /// radio-browser's word for it — data, and what is asked back.
    pub name: String,
    /// How many stations.
    pub stations: u32,
    /// For a language, its code in [`crate::tongues`] when exactly one row
    /// of the table has radio-browser's two-letter code for it (`pt` is two:
    /// Brazil's and Portugal's): the door names it with that row's key in
    /// the person's language, else says [`NamedRow::name`].
    pub tongue: Option<String>,
}

/// A TV channel, and what plays it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelRow {
    /// iptv-org's id (`BBCNews.uk`) — what a favourite names it by.
    pub id: String,
    /// What it is called.
    pub name: String,
    /// Its other names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also: Vec<String>,
    /// Its country.
    pub country: Option<Code>,
    /// iptv-org's categories.
    pub categories: Vec<String>,
    /// The height of the picture of the stream tried first, when said.
    pub lines: Option<u16>,
    /// Every stream of it is labelled geo-blocked: it plays in few places.
    pub geo_blocked: bool,
    /// Every stream of it is labelled `Not 24/7`.
    pub sometimes_off: bool,
    /// The cities and regions its feeds name (data).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<String>,
    /// What the door puts in the queue: the best address, the others asked
    /// with the same headers as the ones tried next.
    pub stream: Stream,
    /// Its mark.
    pub mark: Option<u32>,
}

/// A film of the Internet Archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveRow {
    /// What the search said of it — its rights' address among it.
    pub item: archive::Item,
    /// What the door puts in the queue: the WHOLE item, which the daemon
    /// opens into its files when it is put (`archive::Item::stream`).
    pub stream: Stream,
}

/// A broadcast live on YouTube now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRow {
    /// What the search said of it.
    pub live: ronoctua_ytdlp::Live,
    /// The words it was found by — the category's own, said on the row as
    /// where it came from.
    pub asked: String,
    /// What the door puts in the queue.
    pub stream: Stream,
}

impl Page {
    /// The stream of the row that is `(kind, id)`, and its title and
    /// country — what a favourite or a recent keeps; `None` when no row of
    /// this page is.
    #[must_use]
    pub fn pick(
        &self,
        kind: ronoctua_broadcast::picks::Kind,
        id: &str,
    ) -> Option<(String, Option<Code>, Stream)> {
        use ronoctua_broadcast::picks::Kind;
        match (self, kind) {
            (Self::Stations { stations, .. }, Kind::Radio) => stations
                .iter()
                .find(|r| r.station.uuid == id)
                .map(|r| (r.station.name.clone(), r.station.country, r.stream.clone())),
            (Self::Channels { channels, .. }, Kind::Tv) => channels
                .iter()
                .find(|r| r.id == id)
                .map(|r| (r.name.clone(), r.country, r.stream.clone())),
            (Self::Archive { items, .. }, Kind::Archive) => items
                .iter()
                .find(|r| r.item.identifier == id)
                .map(|r| (r.item.title.clone(), None, r.stream.clone())),
            (Self::Lives { lives, .. }, Kind::Live) => lives
                .iter()
                .find(|r| r.live.id == id)
                .map(|r| (r.live.title.clone(), None, r.stream.clone())),
            _ => None,
        }
    }
}

/// Tags suggested for a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestionCandidate {
    /// Its number.
    pub id: CandidateId,
    /// The fields it would write, and a cover to fetch.
    pub suggestion: Suggestion,
    /// Its details were read (MusicBrainz's release: album artist, track
    /// and disc, label, cover); `false` while the search still reads them,
    /// and for the ones past the few read.
    pub complete: bool,
    /// Its cover, fetched for a door to show beside the file's own — asked
    /// when a person opens the candidate ([`crate::wire::Ask::SuggestedCover`]);
    /// `None` until then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen: Option<SuggestedCover>,
}

/// A suggested cover fetched to be SEEN, before anything is written: the
/// daemon asks the cover's host (a person's press let it, with the search),
/// keeps the picture in its runtime folder while the page is the desk's, and
/// a door draws it beside the file's own cover. Nothing is written into the
/// file by it: taking the candidate fetches the cover a tag carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "cover")]
pub enum SuggestedCover {
    /// Being fetched.
    Fetching,
    /// Fetched: the picture, in the player's runtime folder, until a newer
    /// search replaces the page or the player closes.
    Kept {
        /// Where it waits.
        path: String,
    },
    /// Not fetched, and why.
    Refused(Trouble),
}

/// A subtitle found for a film.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleCandidate {
    /// Its number.
    pub id: CandidateId,
    /// What the service said of it.
    pub found: Found,
    /// Its language as this system codes it (`pt-BR`), when the table has
    /// it.
    pub tongue: Option<String>,
}

/// Lyrics found for a song.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricsCandidate {
    /// Its number.
    pub id: CandidateId,
    /// The record.
    pub record: Record,
    /// How far its length is from the song's, in seconds, when both are
    /// known: what tells a live version from the studio one.
    pub off_by_seconds: Option<f64>,
}

/// Where lyrics taken go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LyricsTo {
    /// `<name>.lrc` beside the song — replacing one that is there only when
    /// the door asked the person first.
    #[default]
    Beside,
    /// Into the song's own lyrics tag.
    Tag,
}

/// How the last take of a desk went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Taken {
    /// The candidate taken.
    pub candidate: CandidateId,
    /// How it went.
    pub status: TakeStatus,
}

/// How a take goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "status")]
pub enum TakeStatus {
    /// Being fetched.
    Taking,
    /// Handed to the writing thread: [`crate::wire::State::retag`] says how
    /// the file's write goes.
    Handed,
    /// Written into this file — a subtitle (and a lane over the film, when
    /// the film plays), or lyrics beside the song.
    Saved {
        /// The new file.
        path: String,
    },
    /// Not taken, and why.
    Refused(Trouble),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a query stops being read as a door writes it — its kind
    /// under `what`, the words and languages left out when there are none.
    #[test]
    fn a_query_is_read_as_doors_write_it() {
        let q: Query = serde_json::from_str(
            r#"{"what":"suggest","location":"/m/a.flac","source":"musicbrainz"}"#,
        )
        .expect("a suggestion query");
        assert_eq!(
            q,
            Query::Suggest {
                location: "/m/a.flac".into(),
                source: Suggester::MusicBrainz,
                words: None,
                tongues: Vec::new(),
            }
        );
        assert_eq!(q.desk(), DeskKind::Suggestions);
        assert_eq!(
            q.services(),
            [Service::MusicBrainz, Service::CoverArtArchive]
        );
        let s: Query = serde_json::from_str(
            r#"{"what":"subtitles","location":"/v/f.mkv","tongues":["pt-BR","en"]}"#,
        )
        .expect("a subtitle query");
        assert_eq!(s.desk(), DeskKind::Subtitles);
        assert_eq!(s.location(), "/v/f.mkv");
        let l: Query =
            serde_json::from_str(r#"{"what":"lyrics","location":"/m/a.flac","words":"x"}"#)
                .expect("a lyrics query");
        assert_eq!(l.services(), [Service::Lrclib]);
    }

    /// FAILS IF: a directory's query stops being read as a door writes it —
    /// the directory under `what`, the list under `list`, `UK` read as `GB`
    /// — or goes to another desk than the one its page is drawn from.
    #[test]
    fn a_directorys_query_is_read_as_doors_write_it() {
        let read = |s: &str| serde_json::from_str::<Query>(s).expect(s);
        let radio = read(r#"{"what":"radio","list":"countries"}"#);
        assert_eq!(radio, Query::Radio(RadioList::Countries));
        assert_eq!(radio.desk(), DeskKind::Radio);
        assert_eq!(radio.directory(), Some(Directory::Radio));
        assert_eq!(radio.location(), "");
        assert!(radio.services().is_empty());
        assert_eq!(
            read(r#"{"what":"radio","list":"country","country":"UK"}"#),
            Query::Radio(RadioList::Country {
                country: ronoctua_broadcast::Code(*b"GB"),
                page: 1
            })
        );
        assert_eq!(
            read(r#"{"what":"radio","list":"country","country":"BR","page":3}"#),
            Query::Radio(RadioList::Country {
                country: ronoctua_broadcast::Code(*b"BR"),
                page: 3
            })
        );
        assert_eq!(
            read(r#"{"what":"tv","list":"placeless"}"#),
            Query::Tv(TvList::Placeless)
        );
        assert!(
            serde_json::from_str::<Query>(r#"{"what":"radio","list":"country","country":"ZZ"}"#)
                .is_err(),
            "a code that is no country is refused, never guessed"
        );
        assert_eq!(
            read(r#"{"what":"radio","list":"language","language":"portuguese"}"#),
            Query::Radio(RadioList::Language {
                language: "portuguese".into(),
                page: 1
            })
        );
        let theme = read(r#"{"what":"tv","list":"category","category":"news"}"#);
        assert_eq!(theme.desk(), DeskKind::Channels);
        assert_eq!(theme.directory(), Some(Directory::Iptv));
        assert_eq!(
            read(r#"{"what":"tv","list":"search","words":"bbc"}"#).desk(),
            DeskKind::Television
        );
        assert_eq!(
            read(r#"{"what":"archive","category":"films"}"#),
            Query::Archive {
                category: "films".into(),
                words: None,
                page: 1
            }
        );
        let lives = read(r#"{"what":"lives","category":"news"}"#);
        assert_eq!(lives.desk(), DeskKind::Lives);
        assert_eq!(lives.directory().map(Directory::host), Some("youtube.com"));
        for q in [
            radio,
            theme,
            lives,
            Query::Tv(TvList::Countries),
            Query::Radio(RadioList::Tags),
            Query::Archive {
                category: "anime".into(),
                words: Some("robot".into()),
                page: 3,
            },
        ] {
            let s = serde_json::to_string(&q).expect("serialises");
            assert_eq!(serde_json::from_str::<Query>(&s).expect(&s), q);
        }
    }

    /// FAILS IF: a directory's page stops carrying what a door draws — the
    /// rows with their stream and their mark, the globe's marks, when the
    /// list was made — or a favourite stops finding its row by kind and id.
    #[test]
    fn a_directorys_page_crosses_whole_and_names_its_rows() {
        use ronoctua_broadcast::picks::Kind;
        let station = Station {
            uuid: "7ada8a81-5ae1-418c-8f18-51d2f38d86a4".into(),
            name: "Bossa".into(),
            url: "https://radio.example/live".into(),
            alternate: None,
            homepage: None,
            country: Some(ronoctua_broadcast::Code(*b"BR")),
            tags: vec!["jazz".into()],
            languages: Vec::new(),
            codec: Some("AAC+".into()),
            bitrate_kbps: 64,
            hls: false,
            votes: 1,
            clicks: 2,
            at: None,
        };
        let desk = Desk {
            query: Some(Query::Radio(RadioList::Country {
                country: ronoctua_broadcast::Code(*b"BR"),
                page: 1,
            })),
            page: Page::Stations {
                stations: vec![StationRow {
                    stream: station.stream(),
                    station,
                    mark: Some(0),
                }],
                globe: Globe {
                    marks: vec![Mark {
                        id: 0,
                        at: Spot {
                            lat: -10.0,
                            lon: -52.0,
                        },
                        weight: 1,
                        kind: MarkKind::Country,
                        label: "BR".into(),
                        country: Some(ronoctua_broadcast::Code(*b"BR")),
                    }],
                    placeless: 0,
                },
                listed: Listed {
                    fetched_unix_seconds: Some(1_700_000_000),
                    stale: true,
                },
            },
            ..Desk::empty(DeskKind::Radio)
        };
        let s = serde_json::to_string(&desk).expect("serialises");
        assert!(s.contains(r#""page":{"page":"stations""#), "{s}");
        assert!(s.contains(r#""kind":"country""#), "{s}");
        assert!(s.contains(r#""stale":true"#), "{s}");
        assert_eq!(serde_json::from_str::<Desk>(&s).expect("parses"), desk);
        let (title, country, stream) = desk
            .page
            .pick(Kind::Radio, "7ada8a81-5ae1-418c-8f18-51d2f38d86a4")
            .expect("the row");
        assert_eq!(title, "Bossa");
        assert_eq!(country, Some(ronoctua_broadcast::Code(*b"BR")));
        assert_eq!(stream.url, "https://radio.example/live");
        assert_eq!(
            desk.page
                .pick(Kind::Tv, "7ada8a81-5ae1-418c-8f18-51d2f38d86a4"),
            None,
            "a station is never a channel"
        );
    }

    /// FAILS IF: a desk stops crossing the socket whole — its page under its
    /// own tag, a take's outcome with its trouble.
    #[test]
    fn a_desk_crosses_whole() {
        let desk = Desk {
            page: Page::Lyrics {
                location: "/m/a.flac".into(),
                duration_seconds: Some(200.0),
                candidates: vec![LyricsCandidate {
                    id: CandidateId(7),
                    record: Record {
                        id: 1,
                        track: "t".into(),
                        artist: "a".into(),
                        album: None,
                        duration_seconds: Some(201.0),
                        instrumental: false,
                        plain: Some("words".into()),
                        synced: None,
                    },
                    off_by_seconds: Some(1.0),
                }],
            },
            taken: Some(Taken {
                candidate: CandidateId(7),
                status: TakeStatus::Refused(Trouble::Exists {
                    path: "/m/a.lrc".into(),
                }),
            }),
            ..Desk::empty(DeskKind::Lyrics)
        };
        let s = serde_json::to_string(&desk).expect("serialises");
        assert!(s.contains(r#""page":{"page":"lyrics""#), "{s}");
        assert!(
            s.contains(r#""status":{"status":"refused","trouble":"exists","path":"/m/a.lrc"}"#),
            "{s}"
        );
        assert_eq!(serde_json::from_str::<Desk>(&s).expect("parses"), desk);
    }
}
