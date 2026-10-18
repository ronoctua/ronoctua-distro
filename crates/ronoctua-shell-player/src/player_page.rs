//! The player's view is a STACK OF PAGES.
//!
//! The home is the level at the bottom, and never leaves: what plays, the
//! queue, and one row for every other page. A row that opens a page pushes it
//! ([`crate::player::Target::Go`]); `Esc` pops one level and puts the cursor
//! back on the row that opened it — BY ITS IDENTITY, the row's spot, so a
//! state that moved the rows under the page in the meantime still lands on it
//! — and closes the door only from the home. A question on screen answers
//! "no" to `Esc` before any of that.
//!
//! # One key a page
//!
//! A page is named by ONE key, `player-page-<id>`: the row that opens it says
//! it, and the page is titled by it. Two words for one place were what the
//! census of `labels-of-a-view-are-distinct` found when the pages were drawn,
//! and one key cannot read two ways.
//!
//! # A page whose subject is gone says so
//!
//! A page shows something of the player's: the lanes playing together need
//! something playing. When that is gone the page stays — the person did not
//! leave it — and says so in a sentence (`player-page-gone`), never blank; and
//! with no player answering at all it says that instead, as the home does.

use crate::key;
use crate::player::{Purpose, View};
use ronoctua_player::wire::State;

/// A page of the player's view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageId {
    /// What plays, the queue, and a row for every other page — the bottom of
    /// the stack.
    Home,
    /// The FILE PICKER, for a purpose: into the queue, something to play
    /// together with what plays, a set or a queue to open, a subtitle, a
    /// cover, where a set or the queue is saved (`crate::file_picker`).
    Pick(Purpose),
    /// The lanes of what plays — pictures, sounds, subtitles on one clock —
    /// and a row to pick another.
    Together,
    /// What reaches the internet: what asks on its own, a switch for every
    /// service and directory a press lets, and the person's keys
    /// (`crate::player_internet`).
    Internet,
    /// The other machines on the tailnet this one plays in step with.
    Machines,
    /// One kind of the media the file carries — its sounds, its pictures or
    /// its subtitles — each one a row, the one in use said so
    /// (`crate::player_tracks`). The subtitles' is a page of its own: every
    /// subtitle lane over the picture, and where more come from
    /// (`crate::player_subtitles`).
    Tracks(TrackKind),
    /// The ambient sounds — recordings that play beside the queue, round and
    /// round (`crate::player_ambience`).
    Ambience,
    /// Who recorded each ambient sound, where, and under what licence.
    AmbienceCredits,
    /// One lane of what plays, by its id: its file, whether it plays, when
    /// it begins, its own volume and mute, whether its picture is shown,
    /// where a subtitle's lines sit, and taking it out
    /// (`crate::player_lane`).
    Lane(ronoctua_player::own::LaneId),
    /// Every lane of what plays on one clock — the deck, uncapped, a row a
    /// lane: the keyboard's door onto what the pointer drags
    /// (`crate::player_timeline`).
    Timeline,
    /// How the pictures of what plays share a screen: the form, a schematic
    /// of it, and each picture shown or hidden, in its place
    /// (`crate::player_layout`).
    Layout,
    /// The cover of the file the Metadata section shows: the picture, and
    /// another one from the files, saved beside the file, or taken out
    /// (`crate::player_metadata`).
    Cover,
    /// Every stream of that file's container.
    Streams,
    /// Its chapters.
    Chapters,
    /// Every tag the tools read of it, grouped by who read it.
    EveryTag,
    /// Tags for that file from the internet: who is asked, the row that asks
    /// them, and what they suggested (`crate::player_suggest`).
    Suggestions,
    /// One suggestion, by its number: each field it would write beside what
    /// the file says now, its cover beside the file's, and the row that
    /// writes what is chosen.
    Candidate(ronoctua_player::find::CandidateId),
    /// Subtitles for a film that plays, from the internet: the languages,
    /// the row that asks, and what was found (`crate::player_find`).
    FindSubtitles,
    /// Lyrics for the file shown, from LRCLIB.
    FindLyrics,
    /// Every language subtitles can be searched in, each on or off, and a
    /// row to type one (`crate::player_find`).
    Languages,
    /// A several-line field of the file the Metadata section shows — its
    /// description, its lyrics, a comment — read whole, and a row that opens
    /// it in the editor (`crate::player_metadata`).
    Field(ronoctua_tags::Field),
    /// WORLD RADIO: the globe of the countries with stations, a search, the
    /// favourites and the played lately, the genres and the languages, and a
    /// row a country (`crate::player_directory`).
    Radio,
    /// The genres of radio, the most stations first.
    RadioGenres,
    /// The languages of radio, the most stations first.
    RadioLanguages,
    /// Radio stations — a country's, a genre's, a language's, a search's:
    /// the listing its level holds ([`Level::query`]), titled by what it is
    /// of.
    Stations,
    /// WORLD TV: the globe of the countries with channels, a search, and a
    /// row a country.
    Tv,
    /// TV channels — a country's or a search's: the listing its level holds.
    TvChannels,
    /// CHANNELS BY THEME, on the local catalogue: one source at a time, its
    /// themes and categories with how many each holds, and the adult ones
    /// behind the person's password (`ronoctua_shell_catalogue::player_catalog`).
    Channels,
    /// A LISTING of the catalogue — a theme's, a category's, a search's: its
    /// letters, its pages, its rings and its items, drawn as a list, in
    /// columns or as posters. What it lists is the view's
    /// (`player_catalog::Listing`).
    Catalog,
    /// The catalogue's SOURCES: the defaults, the person's, and the row that
    /// adds one by its link (`ronoctua_shell_catalogue::player_sources`).
    Sources,
    /// What the person kept among the favourites — stations, channels,
    /// films, lives.
    Favourites,
    /// What played lately, as the player remembers it.
    Recent,
    /// A VIDEO FROM A WEBSITE: a page's address, played now or queued, and
    /// the addresses played lately (`crate::player_website`).
    Website,
    /// THE WHOLE QUEUE: the home shows a window of a long one around what
    /// plays, and its row "Queue · N" opens this — every entry, with the
    /// queue's own title and controls (`crate::player_queue`).
    Queue,
    /// THE RANKING of the video catalogue: its videos, best first, a page at
    /// a time — a click puts one in the queue (`ronoctua_shell_video_metrics::player_ranking`).
    Ranking,
    /// One video of the ranking, by the view's choice: play it now, add it,
    /// watch it in Chrome, download it, hide it, delete it.
    RankedVideo,
}

/// A kind of the media one file carries — the wire's `audio`, `video` and
/// `sub`, each played by the lane of its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    /// Its sounds: a dub, a commentary.
    Sound,
    /// Its pictures: another angle.
    Picture,
    /// Its subtitles — the film's own, and every subtitle lane over the
    /// picture, on their page (`crate::player_subtitles`).
    Subtitles,
}

impl TrackKind {
    /// The three, in the order the home and the transport's `k` walk them.
    pub const ALL: [Self; 3] = [Self::Sound, Self::Picture, Self::Subtitles];

    /// Its word in a row's spot — the wire's.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Sound => "audio",
            Self::Picture => "video",
            Self::Subtitles => "sub",
        }
    }

    /// The wire's kind of track.
    #[must_use]
    pub const fn wire(self) -> ronoctua_player::wire::TrackKind {
        use ronoctua_player::wire::TrackKind as Wire;
        match self {
            Self::Sound => Wire::Audio,
            Self::Picture => Wire::Video,
            Self::Subtitles => Wire::Sub,
        }
    }

    /// The kind of lane that plays one of these.
    #[must_use]
    pub const fn lane(self) -> ronoctua_player::own::LaneKind {
        use ronoctua_player::own::LaneKind;
        match self {
            Self::Sound => LaneKind::Audio,
            Self::Picture => LaneKind::Video,
            Self::Subtitles => LaneKind::Subtitle,
        }
    }

    /// Whether `state`'s file offers a choice of this kind: two sounds or
    /// two pictures — a sound or a picture cannot be switched off — and, for
    /// the subtitles, a picture to lay them over or one of them already: the
    /// subtitles' page is where they come from too, the file's own, a file
    /// beside it, any file (`crate::player_subtitles`). A cover is no picture
    /// of the file (`player_tracks::of_kind`).
    #[must_use]
    pub fn a_choice_in(self, state: &State) -> bool {
        match self {
            Self::Subtitles => {
                crate::player::has_a_picture(state)
                    || state
                        .lanes
                        .iter()
                        .any(|l| l.kind == ronoctua_player::own::LaneKind::Subtitle)
            }
            Self::Sound | Self::Picture => crate::player_tracks::of_kind(self, state).len() > 1,
        }
    }

    /// The kinds of `state`'s file there is a choice of, in order — the
    /// home's rows, what `k` walks and what the transport's glyph stands for.
    /// Every kind is chosen the same way, its lane asked to play another of
    /// the file's tracks (`Ask::Track`) — a picture too, since the player
    /// plays lanes.
    pub fn with_a_choice(state: &State) -> impl Iterator<Item = Self> + '_ {
        Self::ALL.into_iter().filter(|k| k.a_choice_in(state))
    }
}

/// Whether what a page shows is there to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    /// It is: the page draws its rows.
    Here,
    /// No player answers: the page says so, as the home does.
    NotRunning,
    /// The player answers and what the page was about is gone.
    Gone,
}

impl PageId {
    /// Every page above the home — each one reached by a row of another
    /// (`every_page_is_reached_by_a_row`). A NEW PAGE GOES HERE BY HAND: the
    /// matches in [`PageId::key`] and [`PageId::spot`] will not compile
    /// without it being named, but nothing can list an enum's values for a
    /// test, and one left out of this list is one that test does not walk.
    ///
    /// A LANE'S PAGE is opened by `RET` on the lane's own row, whose target is
    /// the lane and not the page (its arrows and `d` act on the lane): the
    /// test follows those rows too, and stands on lane 3 — the subtitles of
    /// the film it plays.
    ///
    /// And what `photograph --open` names a page among ([`PageId::by_key`]),
    /// which is how `player-pages-fit-every-size` photographs each one.
    pub const PAGES: [Self; 43] = [
        Self::Pick(Purpose::Queue),
        Self::Pick(Purpose::Lane),
        Self::Pick(Purpose::Open),
        Self::Pick(Purpose::Subtitle),
        Self::Pick(Purpose::Cover),
        Self::Pick(Purpose::SaveSet),
        Self::Pick(Purpose::SaveQueue),
        Self::Cover,
        Self::Streams,
        Self::Chapters,
        Self::EveryTag,
        Self::Together,
        Self::Internet,
        Self::Machines,
        Self::Tracks(TrackKind::Sound),
        Self::Tracks(TrackKind::Picture),
        Self::Tracks(TrackKind::Subtitles),
        Self::Ambience,
        Self::AmbienceCredits,
        Self::Lane(ronoctua_player::own::LaneId(3)),
        Self::Timeline,
        Self::Layout,
        Self::Suggestions,
        Self::Candidate(ronoctua_player::find::CandidateId(1)),
        Self::FindSubtitles,
        Self::FindLyrics,
        Self::Languages,
        Self::Field(ronoctua_tags::Field::Lyrics),
        Self::Radio,
        Self::RadioGenres,
        Self::RadioLanguages,
        Self::Stations,
        Self::Tv,
        Self::TvChannels,
        Self::Channels,
        Self::Catalog,
        Self::Sources,
        Self::Favourites,
        Self::Recent,
        Self::Website,
        Self::Queue,
        Self::Ranking,
        Self::RankedVideo,
    ];

    /// The page whose own key (its title, [`PageId::key`]) is `key`, among
    /// [`PageId::PAGES`]: how `photograph --open` names one.
    #[must_use]
    pub fn by_key(key: &str) -> Option<Self> {
        Self::PAGES.into_iter().find(|p| p.key() == key)
    }

    /// Its ONE key: the label of the row that opens it, and its title. The
    /// home's is the door's own title, which it is drawn under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Home => key::PLAYER_TITLE,
            Self::Pick(Purpose::Queue) => key::PLAYER_PAGE_FILES,
            Self::Pick(Purpose::Lane) => key::PLAYER_PAGE_FILES_TOGETHER,
            Self::Pick(Purpose::Open) => key::PLAYER_PAGE_FILES_OPEN,
            Self::Together => key::PLAYER_PAGE_TOGETHER,
            Self::Internet => key::PLAYER_PAGE_INTERNET,
            Self::Machines => key::PLAYER_PAGE_MACHINES,
            Self::Tracks(TrackKind::Sound) => key::PLAYER_PAGE_SOUND,
            Self::Tracks(TrackKind::Picture) => key::PLAYER_PAGE_PICTURE,
            Self::Tracks(TrackKind::Subtitles) => key::PLAYER_PAGE_SUBTITLES,
            Self::Pick(Purpose::Subtitle) => key::PLAYER_PAGE_FILES_SUBTITLE,
            Self::Ambience => key::PLAYER_PAGE_AMBIENCE,
            Self::AmbienceCredits => key::PLAYER_PAGE_AMBIENCE_CREDITS,
            Self::Lane(_) => key::PLAYER_PAGE_LANE,
            Self::Timeline => key::PLAYER_PAGE_TIMELINE,
            Self::Layout => key::PLAYER_PAGE_LAYOUT,
            Self::Pick(Purpose::Cover) => key::PLAYER_PAGE_FILES_COVER,
            Self::Pick(Purpose::SaveSet) => key::PLAYER_PAGE_PICK_SAVE_SET,
            Self::Pick(Purpose::SaveQueue) => key::PLAYER_PAGE_PICK_SAVE_QUEUE,
            Self::Cover => key::PLAYER_PAGE_COVER,
            Self::Streams => key::PLAYER_PAGE_STREAMS,
            Self::Chapters => key::PLAYER_PAGE_CHAPTERS,
            Self::EveryTag => key::PLAYER_PAGE_EVERY_TAG,
            Self::Suggestions => key::PLAYER_PAGE_SUGGESTIONS,
            Self::Candidate(_) => key::PLAYER_PAGE_CANDIDATE,
            Self::FindSubtitles => key::PLAYER_PAGE_FIND_SUBTITLES,
            Self::FindLyrics => key::PLAYER_PAGE_FIND_LYRICS,
            Self::Languages => key::PLAYER_PAGE_LANGUAGES,
            // A FIELD'S PAGE IS NAMED BY THE FIELD, as its row is.
            Self::Field(f) => crate::tag_words::field_key(f),
            Self::Radio => key::PLAYER_PAGE_RADIO,
            Self::RadioGenres => key::PLAYER_PAGE_RADIO_GENRES,
            Self::RadioLanguages => key::PLAYER_PAGE_RADIO_LANGUAGES,
            Self::Stations => key::PLAYER_PAGE_STATIONS,
            Self::Tv => key::PLAYER_PAGE_TV,
            Self::TvChannels => key::PLAYER_PAGE_TV_CHANNELS,
            Self::Channels => key::PLAYER_PAGE_CHANNELS,
            Self::Catalog => key::PLAYER_PAGE_CATALOG,
            Self::Sources => key::PLAYER_PAGE_SOURCES,
            Self::Favourites => key::PLAYER_PAGE_FAVOURITES,
            Self::Recent => key::PLAYER_PAGE_RECENT,
            Self::Website => key::PLAYER_PAGE_WEBSITE,
            Self::Queue => key::PLAYER_PAGE_QUEUE,
            Self::Ranking => key::PLAYER_PAGE_RANKING,
            Self::RankedVideo => key::PLAYER_PAGE_RANKED_VIDEO,
        }
    }

    /// The spot of the row that opens it.
    #[must_use]
    pub const fn spot(self) -> &'static str {
        match self {
            Self::Home => "page-home",
            Self::Pick(Purpose::Queue) => "page-files",
            Self::Pick(Purpose::Lane) => "page-files-together",
            Self::Pick(Purpose::Open) => "page-files-open",
            Self::Together => "page-together",
            Self::Internet => "page-internet",
            Self::Machines => "page-machines",
            Self::Tracks(TrackKind::Sound) => "page-sound",
            Self::Tracks(TrackKind::Picture) => "page-picture",
            Self::Tracks(TrackKind::Subtitles) => "page-subtitles",
            Self::Pick(Purpose::Subtitle) => "page-files-subtitle",
            Self::Ambience => "page-ambience",
            Self::AmbienceCredits => "page-ambience-credits",
            Self::Lane(_) => "page-lane",
            Self::Timeline => "page-timeline",
            Self::Layout => "page-layout",
            Self::Pick(Purpose::Cover) => "page-files-cover",
            Self::Pick(Purpose::SaveSet) => "page-save-set",
            Self::Pick(Purpose::SaveQueue) => "page-save-queue",
            Self::Cover => "page-cover",
            Self::Streams => "page-streams",
            Self::Chapters => "page-chapters",
            Self::EveryTag => "page-every-tag",
            Self::Suggestions => "page-suggestions",
            Self::Candidate(_) => "page-candidate",
            Self::FindSubtitles => "page-find-subtitles",
            Self::FindLyrics => "page-find-lyrics",
            Self::Languages => "page-languages",
            // A FIELD'S ROW on the Metadata section is its own.
            Self::Field(f) => crate::player_metadata::field_spot(f),
            Self::Radio => "page-radio",
            Self::RadioGenres => "page-radio-genres",
            Self::RadioLanguages => "page-radio-languages",
            Self::Stations => "page-stations",
            Self::Tv => "page-tv",
            Self::TvChannels => "page-tv-channels",
            Self::Channels => "page-channels",
            Self::Catalog => "page-catalog",
            Self::Sources => "page-sources",
            Self::Favourites => "page-favourites",
            Self::Recent => "page-recent",
            Self::Website => "page-website",
            Self::Queue => "page-queue",
            Self::Ranking => "page-ranking",
            Self::RankedVideo => "page-ranked-video",
        }
    }

    /// Whether what it shows is there, in `state`.
    #[must_use]
    pub fn subject(self, state: Option<&State>) -> Subject {
        match (self, state) {
            // THE SWITCHES AND THE FILES ARE THE MACHINE'S, and a player that
            // is not running reads them when it starts.
            (Self::Home | Self::Pick(_) | Self::Internet, _) => Subject::Here,
            // AN ADDRESS IS TYPED with no player running: the one that plays
            // it is started to play it.
            (Self::Website, _) => Subject::Here,
            // THE LANGUAGES ARE THE VIEW'S CHOICE, there with or without a
            // player: the search that uses them asks it.
            (Self::Languages, _) => Subject::Here,
            // THE CATALOGUE IS A FILE this door reads, there with no player
            // running; only a reading of a source is the player's.
            (Self::Channels | Self::Catalog, _) => Subject::Here,
            // THE RANKING IS THE VIDEO CATALOGUE'S, a file this door reads:
            // there with no player running — the one that plays a video is
            // started to play it (`ronoctua_shell_video_metrics::player_ranking`).
            (Self::Ranking | Self::RankedVideo, _) => Subject::Here,
            // ITS SOURCES are the player's to keep: with none running, the
            // page says so.
            (Self::Sources, None) => Subject::NotRunning,
            (Self::Sources, Some(_)) => Subject::Here,
            // THE QUEUE is the player's: an empty one says so on its page.
            (Self::Queue, None) => Subject::NotRunning,
            (Self::Queue, Some(_)) => Subject::Here,
            // THE DIRECTORIES ARE ASKED OF THE PLAYER, which keeps their
            // lists — and the favourites and the played lately are played by
            // it: with none running, the page says so.
            (
                Self::Radio
                | Self::RadioGenres
                | Self::RadioLanguages
                | Self::Stations
                | Self::Tv
                | Self::TvChannels
                | Self::Favourites
                | Self::Recent,
                None,
            ) => Subject::NotRunning,
            (
                Self::Radio
                | Self::RadioGenres
                | Self::RadioLanguages
                | Self::Stations
                | Self::Tv
                | Self::TvChannels
                | Self::Favourites
                | Self::Recent,
                Some(_),
            ) => Subject::Here,
            (
                Self::Together
                | Self::Machines
                | Self::Tracks(_)
                | Self::Ambience
                | Self::AmbienceCredits
                | Self::Lane(_)
                | Self::Timeline
                | Self::Layout
                | Self::Cover
                | Self::Streams
                | Self::Chapters
                | Self::EveryTag
                | Self::Suggestions
                | Self::Candidate(_)
                | Self::FindSubtitles
                | Self::FindLyrics
                | Self::Field(_),
                None,
            ) => Subject::NotRunning,
            // SUBTITLES ARE FOR A FILM, while one plays.
            (Self::FindSubtitles, Some(s)) => {
                if crate::player::loaded(s) && crate::player::has_a_picture(s) {
                    Subject::Here
                } else {
                    Subject::Gone
                }
            }
            // WHAT A FILE THAT PLAYS SAYS OF ITSELF, while something plays —
            // and what the internet suggests for it.
            (
                Self::Cover
                | Self::Streams
                | Self::Chapters
                | Self::EveryTag
                | Self::Suggestions
                | Self::Candidate(_)
                | Self::FindLyrics
                | Self::Field(_),
                Some(s),
            ) => {
                if crate::player::loaded(s) {
                    Subject::Here
                } else {
                    Subject::Gone
                }
            }
            // THE AMBIENT SOUNDS ARE THE MACHINE'S: the page says what there
            // is — and, with none, why (`Ambience::trouble`).
            (Self::Machines | Self::Ambience, Some(_)) => Subject::Here,
            (Self::AmbienceCredits, Some(s)) => {
                if s.ambience.sounds.is_empty() {
                    Subject::Gone
                } else {
                    Subject::Here
                }
            }
            // A LANE, while what plays still has it.
            (Self::Lane(id), Some(s)) => {
                if s.lanes.iter().any(|l| l.id == id) {
                    Subject::Here
                } else {
                    Subject::Gone
                }
            }
            // THE FILE'S MEDIA, while it still offers a choice of this kind:
            // the next file may carry one sound, and the page says so.
            (Self::Tracks(kind), Some(s)) => {
                if kind.a_choice_in(s) {
                    Subject::Here
                } else {
                    Subject::Gone
                }
            }
            // THE LANES OF WHAT PLAYS, which has to be there.
            (Self::Together | Self::Timeline | Self::Layout, Some(s)) => {
                if crate::player::loaded(s) && !s.lanes.is_empty() {
                    Subject::Here
                } else {
                    Subject::Gone
                }
            }
        }
    }
}

/// One level of the stack: its page, where its cursor is, and the row of the
/// level below that opened it.
#[derive(Debug)]
pub struct Level {
    /// Which page.
    pub page: PageId,
    /// Which row the cursor is on.
    pub cursor: usize,
    /// Which entry of a grid row the cursor is on.
    pub cell: usize,
    /// A person moved the cursor here: settling leaves it where it is.
    pub moved: bool,
    /// The file picker, on its page — its folder, what was typed, its own
    /// cursor (`crate::file_picker`).
    pub picker: Option<Box<crate::file_picker::FilePicker>>,
    /// The spot of the row that opened this page, on the level below — where
    /// `Esc` puts the cursor back.
    pub opened_from: Option<String>,
    /// WHAT A DIRECTORY'S PAGE LISTS — a country's stations, a genre's, a
    /// search's, a theme's page of films: the page is one, and what it is of
    /// is the level's (`crate::player_directory`).
    pub query: Option<ronoctua_player::find::Query>,
    /// THE ROWS NARROWED to a group of the globe's marks — its members — a
    /// second click on a group of several asked for (`crate::player_globe`).
    pub only: Option<Vec<u32>>,
    /// A WORLD PAGE'S COUNTRIES A TO Z by their name in the person's
    /// language, and not the most first (`O`, the glyph on their title):
    /// the page's own, never kept past it.
    pub by_name: bool,
}

impl Level {
    /// The home, the cursor on its first row.
    #[must_use]
    pub const fn home() -> Self {
        Self {
            page: PageId::Home,
            cursor: 1,
            cell: 0,
            moved: false,
            picker: None,
            opened_from: None,
            query: None,
            only: None,
            by_name: false,
        }
    }
}

impl View {
    /// The level on top — the page on screen. There is always one: the home
    /// is never popped.
    pub fn top(&self) -> &Level {
        self.levels
            .last()
            .expect("the player's view always has its home")
    }

    /// The level on top, to change.
    pub fn top_mut(&mut self) -> &mut Level {
        self.levels
            .last_mut()
            .expect("the player's view always has its home")
    }

    /// The page on screen.
    #[must_use]
    pub fn page(&self) -> PageId {
        self.top().page
    }

    /// Opens `page` over the one on screen, remembering the row that opened
    /// it; the picker's page opens where its purpose was left. A page ALREADY IN
    /// THE STACK is gone back to instead, and the pages over it close: `LEADER
    /// p m` and `k` pressed in turn stacked the same two pages without end,
    /// and `Esc` had to unwind every one of them.
    pub fn push(&mut self, page: PageId, opened_from: Option<String>) {
        if let Some(at) = self.levels.iter().position(|l| l.page == page) {
            self.levels.truncate(at + 1);
            return;
        }
        let picker = match page {
            PageId::Pick(purpose) => Some(Box::new(self.picker_for(purpose, None, String::new()))),
            _ => None,
        };
        // THE FILES BESIDE THE FILM are read again when the subtitles' page
        // opens: one laid there since it was last open is on it.
        if page == PageId::Tracks(TrackKind::Subtitles) {
            self.beside.forget();
        }
        self.levels.push(Level {
            page,
            cursor: 0,
            cell: 0,
            moved: false,
            picker,
            opened_from,
            query: crate::player_directory::listed_on(page),
            only: None,
            by_name: false,
        });
        // A DIRECTORY'S PAGE ASKS FOR ITS LIST as it opens — the daemon's
        // desk is every door's, and another may hold something else on it.
        self.listing_follows();
    }

    /// Opens a page of a directory that lists `query` — a country's
    /// stations, a search's — over the one on screen. The same page already
    /// in the stack is gone back to and made to list this instead: a second
    /// search is a new list on the same page, not a page over it.
    pub fn push_listing(
        &mut self,
        page: PageId,
        query: ronoctua_player::find::Query,
        opened_from: Option<String>,
    ) {
        if let Some(at) = self.levels.iter().position(|l| l.page == page) {
            self.levels.truncate(at + 1);
            let top = self.top_mut();
            top.query = Some(query);
            top.only = None;
            top.cursor = 0;
            top.moved = false;
            self.listing_follows();
            return;
        }
        self.push(page, opened_from);
        let top = self.top_mut();
        top.query = Some(query);
        self.listing_follows();
    }

    /// Back one level, the cursor on the row that opened the page — by its
    /// spot, and where it was when that row is gone. `false` on the home,
    /// which is never popped: `Esc` there closes the door.
    pub fn back(
        &mut self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> bool {
        if self.levels.len() < 2 {
            return false;
        }
        let Some(left) = self.levels.pop() else {
            return false;
        };
        if let Some(spot) = left.opened_from {
            self.follow(&spot, state, columns, texts);
        }
        // AND THE PAGE COME BACK TO asks for its list again, when it is a
        // directory's: the desk may hold the page just left.
        self.listing_follows();
        // WHERE A PERSON PUT IT: settling must not move it off the row.
        self.top_mut().moved = true;
        self.settle_in(state, columns, texts);
        true
    }
}
