//! The player — one view, three doors: the footer (`LEADER p p`), a window
//! (`LEADER p w`) and the column on the right (`LEADER p c`).
//!
//! # What is here
//!
//! Which rows exist, what a key does on each, and what a door must do about
//! it. Nothing here plays anything: `ronoctua-player` does, in a process of
//! its own, and every door is a client of its socket. The shape is
//! `camera_settings.rs`, down to the rings.
//!
//! The view is a STACK OF PAGES (`crate::player_page`): the home at the bottom
//! (`crate::player_home`) — what plays and a row for every other page — and
//! the pages above it, the files (`f`, `LEADER p l`), playing together, the
//! internet (`crate::player_internet`), the other machines
//! (`crate::player_machines`, `LEADER p m`), the media in the file — its
//! sounds and pictures (`crate::player_tracks`, `k`) — the subtitles over the
//! picture (`crate::player_subtitles`, `u`, `LEADER p u`), a lane's own page
//! (`crate::player_lane`) and the ambient sounds with their credits
//! (`crate::player_ambience`, `LEADER p e`). `Esc` goes
//! back one page, the cursor on the row that opened it, and closes the door
//! from the home. Every page writes its rows with one builder
//! (`crate::player_rows`), in FLAT files beside this one: nine checks read the
//! shell's sources one directory deep.
//!
//! # Every value is a ring
//!
//! `←`/`→` turn the value on the row and `RET` does what a click on it does
//! (invariant 19): on the position it jumps ahead, on the volume it mutes, on
//! the home's row of a sound or of the subtitles it opens their page — the
//! arrows still turn the one in use — and on an item of the queue it plays
//! it.
//!
//! # The files, from the player itself
//!
//! A PAGE of the view: the music, videos and music videos folders as a ring,
//! the folders under the one that is open, and the files in it the player
//! plays (`player_browse::glyph_if_played`). `RET` enters a folder or plays a
//! file alone, `a` puts one in the queue — a folder whole — `DEL` goes up, and
//! `Esc` goes back to the page that opened them.
//!
//! # Every door types
//!
//! A file or an address to play, one to add to the queue, a video to play in
//! step and the other machine a pairing is with are a question the view hands
//! its door (`Errand::Ask`, `crate::player_question`): the footer asks it on
//! its minibuffer, a window and the column on their own header's line
//! (`door::Ui::ask`) — the view stays open under it in both. What was typed
//! comes back to ONE function, `player_question::answered`, whichever door
//! typed it.
//!
//! # The bars are controls in every door
//!
//! The position and the volume are drawn as bars (`player_rows::Slider`), and
//! a press on one sets the value there and holding drags it —
//! `player_rows::slider_at` and `player_rows::slid`, the one arithmetic both
//! doors ask, beside the window's own time bar and volume track.

use crate::player_page::{Level, Subject};
pub use crate::player_page::{PageId, TrackKind};
use crate::player_question::{ENQUEUE, OPEN_SAVED, Question, SUBTITLE_FILE, TOGETHER};
use crate::player_rows::{Builder, Row};
use crate::player_transport::{button_errand, stepped_volume, volume_step};
use ronoctua_player::{
    limit::Limit,
    own::LaneId,
    queue::{EntryId, Name, Node, Outline},
    remote::Role,
    wire::{Ask, DeskKind, LaneChange, State, Thing, Track, Trouble},
};
use ronoctua_tokens as tokens;

/// The keys this view teaches, and where each one applies.
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "player-down", Where::Anywhere),
    ("<up>", "player-up", Where::Anywhere),
    ("<left>", "player-less", Where::OnAValue),
    ("<right>", "player-more", Where::OnAValue),
    ("RET", "player-enter", Where::OnARow),
    ("SPC", "player-toggle", Where::Anywhere),
    ("s", "player-stop", Where::Anywhere),
    ("n", "player-next", Where::Anywhere),
    ("p", "player-previous", Where::Anywhere),
    ("d", "player-remove", Where::OnRemovable),
    ("f", "player-browse", Where::OutOfTheFiles),
    ("a", "player-add", Where::OnAFile),
    ("DEL", "player-parent", Where::InTheFiles),
    ("P", "player-play-now", Where::OnAFile),
    ("c", "player-clear", Where::OutOfTheFiles),
    ("-", "player-quieter", Where::Anywhere),
    ("=", "player-louder", Where::Anywhere),
    ("o", "player-only", Where::Anywhere),
    ("m", "player-maximize", Where::Anywhere),
    ("i", "player-pip", Where::Anywhere),
    ("0", "player-mute", Where::Anywhere),
    ("w", "player-swap", Where::Anywhere),
    ("k", "player-tracks", Where::WithAChoice),
    ("TAB", "player-fold", Where::OnAGroup),
    // `C-↑`/`C-↓` MOVE THE ENTRY under the cursor — a file, a playlist, a
    // set — one place earlier or later among its siblings: the `↑` and `↓`
    // at the end of its row, which a click presses (invariant 19). The plain
    // arrows walk the rows, and on a group `←`/`→` fold it.
    ("C-<up>", "player-earlier", Where::OnAnEntry),
    ("C-<down>", "player-later", Where::OnAnEntry),
    // `a` ON ONE OF THE FILE'S MEDIA plays it too, beside the one in use —
    // the same letter as adding a file to the queue, taught with its own
    // words where it means this (`pressed` reads the first `a`, the add).
    ("a", "player-also", Where::OnATrack),
    ("r", "player-rename", Where::OnAnEntry),
    // `u` FOR THE SUBTITLES over the picture — their page, from anywhere
    // while a picture plays (`LEADER p u` from outside the view).
    ("u", "player-subtitles", Where::WithAPicture),
    // `v` SHOWS OR HIDES a lane — a subtitle over the picture, any lane of
    // what plays — as a click on its chip does: `RET` on the row opens the
    // lane's own page.
    ("v", "player-lane-shown", Where::OnALane),
    // `t` FOR THE TIMELINE: every lane of what plays on one clock, a row a
    // lane — from anywhere while something with lanes plays (`LEADER p i`
    // from outside the view).
    ("t", "player-timeline", Where::WithLanes),
    // `y` FOR THE LAYOUT — how the pictures share a screen (`LEADER p y`
    // from outside the view) — and `M` for showing what plays on the whole
    // of every screen it is routed to: `m` is the whole of this one, and like
    // it `M` acts from anywhere, the rows drawn or not.
    ("y", "player-layout", Where::WithLanes),
    ("M", "player-every-screen", Where::Anywhere),
    // `/` SEARCHES a directory's page by name — stations, channels, films —
    // asked on the door's line (`crate::player_directory`).
    ("/", "player-search", Where::OnADirectory),
    // `b` KEEPS A STATION — a channel, a film, a live — among the favourites,
    // or takes it off: the star at the head of its row.
    ("b", "player-favourite", Where::OnAPick),
    // THE GLOBE of a directory's page: nearer, further, back to how the page
    // frames it — the file manager's three for its globe
    // (`crate::player_globe`).
    ("C-k", "player-nearer", Where::OnTheGlobe),
    ("C-j", "player-further", Where::OnTheGlobe),
    ("C-r", "player-world", Where::OnTheGlobe),
];

/// Where a key applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// On a row whose value turns.
    OnAValue,
    /// On a row that does something.
    OnARow,
    /// On an entry of the queue, or a lane playing together.
    OnRemovable,
    /// On an entry of the queue — a file, a set, a playlist.
    OnAnEntry,
    /// On a group of the queue: a playlist, a set.
    OnAGroup,
    /// On one of the file's media, on its kind's page.
    OnATrack,
    /// Anywhere but among the files.
    OutOfTheFiles,
    /// Among the files.
    InTheFiles,
    /// On a folder or a file of the listing, or the folder that is open.
    OnAFile,
    /// While the file offers a choice of its media that can be made here —
    /// two sounds, a subtitle (`TrackKind::with_a_choice`).
    WithAChoice,
    /// While a picture plays, which subtitles can go over.
    WithAPicture,
    /// On a lane of what plays: a subtitle on its page, a lane playing
    /// together.
    OnALane,
    /// While something with lanes plays — what the timeline shows.
    WithLanes,
    /// On a directory's page that searches by name: the world radio and TV,
    /// a list of stations or channels, a theme with films.
    OnADirectory,
    /// On a row that plays from a directory — a station, a channel, a film,
    /// a live.
    OnAPick,
    /// On a page that draws a globe.
    OnTheGlobe,
}

impl Where {
    fn shown(self, target: &Target, state: Option<&State>, page: PageId) -> bool {
        match self {
            Self::Anywhere => true,
            Self::OnADirectory => match page {
                PageId::Radio
                | PageId::Stations
                | PageId::RadioGenres
                | PageId::RadioLanguages
                | PageId::Tv
                | PageId::TvChannels => true,
                PageId::Theme(c) => !c.archive.is_empty(),
                _ => false,
            },
            Self::OnAPick => matches!(target, Target::Pick(..)),
            Self::OnTheGlobe => crate::player_globe::on(page) && crate::globe::present(),
            Self::WithAChoice => {
                state.is_some_and(|s| TrackKind::with_a_choice(s).next().is_some())
            }
            Self::WithAPicture => state.is_some_and(has_a_picture),
            Self::WithLanes => state.is_some_and(|s| loaded(s) && !s.lanes.is_empty()),
            Self::OnALane => matches!(target, Target::Subtitle(_) | Target::Lane(_)),
            Self::OnAValue => target.turns(),
            Self::OnARow => target.acts(),
            // A FAVOURITE, on its own page, is taken off with `d`.
            Self::OnRemovable if matches!(target, Target::Pick(..)) => page == PageId::Favourites,
            Self::OnRemovable => matches!(
                target,
                Target::Entry(_)
                    | Target::Group(_)
                    | Target::Lane(_)
                    | Target::Subtitle(_)
                    | Target::LaneUntil(_)
                    | Target::Tag(_)
                    | Target::OtherTag(..)
                    | Target::Host(_)
                    | Target::Guest(_)
                    | Target::Language(_)
                    | Target::Key(_)
            ),
            Self::OnAnEntry => matches!(target, Target::Entry(_) | Target::Group(_)),
            Self::OnAGroup => matches!(target, Target::Group(_)),
            Self::OnATrack => matches!(target, Target::Track { id: Some(_), .. }),
            Self::OutOfTheFiles => !target.among_the_files(),
            Self::InTheFiles => target.among_the_files(),
            // A ROW THAT PLAYS from a directory is added and played as a
            // file is.
            Self::OnAFile => matches!(
                target,
                Target::Folder(_)
                    | Target::File(_)
                    | Target::PlayFolder
                    | Target::EnqueueFolder
                    | Target::Pick(..)
            ),
        }
    }
}

/// A key, read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up a row.
    Up,
    /// Down a row.
    Down,
    /// The value back a step.
    Less,
    /// The value on a step.
    More,
    /// What a click on the row does.
    Enter,
    /// Play or pause, wherever the cursor is.
    Toggle,
    /// Stop: the queue stays, and nothing plays.
    Stop,
    /// The next item of the queue.
    Next,
    /// The previous one.
    Previous,
    /// Take the entry (or the lane) off.
    Remove,
    /// Open the files.
    Browse,
    /// Open a page over the one on screen — a command that names one
    /// (`LEADER p m`).
    Go(PageId),
    /// Put the file, or the folder whole, in the queue.
    Add,
    /// The folder above.
    Parent,
    /// The file (or folder), now: right after what plays, the queue kept.
    PlayNow,
    /// Empty the queue, after a question.
    Clear,
    /// The volume down a step.
    Quieter,
    /// The volume up a step.
    Louder,
    /// Only the picture, in this door.
    Only,
    /// The picture on the whole screen.
    Maximize,
    /// The picture in a small window in the corner.
    Pip,
    /// Mute or unmute.
    Mute,
    /// This door for the other — see [`Errand::Swap`].
    Swap,
    /// The media in the file: the page of the first kind there is a choice
    /// of, and from one of those pages the next (`player_tracks::next_page`).
    Tracks,
    /// Fold or unfold the group of the queue under the cursor.
    Fold,
    /// Another name for the entry under the cursor, or for what plays.
    Rename,
    /// The entry under the cursor — a file, a playlist, a set — one place
    /// earlier among its siblings.
    Earlier,
    /// And one place later.
    Later,
    /// Save what plays as a set — a name, then a folder.
    SaveSet,
    /// Save the queue — a name, then a folder.
    SaveQueue,
    /// Show or hide the lane under the cursor — what a click on a subtitle
    /// lane's chip does.
    Shown,
    /// What plays, on the whole of every screen something of it is routed
    /// to — pressed again, those windows close ([`Errand::EveryScreen`]).
    EveryScreen,
    /// THE METADATA of what plays: the home, the cursor on its section's
    /// first row (`LEADER p d`).
    Metadata,
    /// The search of a directory's page, typed (`/`).
    Search,
    /// The row that plays from a directory kept among the favourites, or
    /// taken off (`b`).
    Favourite,
    /// The globe of a directory's page turned: nearer, further, whole.
    Globe(crate::player_globe::Turn),
    /// Back one page, or leave the view from the home.
    Escape,
}

/// What a door that is NOT DRAWING THE ROWS does with a press — the window's
/// stage, the footer's picture alone (`o`) and the column's bar: the ONE
/// answer the three read ([`Press::unseen`]).
///
/// A key that acts on the row under the cursor, pressed where no row is drawn,
/// acted on a row nobody saw — `RET` played it, `d` took it off the queue —
/// and one that opens a page opened it behind the picture, where the click
/// on the media glyph seemed to do nothing and the next arrows switched the
/// sound unseen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unseen {
    /// It acts wherever the cursor is: play, the next, the volume, the
    /// stages, the swap — and `Esc`, which each door reads first.
    Acts,
    /// It acts on the row under the cursor: the rows come back instead, and
    /// the key is spent on that — the column's bar has always done this.
    ShowsTheRows,
    /// It opens what it shows — a page, the files, a question: the rows come
    /// back, and it opens there.
    OpensThere,
}

impl Press {
    /// What it does where the rows are not drawn ([`Unseen`]). Exhaustive: a
    /// new press is placed here or does not compile.
    #[must_use]
    pub const fn unseen(self) -> Unseen {
        match self {
            Self::Toggle
            | Self::Stop
            | Self::Next
            | Self::Previous
            | Self::Quieter
            | Self::Louder
            | Self::Only
            | Self::Maximize
            | Self::Pip
            | Self::Mute
            | Self::Swap
            | Self::EveryScreen
            | Self::Escape => Unseen::Acts,
            Self::Up
            | Self::Down
            | Self::Less
            | Self::More
            | Self::Enter
            | Self::Remove
            | Self::Add
            | Self::Parent
            | Self::PlayNow
            | Self::Fold
            | Self::Earlier
            | Self::Later
            | Self::Shown
            | Self::Favourite
            | Self::Globe(_) => Unseen::ShowsTheRows,
            Self::Browse
            | Self::Go(_)
            | Self::Tracks
            | Self::Clear
            | Self::Rename
            | Self::SaveSet
            | Self::SaveQueue
            | Self::Metadata
            | Self::Search => Unseen::OpensThere,
        }
    }
}

/// What a key does on a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A fact: nothing to do.
    Nothing,
    /// Play or pause.
    Playing,
    /// Where in the item: `←`/`→` seek, `RET` jumps ahead.
    Position,
    /// The volume: `←`/`→` turn it, `RET` mutes.
    Volume,
    /// The speed ring.
    Speed,
    /// The repeat ring.
    Repeat,
    /// Shuffle on or off.
    Shuffle,
    /// One of the file's media, on its kind's page: `RET` asks the player
    /// for it — `None` is no subtitles (`crate::player_tracks`).
    Track {
        /// Which kind.
        kind: TrackKind,
        /// mpv's id; `None` switches the subtitles off.
        id: Option<i64>,
    },
    /// An entry of the queue, by its id — which survives a move, so the
    /// cursor and "play this one" never land on another.
    Entry(EntryId),
    /// A GROUP of the queue — a named playlist, a set — by its id: `RET`
    /// plays it, `TAB` folds it, `→` unfolds it and `←` folds it
    /// (`crate::player_queue`).
    Group(EntryId),
    /// A group's triangle, at the head of its row: a click folds or unfolds
    /// it at once.
    Fold(EntryId),
    /// The glyph at the end of a row of the file's media: that one plays
    /// too, a lane of its own beside the one in use (`Ask::AddLane`).
    Also {
        /// Which kind.
        kind: TrackKind,
        /// mpv's id of the track.
        id: i64,
    },
    /// Another name for what plays — a question, the name it has on the line.
    Rename,
    /// Save what plays as a set: a name, a folder, and the question about a
    /// file already there.
    SaveSet,
    /// Save the queue, the same way.
    SaveQueue,
    /// A lane of what plays, on the page of playing together and under a
    /// set in the queue: `RET` opens its page, `v` shows or hides it, `d`
    /// takes it off.
    Lane(LaneId),
    /// A SUBTITLE LANE on the subtitles' page: `RET` opens its page, `←`/`→`
    /// move it between the foot of the picture and the top, `v` shows or
    /// hides it, `d` takes it off (`crate::player_subtitles`).
    Subtitle(LaneId),
    /// A subtitle lane's colour chip, at the head of its row: a click shows
    /// or hides the lane at once.
    Shown(LaneId),
    /// A subtitle file beside the film, named after it: `RET` lays it over
    /// the picture, a lane of its own.
    Beside(std::path::PathBuf),
    /// An ambient sound, by its id: `RET` turns it on or off, `←`/`→` its own
    /// volume (`crate::player_ambience`).
    Sound(String),
    /// Every ambient sound off, each keeping its volume.
    Silence,
    /// The ambient sounds that are on, held — or let go on.
    Hold,
    /// A lane's page: whether it plays at all — a switch.
    LaneOn(LaneId),
    /// A lane's page: where a subtitle's lines sit — a ring of two.
    LaneStack(LaneId),
    /// A lane's page: take it out of what plays.
    LaneRemove(LaneId),
    /// A lane's page: when it begins on the set's clock — `←`/`→` move it
    /// half a second, `RET` asks it typed (`player_timeline::parse_start`).
    LaneStart(LaneId),
    /// A lane's page: where in its file it stops — its end, or a time: the
    /// arrows move it half a second, `RET` asks it typed, `d` gives it its
    /// end back.
    LaneUntil(LaneId),
    /// A lane's page: its own name — `RET` asks it typed, holding the one it
    /// has.
    LaneTitle(LaneId),
    /// A lane's page: which picture a subtitle lane sits over — a ring of the
    /// films of what plays.
    LaneOver(LaneId),
    /// A lane's page: its own volume — a bar; `←`/`→` turn it, `RET` mutes.
    LaneVolume(LaneId),
    /// A lane's page: its own mute — a switch.
    LaneMute(LaneId),
    /// A lane's page: whether its picture — a film's, or a sound's cover —
    /// is shown among the pictures — a switch (`Ask::Arrange`).
    LaneShown(LaneId),
    /// A lane's page: its picture on every screen — a switch; off, it goes
    /// to the screen this door is on (`crate::player_lane`).
    LaneEveryScreen(LaneId),
    /// A lane's page: its picture on screen `1` (a connector) — a switch;
    /// the last screen it goes to is not taken away, and a screen it goes to
    /// that is not connected says so.
    LaneScreen(LaneId, String),
    /// A lane's page: the sound output it goes to — `None` the machine's
    /// default, else a sink's `device` (`State::sinks`).
    LaneSink(LaneId, Option<String>),
    /// The Layout page's ring of forms: the arrows and `RET` turn it
    /// (`crate::player_layout`).
    LayoutForm,
    /// A picture on the Layout page: `RET` shows or hides it, the arrows
    /// move it earlier or later.
    LayoutPicture(LaneId),
    /// The `◀` or `▶` at the end of a picture's row: it moves one place.
    LayoutMove {
        /// Which picture.
        lane: LaneId,
        /// Later, or earlier.
        later: bool,
    },
    /// The `↑` or `↓` at the end of the cursor's row of the queue: the entry
    /// — a file, a playlist, a set — one place earlier or later among its
    /// siblings. A group's arrows fold it; these move it.
    EntryMove {
        /// Which entry.
        id: EntryId,
        /// Later, or earlier.
        later: bool,
    },
    /// What plays on the whole of every screen it goes to — the home's row,
    /// and `M`.
    EveryScreen,
    /// The Metadata section's ring of the files of what plays: which one it
    /// shows (`crate::player_metadata`).
    MetaFile,
    /// Read the file shown again (`Ask::Refresh`).
    MetaRefresh,
    /// A field of the file shown that it can take: `RET` asks it — typed, or
    /// in the editor for one of several lines — and `d` clears it after a
    /// question.
    Tag(ronoctua_tags::Field),
    /// A field it cannot take here — an address, a program missing, a file
    /// read only: a fact, and `RET` says why.
    TagFixed(ronoctua_tags::Field),
    /// A tag of the Every-tag page the file can take, by who read it and
    /// its own name: `RET` asks its value — holding it when it is one line
    /// and the name's only value — and `d` takes it out after a question
    /// (`crate::player_metadata`).
    OtherTag(ronoctua_tags::Origin, String),
    /// A tag it cannot take here — read to be shown, data and not text, a
    /// track's own: a fact, and `RET` says why.
    OtherTagFixed(ronoctua_tags::Origin, String),
    /// "Add a tag" on the Every-tag page: its name, then its value, typed —
    /// into the file's main tag (`Sheet::adding`).
    AddTag,
    /// The Suggestions' ring of who is asked: MusicBrainz, Wikidata, TMDB
    /// (`crate::player_suggest`).
    SuggestSource,
    /// The row that asks a desk's search — its label names the hosts, and
    /// pressing it lets the player ask them (`crate::player_find`).
    Search(ronoctua_player::wire::DeskKind),
    /// The same search with other words, typed.
    Words(ronoctua_player::wire::DeskKind),
    /// A host a search was refused for, not being let: `RET` lets it and asks
    /// the search again.
    Let(String),
    /// A suggestion found for the file shown: `RET` opens its page.
    Candidate(ronoctua_player::find::CandidateId),
    /// A suggestion's field, or its cover, to be written or not — a toggle.
    Import(
        ronoctua_player::find::CandidateId,
        crate::player_find::ImportOf,
    ),
    /// Write what is turned on of a suggestion into the file.
    Write(ronoctua_player::find::CandidateId),
    /// The ring of the films of what plays that subtitles are searched for.
    FindFilm,
    /// A several-line field's page: `RET` opens it in the editor.
    Draft(ronoctua_tags::Field),
    /// A language subtitles are searched in: `RET` says so, `d` takes it out.
    Language(String),
    /// The glyph at the end of that row: a click takes the language out.
    LanguageOut(String),
    /// A language of the Languages' page: on or off.
    LanguagePick(String),
    /// Another language to search in, typed — its code or its name.
    AddLanguage,
    /// A subtitle found: `RET` downloads it beside the film and lays it over
    /// the picture.
    TakeSubtitle(ronoctua_player::find::CandidateId),
    /// Where lyrics taken go: a ring of two.
    LyricsTo,
    /// Lyrics found: `RET` takes them where the ring says.
    TakeLyrics(ronoctua_player::find::CandidateId),
    /// A switch of the Internet page: whether a press on a row naming these
    /// services lets the player ask them (`crate::player_internet`).
    Consent(&'static [ronoctua_lookup::Service]),
    /// A switch of the Internet page: whether a directory is let.
    LetDirectory(ronoctua_player::find::Directory),
    /// One of the person's keys: `RET` asks it on a masked line, `d` forgets
    /// it after a question.
    Key(ronoctua_lookup::keys::Key),
    /// The cover's page: its picture saved beside the file.
    CoverSave,
    /// The cover's page: its picture taken out of the file, after a
    /// question.
    CoverRemove,
    /// A paired machine this one can join, by its name: `RET` joins (or
    /// leaves), `←`/`→` turn the role, `d` forgets it.
    Host(String),
    /// A paired machine that can join this one: `d` forgets it.
    Guest(String),
    /// Show a pairing code.
    Invite,
    /// Lyrics asked of `lrclib.net`, on or off.
    LyricsOnline,
    /// Sponsored segments skipped, on or off.
    Sponsors,
    /// One of SponsorBlock's categories, skipped or not.
    Skip(&'static str),
    /// Pair with another machine: its name, then the code it shows.
    Pair,
    /// A page, opened over this one: the row is labelled by the page's own
    /// key (`PageId::key`), the one the page is titled by.
    Go(PageId),
    /// Which of the three places the files start from: a ring.
    Place,
    /// The folder above the one that is open.
    Up,
    /// Play the open folder, every file under it.
    PlayFolder,
    /// Put the open folder in the queue, every file under it.
    EnqueueFolder,
    /// A folder of the listing: `RET` enters it, `a` queues it whole.
    Folder(std::path::PathBuf),
    /// A file of the listing: `RET` puts it in the queue, `P` plays it alone.
    File(std::path::PathBuf),
    /// A path or an address, typed: a question the door asks.
    Typed,
    /// Empty the queue: the glyph beside its title, and `c`.
    ClearQueue,
    /// This door for the other: the glyph beside the "Now" title, in the
    /// footer — where there is no transport to carry it — and `w`.
    Swap,
    /// Close the player everywhere, after a question.
    Quit,
    /// Start the player again — one OLDER than this door, which cannot speak
    /// to it — after a question.
    Restart,
    /// An answer to the question on screen.
    Answer(bool),
    /// A directory's list, ASKED: the row that names its host — the press
    /// allows the directory, then asks — and "Try again" after a refusal
    /// (`crate::player_directory`).
    Asks(ronoctua_player::find::Query),
    /// A directory's list OPENED on a page of its own: a country's stations
    /// or channels, a genre's, a language's.
    Opens(ronoctua_player::find::Query),
    /// A directory's search by name, typed — `/` too.
    SearchDirectory(ronoctua_player::wire::DeskKind),
    /// Every row back, where a group of the globe's marks narrowed them.
    EveryRow,
    /// Another page of a theme's films, by its number.
    FilmsPage(u32),
    /// Another page of a list of stations, by its number.
    StationsPage(u32),
    /// Something that plays from a directory — a station, a channel, a film,
    /// a live — by its kind and its id: `RET` plays it now, `a` adds it to the
    /// queue, `b` keeps it among the favourites.
    Pick(ronoctua_broadcast::picks::Kind, String),
    /// The star at the head of such a row: a click keeps it among the
    /// favourites, or takes it off.
    Star(ronoctua_broadcast::picks::Kind, String),
}

impl Target {
    const fn turns(&self) -> bool {
        matches!(
            self,
            Self::Playing
                | Self::Position
                | Self::Volume
                | Self::Speed
                | Self::Repeat
                | Self::Shuffle
                // THE HOME'S ROW OF A SOUND OR OF THE SUBTITLES opens their
                // page on `RET` and turns the one in use on the arrows, the
                // ring it always was.
                | Self::Go(PageId::Tracks(TrackKind::Sound | TrackKind::Subtitles))
                | Self::Subtitle(_)
                | Self::Sound(_)
                | Self::Hold
                | Self::LaneOn(_)
                | Self::LaneStack(_)
                // A LANE, wherever its row is — the timeline's, playing
                // together, a set's in the queue: the arrows move when it
                // begins, half a second a step.
                | Self::Lane(_)
                | Self::LaneStart(_)
                | Self::LaneUntil(_)
                | Self::LaneOver(_)
                | Self::LaneVolume(_)
                | Self::LaneMute(_)
                | Self::LaneShown(_)
                | Self::LaneEveryScreen(_)
                | Self::LaneScreen(..)
                // THE LAYOUT'S RING, and a picture's place: the arrows turn
                // the form and move the picture.
                | Self::LayoutForm
                | Self::LayoutPicture(_)
                // WHICH FILE the Metadata section shows: a ring — and who is
                // asked for its suggestions, which film the subtitles are for,
                // where lyrics go, a suggestion's toggles and the Internet
                // page's switches.
                | Self::MetaFile
                | Self::SuggestSource
                | Self::FindFilm
                | Self::LanguagePick(_)
                | Self::LyricsTo
                | Self::Import(..)
                | Self::Consent(_)
                | Self::LetDirectory(_)
                | Self::LyricsOnline
                | Self::Sponsors
                | Self::Skip(_)
                | Self::Host(_)
                | Self::Place
        )
    }

    const fn among_the_files(&self) -> bool {
        matches!(
            self,
            Self::Place
                | Self::Up
                | Self::PlayFolder
                | Self::EnqueueFolder
                | Self::Folder(_)
                | Self::File(_)
                | Self::Typed
        )
    }

    /// Whether a key does something on it — a field the file cannot take
    /// is a FACT, drawn as one, though `RET` on it says why.
    pub(crate) const fn acts(&self) -> bool {
        !matches!(
            self,
            Self::Nothing | Self::TagFixed(_) | Self::OtherTagFixed(..)
        )
    }
}

/// What a choice of the files is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Purpose {
    /// Into the queue: `RET` adds, `P` plays alone.
    #[default]
    Queue,
    /// Something played together with what plays: a LANE of it — a picture,
    /// a sound, subtitles.
    Lane,
    /// One of the player's own files to open: a set plays now, after what
    /// plays; a queue takes the queue's place, after a question — the ONE
    /// place the queue is replaced.
    Open,
    /// A subtitle file to lay over the picture — any file the person picks,
    /// the ones whose name says nothing included: a subtitle lane.
    Subtitle,
    /// A picture for the cover of the file the Metadata section shows
    /// (`crate::player_metadata`).
    Cover,
}

impl Purpose {
    /// What its page lists besides the folders.
    #[must_use]
    pub const fn wanted(self) -> crate::player_browse::Wanted {
        use crate::player_browse::Wanted;
        match self {
            Self::Queue => Wanted::Media,
            Self::Lane => Wanted::Lanes,
            Self::Open => Wanted::Saved,
            Self::Subtitle => Wanted::Subtitles,
            Self::Cover => Wanted::Images,
        }
    }
}

/// A yes-or-no question on screen, and what each answer does.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Choice {
    /// What is asked, already in words.
    sentence: String,
    /// Where the cursor was, to go back to once it is answered.
    back: (usize, usize),
    /// What `yes` does.
    yes: Box<Errand>,
    /// What `no` does — and `Esc`.
    no: Box<Errand>,
    /// The saved draft it asks about, when it does: the question goes when
    /// the draft does — another door answered it, or it was written.
    draft: Option<ronoctua_player::wire::DraftId>,
    /// What it would write, as the editor saved it, under the answers.
    quoted: Vec<String>,
}

/// One switch of the player's preferences, as a press turns it: what
/// [`Errand::Keep`] lays over the file as it is on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Switch {
    /// Lyrics from LRCLIB when a song has none, on or off.
    LyricsOnline(bool),
    /// SponsorBlock's parts skipped, on or off.
    Sponsors(bool),
    /// One of SponsorBlock's categories skipped, or not.
    Skip {
        /// The category.
        category: String,
        /// Skipped.
        on: bool,
    },
}

impl Errand {
    /// Whether it writes `player.json` — a consent, a switch — alone or
    /// before or after something else.
    #[must_use]
    pub fn writes_prefs(&self) -> bool {
        match self {
            Self::Let { .. } | Self::Keep(_) => true,
            Self::Then(first, then) => first.writes_prefs() || then.writes_prefs(),
            _ => false,
        }
    }
}

impl Switch {
    /// Laid over `prefs`.
    pub fn apply(&self, prefs: &mut ronoctua_player::prefs::Preferences) {
        match self {
            Self::LyricsOnline(on) => prefs.lyrics_online = *on,
            Self::Sponsors(on) => prefs.sponsorblock = *on,
            Self::Skip { category, on } => {
                prefs.skipped.retain(|c| c != category);
                if *on {
                    prefs.skipped.push(category.clone());
                }
            }
        }
    }
}

/// What the door has to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// Draw again.
    Nothing,
    /// Shut the view.
    Close,
    /// Ask a line of the door — see `player_question::Question`.
    Ask(Question),
    /// Pair with another machine: its name and the code it shows.
    Pair {
        /// Its name on the tailnet.
        to: String,
        /// The code, a secret.
        code: crate::player_machines::Code,
    },
    /// Tell the player.
    Tell(Ask),
    /// Tell the player, and say what was done — the sentence already in
    /// words.
    TellSaying(Ask, String),
    /// Say a sentence, already in words.
    Said(String),
    /// Turn ONE switch of the preferences — MERGED into `player.json` as it
    /// is on disk, never this door's whole copy of it (another door may have
    /// let or withdrawn something since this one read it) — and tell the
    /// player to reread them.
    Keep(Switch),
    /// Show the picture alone, or on the whole screen — the DOOR's to do,
    /// because only a door has a picture to show.
    Show(Show),
    /// THIS DOOR FOR THE OTHER — the door's to do, because only it knows
    /// which it is: a window raises the column and closes (a stage's window
    /// too, which closes the stage); the column opens the window and closes;
    /// the footer shuts its view and opens the window. RAISED, never toggled:
    /// a column already up stays up (`RAISE`).
    Swap,
    /// CLOSE THE PLAYER EVERYWHERE — the door's to do, because only it
    /// knows what closing is there: the footer tells the daemon, shuts its
    /// view and ends the column and a stage's window; a window or the column
    /// tells the daemon too, hands the footer the same order, and closes.
    /// The daemon's farewell closes every other door.
    QuitEverywhere,
    /// Start the player again: the one that runs is older than this door
    /// ([`State::protocol`]), closed through MPRIS and started anew, off the
    /// loop ([`restart`]). Every door's watch finds the new one by itself.
    Restart,
    /// LET the player ask these services and list from these directories —
    /// `on` — or stop letting it: MERGED into `player.json` as it is on disk
    /// (another door may have let others meanwhile), and then `then` carried
    /// out: a press on a row that names the hosts is the consent, and what it
    /// asks goes after it (`crate::player_find`).
    Let {
        /// The services.
        services: Vec<ronoctua_lookup::Service>,
        /// The directories.
        directories: Vec<ronoctua_player::find::Directory>,
        /// Let, or no longer.
        on: bool,
        /// What is done once it is written.
        then: Box<Errand>,
    },
    /// Keep one of the person's keys in its private file
    /// (`ronoctua_lookup::keys`), and have the player try it at once — off
    /// the loop, the door saying whether the service took it.
    KeepKey {
        /// Which.
        key: ronoctua_lookup::keys::Key,
        /// What was typed — never printed.
        secret: ronoctua_lookup::Secret,
    },
    /// Keep the OpenSubtitles account, and have it tried.
    KeepAccount {
        /// The user name.
        user: String,
        /// Its password — never printed.
        password: ronoctua_lookup::Secret,
    },
    /// Forget one of the person's keys.
    ForgetKey(ronoctua_lookup::keys::Key),
    /// ONE ERRAND, THEN ANOTHER — both reaching past the door: a page that
    /// came up asks for its lists after what the key did
    /// (`player_directory::View::owing`).
    Then(Box<Errand>, Box<Errand>),
    /// WHAT PLAYS ON THE WHOLE OF EVERY SCREEN something of it is routed to:
    /// a window a screen, each made whole-screen on its own output — or,
    /// pressed while they are up, those windows closed
    /// (`player_stage::everywhere`). The door's to do: only a door knows the
    /// screens by name.
    EveryScreen,
}

/// What a door does with what the player's watch heard — the ONE answer for
/// the footer, the window and the column.
#[derive(Debug, Clone, PartialEq)]
pub enum Hearing {
    /// The player's state to show — `None`, nobody answers: the door stays
    /// and says the player is not running. A crash is not a farewell.
    Shows(Option<Box<State>>),
    /// The queue's outline, whole: the door KEEPS it beside the state and
    /// draws the queue from it ([`View::outline_arrived`]) — the state says
    /// only which entry is current.
    Outline(Box<Outline>),
    /// The player closed everywhere: the door closes.
    Closes,
}

/// What `heard` means to a door ([`Hearing`]).
#[must_use]
pub fn hearing(heard: ronoctua_player::client::Heard) -> Hearing {
    use ronoctua_player::client::Heard;
    match heard {
        Heard::State(s) => {
            remember_sinks(&s.sinks);
            Hearing::Shows(Some(s))
        }
        Heard::Queue(o) => Hearing::Outline(o),
        Heard::Gone => Hearing::Shows(None),
        Heard::Farewell => Hearing::Closes,
    }
}

/// How an ask told OFF THE LOOP ended ([`tell_then`]) — what the door that
/// told it says when the answer comes back.
#[derive(Debug, Clone, PartialEq)]
pub enum Told {
    /// Taken.
    Taken,
    /// Refused, and why.
    Refused(Trouble),
    /// Nobody answered — or the player answered with its farewell: it was
    /// leaving, and took nothing.
    NotRunning,
    /// The player is not on this machine.
    NotInstalled,
    /// The player took the connection and closed it WITHOUT A WORD — what an
    /// OLDER player does with an ask it cannot read. One of this build
    /// answers even that (`Trouble::NotUnderstood`).
    Unheard,
    /// A several-line field's draft, where the daemon wrote it
    /// ([`Ask::Draft`]): the door opens it in the editor.
    Draft(std::path::PathBuf),
    /// The service took the key the person just kept (`Ask::KeyCheck`).
    KeyTaken(ronoctua_lookup::Service),
    /// No word yet when the teller stopped waiting: the player is STILL AT IT
    /// — a folder put in the queue is walked before the answer. Nothing is
    /// said: what it did arrives in the state and the queue once it is done,
    /// and "not running" would be untrue.
    Working,
    /// A key of the person's could not be kept or forgotten — the thread that
    /// writes it says why.
    KeyTrouble(ronoctua_lookup::keys::KeyTrouble),
    /// How an ask for a DIRECTORY'S LIST ended — `answer` — handed to the view
    /// with the list it was for: a list refused at once (a directory not
    /// allowed, no yt-dlp for the lives, no such theme) never reaches its
    /// desk, and its page says so in place of "asking" (`player_directory`).
    Listing {
        /// The list asked for.
        query: ronoctua_player::find::Query,
        /// How the ask ended.
        answer: Box<Told>,
    },
}

/// What a door says of `told`: the refusal, in a sentence, or nothing when it
/// was taken. `older`: the door's last state is an older player's
/// ([`older`]) — an ask it dropped unheard is said as that, which its home
/// says too, and not as a player that does not run.
#[must_use]
pub fn told_sentence(told: &Told, older: bool, texts: &ronoctua_i18n::Texts) -> Option<String> {
    use ronoctua_i18n::key;
    match told {
        Told::Taken | Told::Working | Told::Draft(_) => None,
        Told::Listing { answer, .. } => told_sentence(answer, older, texts),
        Told::KeyTrouble(t) => Some(crate::player_internet::key_trouble(*t, texts)),
        Told::KeyTaken(service) => Some(texts.t_with(
            key::PLAYER_KEY_ACCEPTED,
            &[("service", &texts.t(service_key(*service)))],
        )),
        Told::Refused(t) => Some(trouble_sentence(t, texts)),
        Told::Unheard if older => Some(texts.t(key::PLAYER_OLD_DAEMON)),
        Told::Unheard | Told::NotRunning => Some(texts.t(key::PLAYER_NOT_RUNNING)),
        Told::NotInstalled => Some(texts.t(key::PLAYER_NOT_INSTALLED)),
    }
}

/// Whether what `s` says plays is there: a leaf of the queue is current, or
/// this machine plays in step with another — whose leaf is not one of this
/// queue's, so `current` is `None` then.
#[must_use]
pub fn loaded(s: &State) -> bool {
    s.current.is_some() || s.joined.is_some()
}

/// Whether the AMBIENT SOUNDS are all that plays: nothing of the queue is
/// loaded — no leaf current, no other machine followed — and a sound is on
/// (`State::only_ambience`, and the machine followed on top). Then the
/// modeline's pair and every door's toggle are the ambience's: with a leaf
/// current, even stopped, they are the queue's, and a click never holds the
/// rain in place of playing the song.
#[must_use]
pub fn only_ambience(s: &State) -> bool {
    s.joined.is_none() && s.only_ambience()
}

/// What PLAY/PAUSE asks — `SPC`, the transport's button, `LEADER p t` and the
/// modeline's glyph, the one answer each reads: the ambient sounds held or
/// let go on while they are all that plays ([`only_ambience`]), and the
/// queue's toggle otherwise. The media keys never come here: they speak to
/// the player through MPRIS, which toggles the queue only.
#[must_use]
pub fn toggle_ask(s: Option<&State>) -> Ask {
    if s.is_some_and(only_ambience) {
        Ask::AmbientToggle
    } else {
        Ask::Toggle
    }
}

/// Whether what plays has a PICTURE of a film — a lane of pictures, which
/// subtitles go over. A cover is no such lane.
#[must_use]
pub fn has_a_picture(s: &State) -> bool {
    s.lanes
        .iter()
        .any(|l| l.kind == ronoctua_player::own::LaneKind::Video)
}

/// Whether the player that answers is OLDER than this door: its state says a
/// protocol before this one's, or none at all. It may not understand a word
/// this door says — an ask it cannot read, it drops unanswered — so the door
/// says so and offers to start it again ([`Errand::Restart`]).
#[must_use]
pub const fn older(s: &State) -> bool {
    s.protocol < ronoctua_player::wire::PROTOCOL
}

/// The argument that makes `player-column` RAISE the column and never close
/// it — what a window asks the footer for when it swaps itself for the
/// column. Without it the command is the toggle it always was.
pub const RAISE: &str = "raise";

/// How the picture is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Show {
    /// Alone in the door, everything else hidden.
    Only,
    /// Alone, on the whole screen.
    Maximized,
    /// Alone, in a small window floating in the bottom-right corner —
    /// picture-in-picture.
    Small,
}

/// How far `←`/`→` move the position, in seconds.
pub const SEEK_STEP: f64 = 10.0;
/// How far `RET` on the position jumps, in seconds.
pub const JUMP_STEP: f64 = 60.0;
/// How far `←`/`→` move the volume, in percent.
pub const VOLUME_STEP: u8 = 5;
/// The loudest the volume ring goes: mpv amplifies past 100.
pub const VOLUME_MOST: u8 = 130;
/// The speeds the ring walks.
pub const SPEEDS: [f64; 7] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
/// How many lines of lyrics the view shows around the current one.
pub const LYRICS_AROUND: usize = 2;

/// The view: a stack of pages over the home, and what every page shares.
#[derive(Debug)]
pub struct View {
    /// The pages, the home first — never empty: the home is never popped
    /// (`crate::player_page`).
    pub(crate) levels: Vec<Level>,
    /// `player.json`, read when the view opens and kept in step with what
    /// this door writes.
    pub(crate) prefs: ronoctua_player::prefs::Preferences,
    /// Where the files were left: opening them again goes back there.
    left_at: Option<(usize, std::path::PathBuf)>,
    /// The folder just left by going up, for the cursor to land on.
    came_from: Option<std::path::PathBuf>,
    /// The yes-or-no question on screen, when one is.
    asking: Option<Choice>,
    /// A control on a title's line, clicked: what the next `RET` does.
    clicked_control: Option<Target>,
    /// The home the places are in, instead of this machine's own folders —
    /// a photograph's ([`View::blank`]).
    home: Option<std::path::PathBuf>,
    /// The swap's glyph is on the "Now" title — the footer's view, while it
    /// has no transport to carry it ([`View::in_the_footer`]).
    pub(crate) swap: bool,
    /// The footer's media panel stands beside the rows, and its transport
    /// carries the swap: the "Now" title does not carry a second one
    /// ([`View::beside_the_panel`]).
    pub(crate) panel: bool,
    /// The footer's region is too narrow for the picture beside the rows
    /// (`player_media::footer_panel`): the home says so under "Now" — a
    /// refusal is a sentence — and how to see it alone. The footer's to say
    /// ([`View::picture_refused`]): only it knows the width of the screen.
    pub(crate) no_room: bool,
    /// The queue's outline, as the door last heard it — the queue is drawn
    /// from it, and the state says only which entry is current
    /// ([`View::outline_arrived`]).
    pub(crate) outline: Option<Box<Outline>>,
    /// Which groups of the queue are open, by id — each one a person folded
    /// or unfolded, and each one what plays entered (`crate::player_queue`).
    pub(crate) folds: std::collections::BTreeMap<EntryId, bool>,
    /// The entry that was current when the rows last settled: a new one
    /// opens the groups it is in.
    pub(crate) last_current: Option<EntryId>,
    /// A question of the view's own on the door's line, and what its answer
    /// is about ([`View::answered`]).
    pub(crate) typing: Option<Typing>,
    /// The subtitle files beside the film that plays, named after it — read
    /// once per film, while the subtitles' page is open
    /// (`crate::player_subtitles::Beside`).
    pub(crate) beside: crate::player_subtitles::Beside,
    /// What wakes the door once a read it started off its loop is done —
    /// the folder beside the film. `None`: the door has no loop (a
    /// photograph, a test), and reads in place.
    pub(crate) wake: Option<crate::player_subtitles::Wake>,
    /// A SET's lanes are drawn in "Now" as the deck's rows — the footer's
    /// view ([`View::in_the_footer`]), which has no block of its own to draw
    /// them in: the window and the column draw the deck under their
    /// transport (`player_window`), and "Now" keeps its position there.
    pub(crate) deck_in_now: bool,
    /// The screens the door knows by name, and the one it is on — what a
    /// lane's page routes a picture to (`crate::player_lane`). The door's to
    /// say ([`View::screens_are`]): a view knows no output.
    pub(crate) screens: Vec<crate::player_lane::Known>,
    /// The screen the door is on, when it knows it.
    pub(crate) here: Option<String>,
    /// The box the door lays its pictures out in, in logical pixels, when it
    /// shows any — the door's to say ([`View::pictures_room_is`]): the Layout
    /// page draws what THIS door shows, and says which picture it leaves out
    /// for being smaller than a picture is placed at.
    pub(crate) pictures_room: Option<(f32, f32)>,
    /// THE METADATA of what plays: which file, the sheet held of it, the one
    /// being fetched, the drafts asked about (`crate::player_metadata`).
    pub(crate) meta: crate::player_metadata::Meta,
    /// THE LOOKUPS' DESKS as this door heard them, and the suggested covers
    /// decoded (`crate::player_find`).
    pub(crate) found: crate::player_find::Found,
    /// What the view chose for its searches: who suggests, the languages, the
    /// film, where lyrics go, a suggestion's toggles.
    pub(crate) finding: crate::player_find::Choices,
    /// Which of the person's keys are kept — whether, never what
    /// (`crate::player_internet`).
    pub(crate) kept: crate::player_internet::Kept,
    /// THE DIRECTORIES: the lists owed to the player, the favourites and the
    /// played lately (`crate::player_directory`).
    pub(crate) directories: crate::player_directory::Directories,
    /// THE GLOBE of a directory's page (`crate::player_globe`).
    pub(crate) globe: crate::player_globe::Seen,
    /// `player.json`, watched: another door that lets, withdraws or turns
    /// something is heard here, and [`View::prefs`] read again — THE FILE IS
    /// THE BROADCAST (`player_internet::Watched`).
    pub(crate) prefs_on_disk: crate::player_internet::Watched,
    /// The preferences are read from this machine's file — a door on a
    /// screen; a photograph's and a test's view keeps what it was given.
    reads_prefs: bool,
}

/// Which of the player's own files is being saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saved {
    /// What plays, as a set (`.rset`).
    Set,
    /// The queue (`.rqueue`).
    Queue,
}

/// A question of the view's own being typed, and what its answer is about —
/// the entry a new name is for, the name a file is saved under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Typing {
    /// A new name for entry `id`, which was called `was`.
    Rename {
        /// The entry.
        id: EntryId,
        /// Its name when the question opened: answered unchanged, nothing
        /// changes.
        was: String,
    },
    /// The name a set or the queue is saved under.
    SaveName(Saved),
    /// The folder it is saved in, the name already typed.
    SaveFolder(Saved, String),
    /// When lane `0` begins, typed on its page.
    LaneStart(LaneId),
    /// Where in its file lane `0` stops, typed on its page.
    LaneUntil(LaneId),
    /// A value for a field of a file, typed on the Metadata section
    /// (`player_metadata::Typed`).
    Meta(crate::player_metadata::Typed),
    /// Other words for a desk's search.
    Words(ronoctua_player::wire::DeskKind),
    /// A language to search subtitles in.
    Language,
    /// One of the person's keys, masked.
    Key(ronoctua_lookup::keys::Key),
    /// The OpenSubtitles account's user name.
    AccountUser,
    /// Its password, masked, the user name already typed.
    AccountPassword(String),
    /// Words to search a directory's desk by: stations, channels, films.
    Search(ronoctua_player::wire::DeskKind),
    /// A new name for lane `id`, which was called `was`.
    LaneTitle {
        /// The lane.
        id: LaneId,
        /// Its name when the question opened: answered unchanged, nothing
        /// changes.
        was: String,
    },
}

/// A page of the files: which place, and the folder that is open.
#[derive(Debug, Clone)]
pub struct Files {
    place: usize,
    listing: crate::player_browse::Listing,
}

/// `m:ss`, or `h:mm:ss` from an hour on.
#[must_use]
pub fn clock(seconds: f64) -> String {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let s = seconds.max(0.0).floor() as u64;
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// What the queue calls an entry, in words — the ONE place a door words a
/// [`Name`]: a title as it is, a location by its last piece
/// (`queue::file_name`), and a set nobody named as every source it plays,
/// joined by `player-set-joiner` and cut to `width_cells` by grapheme
/// (`queue::together`) — never one file's name for several.
#[must_use]
pub fn name_of(name: &Name, texts: &ronoctua_i18n::Texts, width_cells: u16) -> String {
    match name {
        Name::Titled(title) => title.clone(),
        Name::Location(location) => {
            ronoctua_player::queue::file_name(location.trim_end_matches('/')).to_owned()
        }
        Name::Together(parts) => ronoctua_player::queue::together(
            parts,
            &texts.t(ronoctua_i18n::key::PLAYER_SET_JOINER),
            width_cells,
        ),
    }
}

/// How wide a name said in a sentence may run, in cells: WHOLE. A hint under
/// the pointer and a title card are cut or wrapped by the surface that draws
/// them, to its own width — a width chosen here would be a size written by
/// hand (invariant 9).
pub const NAME_IN_A_SENTENCE: u16 = u16::MAX;

/// The entry `id` of the outline, wherever it is in the tree.
#[must_use]
pub fn node_of(outline: &Outline, id: EntryId) -> Option<&Node> {
    fn find(nodes: &[Node], id: EntryId) -> Option<&Node> {
        nodes.iter().find_map(|n| {
            if n.id == id {
                Some(n)
            } else {
                find(&n.children, id)
            }
        })
    }
    find(&outline.entries, id)
}

/// What is playing, in the words the modeline's mark says under the pointer
/// and the home's first row: the name the queue gives the current entry —
/// worded here, from the outline, so a set is its sources joined in this
/// language — else the title the player read, else where it plays.
#[must_use]
pub fn now_playing(
    state: &State,
    outline: Option<&Outline>,
    texts: &ronoctua_i18n::Texts,
) -> Option<String> {
    if !loaded(state) {
        return None;
    }
    let node = state
        .current
        .zip(outline)
        .and_then(|(id, o)| node_of(o, id));
    // A SET IS EVERY SOURCE IT PLAYS: the artist the player read is one
    // file's tag, and in front of the set's name it named the set after that
    // one file after all.
    let a_set = node.is_some_and(|n| n.kind == ronoctua_player::queue::NodeKind::Set);
    let named = node.map(|n| name_of(&n.name, texts, NAME_IN_A_SENTENCE));
    let title = named
        .or_else(|| state.title.clone())
        .filter(|t| !t.trim().is_empty())
        .or_else(|| {
            state
                .location
                .as_deref()
                .map(|l| name_of(&Name::Location(l.to_owned()), texts, NAME_IN_A_SENTENCE))
        })?;
    Some(
        match state
            .artist
            .as_deref()
            .filter(|a| !a_set && !a.trim().is_empty())
        {
            Some(a) => format!("{a} — {title}"),
            None => title,
        },
    )
}

/// A number a sentence counts with: a plural's branch needs a NUMBER
/// (`numeric-selectors-get-numbers`).
fn number(n: usize) -> ronoctua_i18n::Arg<'static> {
    ronoctua_i18n::Arg::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

/// A trouble, in a sentence — one for every variant the player can say.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn trouble_sentence(t: &Trouble, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::{Arg, key};
    let with = |k: &str, args: &[(&str, &str)]| texts.t_with(k, args);
    match t {
        Trouble::Unplayable { why } => with(key::PLAYER_TROUBLE_UNPLAYABLE, &[("why", why)]),
        Trouble::NoDecoder { program } => {
            with(key::PLAYER_TROUBLE_NO_DECODER, &[("program", program)])
        }
        Trouble::Unsupported { format } => {
            with(key::PLAYER_TROUBLE_UNSUPPORTED, &[("format", format)])
        }
        Trouble::NoEngine { why } => with(key::PLAYER_TROUBLE_NO_ENGINE, &[("why", why)]),
        Trouble::Remote { why } => with(key::PLAYER_TROUBLE_REMOTE, &[("why", why)]),
        Trouble::NotUnderstood { why } => with(key::PLAYER_TROUBLE_NOT_UNDERSTOOD, &[("why", why)]),
        Trouble::TooMany(too) => texts.t_args(
            match too.what {
                Limit::Leaves => key::PLAYER_TROUBLE_TOO_MANY_LEAVES,
                Limit::Depth => key::PLAYER_TROUBLE_TOO_MANY_DEPTH,
                Limit::Lanes => key::PLAYER_TROUBLE_TOO_MANY_LANES,
                Limit::Subtitles => key::PLAYER_TROUBLE_TOO_MANY_SUBTITLES,
                Limit::SubtitlePictures => key::PLAYER_TROUBLE_TOO_MANY_SUBTITLE_PICTURES,
                Limit::Voices => key::PLAYER_TROUBLE_TOO_MANY_VOICES,
                Limit::Pictures => key::PLAYER_TROUBLE_TOO_MANY_PICTURES,
                Limit::Ambience => key::PLAYER_TROUBLE_TOO_MANY_AMBIENCE,
                Limit::Drafts => key::PLAYER_TROUBLE_TOO_MANY_DRAFTS,
                Limit::Writes => key::PLAYER_TROUBLE_TOO_MANY_WRITES,
                Limit::Takes => key::PLAYER_TROUBLE_TOO_MANY_TAKES,
            },
            &[("most", number(too.most))],
        ),
        Trouble::NoSuchEntry { .. } => texts.t(key::PLAYER_TROUBLE_NO_SUCH_ENTRY),
        Trouble::NotAPlaylist { .. } => texts.t(key::PLAYER_TROUBLE_NOT_A_PLAYLIST),
        Trouble::IntoItself => texts.t(key::PLAYER_TROUBLE_INTO_ITSELF),
        Trouble::Nameless => texts.t(key::PLAYER_TROUBLE_NAMELESS),
        Trouble::Unreadable { path } => with(key::PLAYER_TROUBLE_UNREADABLE, &[("path", path)]),
        Trouble::Unwritable { path, why } => with(
            key::PLAYER_TROUBLE_UNWRITABLE,
            &[("path", path), ("why", why)],
        ),
        Trouble::NowhereToSave { path } => {
            with(key::PLAYER_TROUBLE_NOWHERE_TO_SAVE, &[("path", path)])
        }
        Trouble::NoRuntimeDir => texts.t(key::PLAYER_TROUBLE_NO_RUNTIME_DIR),
        Trouble::TooLarge { path } => with(key::PLAYER_TROUBLE_TOO_LARGE, &[("path", path)]),
        Trouble::NotOurs { path } => with(key::PLAYER_TROUBLE_NOT_OURS, &[("path", path)]),
        Trouble::Newer { path, found } => texts.t_args(
            key::PLAYER_TROUBLE_NEWER,
            &[
                ("path", Arg::Text(path)),
                ("found", Arg::Number(i64::from(*found))),
            ],
        ),
        Trouble::Damaged { path } => with(key::PLAYER_TROUBLE_DAMAGED, &[("path", path)]),
        Trouble::Empty { path } => with(key::PLAYER_TROUBLE_EMPTY, &[("path", path)]),
        Trouble::Missing { program } => with(key::PLAYER_TROUBLE_MISSING, &[("program", program)]),
        Trouble::SinkGone { device } => with(
            key::PLAYER_TROUBLE_SINK_GONE,
            &[("device", &sink_label(device, &[], texts))],
        ),
        Trouble::Unreachable { what, why } => with(
            key::PLAYER_TROUBLE_UNREACHABLE,
            &[("what", what), ("why", why)],
        ),
        Trouble::StreamDown { title, tried } => texts.t_args(
            key::PLAYER_TROUBLE_STREAM_DOWN,
            &[("title", Arg::Text(title)), ("tried", number(*tried))],
        ),
        // A THEME'S LIVES are said by the theme's name in the person's
        // language — the daemon sends its id — and anything else as it is
        // (a playlist's address).
        Trouble::Listing { what, why } => {
            let what = ronoctua_broadcast::table::Category::of(what)
                .map_or_else(|| what.clone(), |c| texts.t(c.key));
            with(listing_key(*why), &[("what", &what)])
        }
        Trouble::Subtitle { path, why } => with(
            key::PLAYER_TROUBLE_SUBTITLE,
            &[("path", path), ("why", &subtitle_refusal(why, texts))],
        ),
        Trouble::NoPicture { path } => with(key::PLAYER_TROUBLE_NO_PICTURE, &[("path", path)]),
        Trouble::NothingPlaying => texts.t(key::PLAYER_TROUBLE_NOTHING_PLAYING),
        Trouble::NoSuchLane { .. } => texts.t(key::PLAYER_TROUBLE_NO_SUCH_LANE),
        Trouble::WrongKind { .. } => texts.t(key::PLAYER_TROUBLE_WRONG_KIND),
        Trouble::BadValue { .. } => texts.t(key::PLAYER_TROUBLE_BAD_VALUE),
        Trouble::LastLane => texts.t(key::PLAYER_TROUBLE_LAST_LANE),
        Trouble::Following { host } => with(key::PLAYER_TROUBLE_FOLLOWING, &[("host", host)]),
        Trouble::StreamPlaysAlone => texts.t(key::PLAYER_TROUBLE_STREAM_PLAYS_ALONE),
        Trouble::NoAmbience => texts.t(key::PLAYER_TROUBLE_NO_AMBIENCE),
        Trouble::NoSuchSound { id } => with(key::PLAYER_TROUBLE_NO_SUCH_SOUND, &[("id", id)]),
        Trouble::SoundUnplayable { id, why } => with(
            key::PLAYER_TROUBLE_SOUND_UNPLAYABLE,
            &[("sound", &ambience_name(id, texts)), ("why", why)],
        ),
        // A FILE'S TAGS: which file, and why — the why is also a row's own
        // sentence on the Metadata section, where the file is the page's.
        Trouble::Tags { path, why } => with(
            key::PLAYER_TROUBLE_TAGS,
            &[
                (
                    "name",
                    &name_of(&Name::Location(path.clone()), texts, NAME_IN_A_SENTENCE),
                ),
                ("why", &tags_refusal(why, texts)),
            ],
        ),
        Trouble::NoSuchDraft { .. } => texts.t(key::PLAYER_TROUBLE_NO_SUCH_DRAFT),
        Trouble::Lookup(r) => lookup_refusal(*r, texts),
        Trouble::Off { host } => with(key::PLAYER_TROUBLE_OFF, &[("host", host)]),
        Trouble::Exists { path } => with(key::PLAYER_TROUBLE_EXISTS, &[("path", path)]),
        Trouble::NoSuchCandidate { .. } => texts.t(key::PLAYER_TROUBLE_NO_SUCH_CANDIDATE),
        Trouble::Directory(r) => directory_refusal(*r, texts),
        Trouble::NoSuchCategory { .. } => texts.t(key::PLAYER_TROUBLE_NO_SUCH_CATEGORY),
        Trouble::NoSuchPick { .. } => texts.t(key::PLAYER_TROUBLE_NO_SUCH_PICK),
        Trouble::Picks { why } => texts.t({
            use ronoctua_broadcast::picks::Trouble as Why;
            match why {
                Why::NoHome => key::PLAYER_TROUBLE_PICKS_NO_HOME,
                Why::Unreadable => key::PLAYER_TROUBLE_PICKS_UNREADABLE,
                Why::Newer => key::PLAYER_TROUBLE_PICKS_NEWER,
                Why::Unwritable => key::PLAYER_TROUBLE_PICKS_UNWRITABLE,
            }
        }),
    }
}

/// Why a file's tags cannot be read or written, in a sentence — a refusal's
/// second half, and the sentence under a row of the Metadata section that
/// cannot be changed (`crate::player_metadata`). One for each reason.
#[must_use]
pub fn tags_refusal(why: &ronoctua_tags::Why, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    use ronoctua_tags::Why;
    let size = |bytes: u64| crate::files::size_text(bytes, texts.decimal_mark());
    match why {
        Why::NotAFile => texts.t(key::PLAYER_TROUBLE_TAG_NOT_A_FILE),
        Why::Gone => texts.t(key::PLAYER_TROUBLE_TAG_GONE),
        Why::ChangedSince => texts.t(key::PLAYER_TROUBLE_TAG_CHANGED_SINCE),
        Why::HardLinked => texts.t(key::PLAYER_TROUBLE_TAG_HARD_LINKED),
        Why::ReadOnlyFolder => texts.t(key::PLAYER_TROUBLE_TAG_READ_ONLY_FOLDER),
        Why::ReadOnlyFile => texts.t(key::PLAYER_TROUBLE_TAG_READ_ONLY_FILE),
        Why::NotYours => texts.t(key::PLAYER_TROUBLE_TAG_NOT_YOURS),
        Why::NoRoom {
            need_bytes,
            free_bytes,
        } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_NO_ROOM,
            &[("need", &size(*need_bytes)), ("free", &size(*free_bytes))],
        ),
        Why::Unreadable => texts.t(key::PLAYER_TROUBLE_TAG_UNREADABLE),
        Why::Unwritable => texts.t(key::PLAYER_TROUBLE_TAG_UNWRITABLE),
        Why::NoWriter { container } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_NO_WRITER,
            &[("container", &texts.t(container_key(*container)))],
        ),
        Why::NoPlace => texts.t(key::PLAYER_TROUBLE_TAG_NO_PLACE),
        Why::Missing { program } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_MISSING,
            &[("program", program.name())],
        ),
        Why::ProgramFailed { program } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_PROGRAM_FAILED,
            &[("program", program.name())],
        ),
        Why::TimedOut { program } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_TIMED_OUT,
            &[("program", program.name())],
        ),
        Why::TooBig { bytes, most_bytes } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_TOO_BIG,
            &[("bytes", &size(*bytes)), ("most", &size(*most_bytes))],
        ),
        Why::NotAnImage => texts.t(key::PLAYER_TROUBLE_TAG_NOT_AN_IMAGE),
        Why::BadNumber => texts.t(key::PLAYER_TROUBLE_TAG_BAD_NUMBER),
        Why::BadDate => texts.t(key::PLAYER_TROUBLE_TAG_BAD_DATE),
        Why::BadIsrc => texts.t(key::PLAYER_TROUBLE_TAG_BAD_ISRC),
        Why::BadLanguage => texts.t(key::PLAYER_TROUBLE_TAG_BAD_LANGUAGE),
        Why::ReadOnlyView => texts.t(key::PLAYER_TROUBLE_TAG_READ_ONLY_VIEW),
        Why::BadName => texts.t(key::PLAYER_TROUBLE_TAG_BAD_NAME),
        Why::NotText => texts.t(key::PLAYER_TROUBLE_TAG_NOT_TEXT),
        Why::NotGlobal => texts.t(key::PLAYER_TROUBLE_TAG_NOT_GLOBAL),
        Why::NoSuchThing => texts.t(key::PLAYER_TROUBLE_TAG_NO_SUCH_THING),
        Why::Taken => texts.t(key::PLAYER_TROUBLE_TAG_TAKEN),
        Why::Cancelled => texts.t(key::PLAYER_TROUBLE_TAG_CANCELLED),
    }
}

/// A container, by the name a person knows it by — its key.
#[must_use]
pub const fn container_key(container: ronoctua_tags::Container) -> &'static str {
    use ronoctua_i18n::key;
    use ronoctua_tags::Container as C;
    match container {
        C::Mpeg => key::PLAYER_CONTAINER_MPEG,
        C::Aac => key::PLAYER_CONTAINER_AAC,
        C::Flac => key::PLAYER_CONTAINER_FLAC,
        C::OggVorbis => key::PLAYER_CONTAINER_OGG_VORBIS,
        C::Opus => key::PLAYER_CONTAINER_OPUS,
        C::Speex => key::PLAYER_CONTAINER_SPEEX,
        C::OggOther => key::PLAYER_CONTAINER_OGG_OTHER,
        C::Mp4 => key::PLAYER_CONTAINER_MP4,
        C::Wav => key::PLAYER_CONTAINER_WAV,
        C::Aiff => key::PLAYER_CONTAINER_AIFF,
        C::Ape => key::PLAYER_CONTAINER_APE,
        C::Mpc => key::PLAYER_CONTAINER_MPC,
        C::WavPack => key::PLAYER_CONTAINER_WAVPACK,
        C::Matroska => key::PLAYER_CONTAINER_MATROSKA,
        C::WebM => key::PLAYER_CONTAINER_WEBM,
        C::Avi => key::PLAYER_CONTAINER_AVI,
        C::MpegTs => key::PLAYER_CONTAINER_MPEG_TS,
        C::Flv => key::PLAYER_CONTAINER_FLV,
        C::Asf => key::PLAYER_CONTAINER_ASF,
        C::Other => key::PLAYER_CONTAINER_OTHER,
    }
}

/// A service a lookup asks, by its own name — a proper name, the same in
/// both languages, kept as a key like every word on screen.
#[must_use]
pub const fn service_key(service: ronoctua_lookup::Service) -> &'static str {
    use ronoctua_i18n::key;
    use ronoctua_lookup::Service as S;
    match service {
        S::Lrclib => key::PLAYER_SERVICE_LRCLIB,
        S::SponsorBlock => key::PLAYER_SERVICE_SPONSORBLOCK,
        S::MusicBrainz => key::PLAYER_SERVICE_MUSICBRAINZ,
        S::CoverArtArchive => key::PLAYER_SERVICE_COVERARTARCHIVE,
        S::Wikidata => key::PLAYER_SERVICE_WIKIDATA,
        S::Tmdb => key::PLAYER_SERVICE_TMDB,
        S::OpenSubtitles => key::PLAYER_SERVICE_OPENSUBTITLES,
        S::SubDl => key::PLAYER_SERVICE_SUBDL,
        S::Podnapisi => key::PLAYER_SERVICE_PODNAPISI,
    }
}

/// Why a lookup gave nothing, in a sentence that names the service.
#[must_use]
pub fn lookup_refusal(r: ronoctua_lookup::Refusal, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    use ronoctua_lookup::Why;
    let k = match r.why {
        Why::NoKey => key::PLAYER_TROUBLE_LOOKUP_NO_KEY,
        Why::KeyRefused => key::PLAYER_TROUBLE_LOOKUP_KEY_REFUSED,
        Why::WrongKey => key::PLAYER_TROUBLE_LOOKUP_WRONG_KEY,
        Why::Quota => key::PLAYER_TROUBLE_LOOKUP_QUOTA,
        Why::TooManyAsks => key::PLAYER_TROUBLE_LOOKUP_TOO_MANY_ASKS,
        Why::NothingFound => key::PLAYER_TROUBLE_LOOKUP_NOTHING_FOUND,
        Why::NothingToAsk => key::PLAYER_TROUBLE_LOOKUP_NOTHING_TO_ASK,
        Why::Unreachable => key::PLAYER_TROUBLE_LOOKUP_UNREACHABLE,
        Why::Malformed => key::PLAYER_TROUBLE_LOOKUP_MALFORMED,
        Why::TooLarge => key::PLAYER_TROUBLE_LOOKUP_TOO_LARGE,
        Why::Declined => key::PLAYER_TROUBLE_LOOKUP_DECLINED,
        Why::Unreadable => key::PLAYER_TROUBLE_LOOKUP_UNREADABLE,
        Why::TooSmall => key::PLAYER_TROUBLE_LOOKUP_TOO_SMALL,
        Why::Dormant => key::PLAYER_TROUBLE_LOOKUP_DORMANT,
    };
    texts.t_with(k, &[("service", &texts.t(service_key(r.service)))])
}

/// Why a directory gave nothing, in a sentence that names it.
#[must_use]
pub fn directory_refusal(r: ronoctua_broadcast::Refusal, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_broadcast::{Source, Why};
    use ronoctua_i18n::key;
    let source = texts.t(match r.source {
        Source::Radio => key::PLAYER_SOURCE_RADIO,
        Source::Iptv => key::PLAYER_SOURCE_IPTV,
        Source::Archive => key::PLAYER_SOURCE_ARCHIVE,
    });
    let k = match r.why {
        Why::Unreachable => key::PLAYER_TROUBLE_DIRECTORY_UNREACHABLE,
        Why::Declined => key::PLAYER_TROUBLE_DIRECTORY_DECLINED,
        Why::TooManyAsks => key::PLAYER_TROUBLE_DIRECTORY_TOO_MANY_ASKS,
        Why::NothingFound => key::PLAYER_TROUBLE_DIRECTORY_NOTHING_FOUND,
        Why::Malformed => key::PLAYER_TROUBLE_DIRECTORY_MALFORMED,
        Why::TooLarge => key::PLAYER_TROUBLE_DIRECTORY_TOO_LARGE,
        Why::NothingPlayable => key::PLAYER_TROUBLE_DIRECTORY_NOTHING_PLAYABLE,
        Why::NothingToAsk => key::PLAYER_TROUBLE_DIRECTORY_NOTHING_TO_ASK,
        Why::NotAName => key::PLAYER_TROUBLE_DIRECTORY_NOT_A_NAME,
    };
    texts.t_with(k, &[("source", &source)])
}

/// Why yt-dlp could not list a playlist, as the key of its sentence — one
/// for each of the closed set it answers with.
const fn listing_key(why: ronoctua_player::Listing) -> &'static str {
    use ronoctua_i18n::key;
    use ronoctua_player::Listing as Why;
    match why {
        Why::Absent => key::PLAYER_TROUBLE_LISTING_ABSENT,
        Why::Unstartable => key::PLAYER_TROUBLE_LISTING_UNSTARTABLE,
        Why::Slow => key::PLAYER_TROUBLE_LISTING_SLOW,
        Why::Failed => key::PLAYER_TROUBLE_LISTING_FAILED,
        Why::Garbled => key::PLAYER_TROUBLE_LISTING_GARBLED,
        Why::TooLarge => key::PLAYER_TROUBLE_LISTING_TOO_LARGE,
        Why::NotAnAddress => key::PLAYER_TROUBLE_LISTING_NOT_AN_ADDRESS,
        Why::NotAList => key::PLAYER_TROUBLE_LISTING_NOT_A_LIST,
        Why::NothingToAsk => key::PLAYER_TROUBLE_LISTING_NOTHING_TO_ASK,
    }
}

/// Bytes in a MiB, for a ceiling said in MiB.
const MIB: usize = 1024 * 1024;

/// Why a subtitle file could not be read, in words — the second half of its
/// trouble's sentence.
fn subtitle_refusal(
    why: &ronoctua_player::subtitles::Refusal,
    texts: &ronoctua_i18n::Texts,
) -> String {
    use ronoctua_i18n::{Arg, key};
    use ronoctua_player::subtitles::Refusal;
    match why {
        Refusal::TooLarge { most_bytes } => texts.t_args(
            key::PLAYER_SUBTITLE_REFUSAL_TOO_LARGE,
            &[("most", number(most_bytes.div_ceil(MIB)))],
        ),
        Refusal::Unknown => texts.t(key::PLAYER_SUBTITLE_REFUSAL_UNKNOWN),
        Refusal::Damaged => texts.t(key::PLAYER_SUBTITLE_REFUSAL_DAMAGED),
        Refusal::Empty => texts.t(key::PLAYER_SUBTITLE_REFUSAL_EMPTY),
        Refusal::Untimed => texts.t(key::PLAYER_SUBTITLE_REFUSAL_UNTIMED),
        Refusal::TooManyCues { most } => texts.t_args(
            key::PLAYER_SUBTITLE_REFUSAL_TOO_MANY_CUES,
            &[("most", number(*most))],
        ),
        Refusal::TooSlow { most_seconds } => texts.t_args(
            key::PLAYER_SUBTITLE_REFUSAL_TOO_SLOW,
            &[(
                "most",
                Arg::Number(i64::try_from(*most_seconds).unwrap_or(i64::MAX)),
            )],
        ),
        Refusal::Streamed => texts.t(key::PLAYER_SUBTITLE_REFUSAL_STREAMED),
        Refusal::NoIndex => texts.t(key::PLAYER_SUBTITLE_REFUSAL_NO_INDEX),
    }
}

/// An ambient sound's name: its own key, `player-ambience-<id>` — and a sound
/// this door has no key for is said as one it cannot name, never by its raw
/// id alone (invariant 8).
#[must_use]
pub fn ambience_name(id: &str, texts: &ronoctua_i18n::Texts) -> String {
    let own = format!("player-ambience-{id}");
    if texts.has(&own) {
        texts.t(&own)
    } else {
        texts.t_with(ronoctua_i18n::key::PLAYER_AMBIENCE_UNNAMED, &[("id", id)])
    }
}

thread_local! {
    /// THE SOUND OUTPUTS THIS DOOR HAS SEEN, by the name the sound server
    /// gives them, and what it calls them for people — fed by every state
    /// heard ([`hearing`]), so an output that went away is still said by the
    /// name a person knows it by (`bluez_output.…` is the server's).
    /// One door, one loop: the thread's own.
    static SINKS_SEEN: std::cell::RefCell<std::collections::BTreeMap<String, String>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// Every output `sinks` says, remembered by its name for people.
fn remember_sinks(sinks: &[ronoctua_player::wire::Sink]) {
    SINKS_SEEN.with(|seen| {
        let mut seen = seen.borrow_mut();
        for sink in sinks.iter().filter(|k| !k.description.trim().is_empty()) {
            seen.insert(sink.device.clone(), sink.description.clone());
        }
    });
}

/// A sound output as a person reads it: what the sound server calls it for
/// people — from `sinks`, what the player says now, else as this door last
/// saw it, else its server name, the one thing left — and, where two of
/// `sinks` would read the same, which of them it is by number.
#[must_use]
pub fn sink_label(
    device: &str,
    sinks: &[ronoctua_player::wire::Sink],
    texts: &ronoctua_i18n::Texts,
) -> String {
    let named = |d: &str| {
        sinks
            .iter()
            .find(|k| k.device == d)
            .map(|k| k.description.trim().to_owned())
            .filter(|n| !n.is_empty())
            .or_else(|| SINKS_SEEN.with(|seen| seen.borrow().get(d).cloned()))
            .unwrap_or_else(|| d.to_owned())
    };
    let name = named(device);
    let twins: Vec<&str> = sinks
        .iter()
        .map(|k| k.device.as_str())
        .filter(|d| named(d) == name)
        .collect();
    match twins.iter().position(|d| *d == device) {
        Some(at) if twins.len() > 1 => texts.t_args(
            ronoctua_i18n::key::PLAYER_SINK_TWIN,
            &[
                ("name", ronoctua_i18n::Arg::Text(&name)),
                ("n", number(at + 1)),
            ],
        ),
        _ => name,
    }
}

/// A group of ambient sounds, as its heading says it: its own key,
/// `player-ambience-group-<group>` — and a group this door has no key for is
/// said as one it cannot name, never by its raw id alone (invariant 8).
#[must_use]
pub fn ambience_group(group: &str, texts: &ronoctua_i18n::Texts) -> String {
    let own = format!("player-ambience-group-{group}");
    if texts.has(&own) {
        texts.t(&own)
    } else {
        texts.t_with(
            ronoctua_i18n::key::PLAYER_AMBIENCE_GROUP_UNNAMED,
            &[("group", group)],
        )
    }
}

/// A track's name among `kin`, the others of its kind: its title, its
/// language, its codec — whichever it has — and, where two of them would read
/// the same, its codec after it, and then its number.
///
/// ONE AT A TIME the old ring could say `eng` for three English sounds; side
/// by side on a page, three rows reading `eng` were three choices nobody can
/// tell apart — and the census of labels cannot see it, because these labels
/// are the file's.
pub(crate) fn track_name(t: &Track, kin: &[&Track]) -> String {
    let twins = |name: &dyn Fn(&Track) -> String| {
        let mine = name(t);
        kin.iter().filter(|o| name(o) == mine).count() > 1
    };
    if !twins(&own_name) {
        return own_name(t);
    }
    let with_codec = |o: &Track| {
        let name = own_name(o);
        match o
            .codec
            .as_deref()
            .filter(|c| !c.is_empty() && !name.contains(c))
        {
            Some(codec) => format!("{name} · {codec}"),
            None => name,
        }
    };
    if twins(&with_codec) {
        format!("{} · {}", with_codec(t), t.id)
    } else {
        with_codec(t)
    }
}

/// A track's own name, alone: its title, its language, its codec — whichever
/// it has.
fn own_name(t: &Track) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(title) = t.title.as_deref().filter(|s| !s.is_empty()) {
        parts.push(title.to_owned());
    }
    if let Some(lang) = t.lang.as_deref().filter(|s| !s.is_empty()) {
        parts.push(lang.to_owned());
    }
    if parts.is_empty()
        && let Some(codec) = t.codec.as_deref()
    {
        parts.push(codec.to_owned());
    }
    if parts.is_empty() {
        parts.push(t.id.to_string());
    }
    parts.join(" · ")
}

/// What the arrows on the home's row of `kind` ask: the next (or previous)
/// track of the lane of that kind — with the lane switched off in the ring
/// when `with_none`, since subtitles can be off and a sound cannot. `None`
/// with nothing to turn to.
fn next_track(state: &State, kind: TrackKind, forward: bool, with_none: bool) -> Option<Ask> {
    let lane = crate::player_tracks::lane_of(kind, state)?;
    let tracks = crate::player_tracks::of_kind(kind, state);
    let mut ring: Vec<Option<i64>> = Vec::new();
    if with_none {
        ring.push(None);
    }
    ring.extend(tracks.iter().map(|t| Some(t.id)));
    if ring.len() < 2 {
        return None;
    }
    let now = tracks
        .iter()
        .find(|t| crate::player_tracks::plays(t, lane))
        .map(|t| t.id);
    let at = ring.iter().position(|x| *x == now).unwrap_or(0);
    let n = ring.len();
    let to = ring[if forward {
        (at + 1) % n
    } else {
        (at + n - 1) % n
    }];
    Some(crate::player_tracks::choose(lane.id, to))
}

/// Where `C-↑`/`C-↓` move entry `id` among its siblings: after the next one, or
/// — back — after the one before the one before it, else first in its
/// playlist (or the queue). `None` at the end it would go past, or for an
/// entry the outline does not hold.
#[must_use]
pub fn moved(outline: &Outline, id: EntryId, forward: bool) -> Option<Ask> {
    fn siblings(
        nodes: &[Node],
        parent: Option<EntryId>,
        id: EntryId,
    ) -> Option<(Option<EntryId>, &[Node], usize)> {
        if let Some(i) = nodes.iter().position(|n| n.id == id) {
            return Some((parent, nodes, i));
        }
        nodes
            .iter()
            .find_map(|n| siblings(&n.children, Some(n.id), id))
    }
    let (parent, nodes, i) = siblings(&outline.entries, None, id)?;
    let (after, into) = if forward {
        (Some(nodes.get(i + 1)?.id), None)
    } else {
        match i {
            0 => return None,
            1 => (None, parent),
            _ => (Some(nodes[i - 2].id), None),
        }
    };
    Some(Ask::Move { id, after, into })
}

impl View {
    /// A view on the home, with the cursor on the first row that does
    /// something.
    #[must_use]
    pub fn fresh() -> Self {
        Self {
            kept: crate::player_internet::Kept::read(),
            directories: crate::player_directory::Directories::of_this_machine(),
            reads_prefs: true,
            ..Self::with_prefs(ronoctua_player::prefs::store().read())
        }
    }

    /// A fresh view on the preferences' DEFAULTS, whatever the person chose
    /// — a photograph's (`crate::photograph`), which is the same picture on
    /// every machine. Its file browser's places are in `home` as a machine
    /// that declares none has them (`player_browse::places_in`), and this
    /// machine's own when `home` is `None`.
    #[must_use]
    pub fn blank(home: Option<std::path::PathBuf>) -> Self {
        Self {
            home,
            ..Self::with_prefs(ronoctua_player::prefs::Preferences::default())
        }
    }

    /// Whether a page of the files is on screen.
    #[must_use]
    pub fn browsing(&self) -> bool {
        self.top().files.is_some()
    }

    /// The places the file browser starts from.
    fn places(&self) -> [(&'static str, std::path::PathBuf); 3] {
        self.home.as_deref().map_or_else(
            crate::player_browse::places,
            crate::player_browse::places_in,
        )
    }

    /// A fresh view on `prefs`.
    fn with_prefs(prefs: ronoctua_player::prefs::Preferences) -> Self {
        Self {
            levels: vec![Level::home()],
            prefs,
            left_at: None,
            came_from: None,
            asking: None,
            clicked_control: None,
            home: None,
            swap: false,
            panel: false,
            no_room: false,
            outline: None,
            folds: std::collections::BTreeMap::new(),
            last_current: None,
            typing: None,
            beside: crate::player_subtitles::Beside::default(),
            wake: None,
            pictures_room: None,
            deck_in_now: false,
            screens: Vec::new(),
            here: None,
            meta: crate::player_metadata::Meta::default(),
            found: crate::player_find::Found::default(),
            finding: crate::player_find::Choices::default(),
            kept: crate::player_internet::Kept::default(),
            directories: crate::player_directory::Directories::default(),
            globe: crate::player_globe::Seen::default(),
            prefs_on_disk: crate::player_internet::Watched::default(),
            reads_prefs: false,
        }
    }

    /// This door wrote `player.json` — a switch, a consent: what the view
    /// shows is read back from the file once the door carried it out
    /// ([`View::errand_carried`]), so a write that was refused is not shown
    /// as done.
    pub(crate) fn prefs_written(&self) {
        self.prefs_on_disk.stir();
    }

    /// `player.json` read again when it moved — this door's write, another
    /// door's — watched from the first look of a door with a loop.
    fn prefs_look(&mut self) {
        if !self.reads_prefs {
            return;
        }
        let store = ronoctua_player::prefs::store();
        let read = match self.wake.as_ref() {
            Some(wake) => {
                let dir = store
                    .path()
                    .and_then(|p| p.parent().map(std::path::Path::to_path_buf));
                self.prefs_on_disk
                    .look(&[(dir, ronoctua_player::prefs::FILE)], wake)
            }
            None => self.prefs_on_disk.take_stir(),
        };
        if read {
            self.prefs = store.read();
        }
    }

    /// THE PLAYER IS GONE — crashed, restarted, closed: what this door held
    /// of its sheets, desks and lists is forgotten. A player that comes back
    /// counts its generations and its candidates from zero again, and a page
    /// held at the same number would show the old one's — a suggestion taken
    /// by an id the new one gave to another would write values nobody saw.
    pub fn player_gone(&mut self) {
        self.meta.forget();
        self.found.forget();
        self.directories.forget_the_asks();
    }

    /// What a door calls once it carried out an errand of the view's: the
    /// preferences and the keys read back from their files, where the errand
    /// wrote them — or was refused writing them.
    pub fn errand_carried(&mut self) {
        self.prefs_look();
    }

    /// How an ask told off the loop ended, for the view: a directory's list
    /// refused at once is its page's to say; a key that could not be kept is
    /// read again from its files. `true` when that changed the rows.
    pub fn told(&mut self, answer: &Told) -> bool {
        match answer {
            Told::Listing { query, answer } => {
                self.listing_told(query, answer);
                true
            }
            Told::KeyTrouble(_) => {
                self.kept.stir();
                true
            }
            _ => false,
        }
    }

    /// The box the door lays its pictures out in now, in logical pixels —
    /// `None` when it shows none. `true` when that changed.
    pub fn pictures_room_is(&mut self, room: Option<(f32, f32)>) -> bool {
        let changed = self.pictures_room != room;
        self.pictures_room = room;
        changed
    }

    /// The view, with what wakes its door's loop when a read it started off
    /// the loop is done ([`crate::player_subtitles::Wake`]).
    #[must_use]
    pub fn waking(mut self, wake: crate::player_subtitles::Wake) -> Self {
        self.wake = Some(wake);
        self
    }

    /// The screens the door knows by name, and the one it is on — told by
    /// the door each time they change. `true` when that changed the rows.
    pub fn screens_are(
        &mut self,
        screens: Vec<crate::player_lane::Known>,
        here: Option<String>,
    ) -> bool {
        let changed = self.screens != screens || self.here != here;
        self.screens = screens;
        self.here = here;
        changed
    }

    /// The queue's outline, as the door's watch heard it — `None` when the
    /// player is gone, whose queue this door no longer knows.
    pub fn outline_arrived(&mut self, outline: Option<Box<Outline>>) {
        self.outline = outline;
    }

    /// Whether the footer's region leaves the picture no room beside the
    /// rows — the footer tells its view, as it tells it everything else it
    /// alone knows. `true` when that changed, and the rows with it.
    pub fn picture_refused(&mut self, refused: bool) -> bool {
        let changed = self.no_room != refused;
        self.no_room = refused;
        changed
    }

    /// Whether the footer's media panel stands beside the rows — its
    /// transport carries the swap then, and the "Now" title leaves it out:
    /// two glyphs for one act were the same button drawn twice.
    pub fn beside_the_panel(&mut self, panel: bool) {
        self.panel = panel;
    }

    /// The view as the FOOTER shows it: the swap's glyph beside the "Now"
    /// title — the window and the column carry it in their transport, and
    /// the footer has one only in its media panel, which stands while
    /// something is loaded — and a set's deck as the first rows of "Now",
    /// the footer having no block under a transport to draw it in.
    #[must_use]
    pub fn in_the_footer(self) -> Self {
        Self {
            swap: true,
            deck_in_now: true,
            ..self
        }
    }

    /// The files where they were left, or the music folder the first time —
    /// what a page of the files opens on (`View::push`), listing what
    /// `purpose` is for.
    pub(crate) fn files_where_left(&self, purpose: Purpose) -> Files {
        let (place, at) = self.left_at.clone().unwrap_or_else(|| {
            let places = self.places();
            (0, places[0].1.clone())
        });
        Files {
            place,
            listing: crate::player_browse::list_for(&at, purpose.wanted()),
        }
    }

    /// Another folder, the cursor on `on` when it is one of its rows.
    fn go_to(&mut self, at: &std::path::Path, on: Option<&std::path::Path>) {
        let wanted = match self.page() {
            PageId::Files(purpose) => purpose.wanted(),
            _ => crate::player_browse::Wanted::Media,
        };
        let top = self.top_mut();
        if let Some(f) = top.files.as_mut() {
            f.listing = crate::player_browse::list_for(at, wanted);
            let place = f.place;
            top.cursor = 0;
            top.cell = 0;
            top.moved = false;
            self.left_at = Some((place, at.to_path_buf()));
        }
        self.came_from = on.map(std::path::Path::to_path_buf);
    }

    /// The spot of the row the cursor is on — what a page opened from it
    /// remembers, to put the cursor back there.
    pub(crate) fn spot_here(
        &self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<String> {
        self.rows(state, columns, texts)
            .get(self.top().cursor)
            .map(|r| r.spot.clone())
            .filter(|s| !s.is_empty())
    }

    /// Opens `page` over the one on screen, from the row under the cursor,
    /// and lands its cursor on a row that acts.
    ///
    /// BY NAME — a command, `LEADER p m`, or the files' `f` — no row opened
    /// it, and the one under the cursor was only where the cursor happened to
    /// be: `Esc` came back to the volume. The page's own row on this level
    /// is what opened it then, when there is one — `Esc` lands on "Other
    /// machines", which is where the page lives.
    fn open(
        &mut self,
        page: PageId,
        by_name: bool,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let own = by_name
            && self
                .rows(state, columns, texts)
                .iter()
                .any(|r| r.spot == page.spot());
        let from = if own {
            Some(page.spot().to_owned())
        } else {
            self.spot_here(state, columns, texts)
        };
        self.push(page, from);
        // A SUBTITLE IS LOOKED FOR BESIDE ITS FILM: the files open on the
        // film's own folder, when it has one here.
        if page == PageId::Files(Purpose::Subtitle)
            && let Some(folder) = state
                .and_then(crate::player_subtitles::film)
                .filter(|l| !l.source.contains("://"))
                .and_then(|l| std::path::Path::new(&l.source).parent())
                .filter(|f| f.is_dir())
        {
            let folder = folder.to_path_buf();
            self.go_to(&folder, None);
        }
        // A COVER IS LOOKED FOR BESIDE ITS FILE, where a scan of it lives.
        if page == PageId::Files(Purpose::Cover)
            && let Some(folder) = state
                .and_then(|s| self.meta.location(s))
                .filter(|l| !l.contains("://"))
                .and_then(|l| {
                    std::path::Path::new(&l)
                        .parent()
                        .map(std::path::Path::to_path_buf)
                })
                .filter(|f| f.is_dir())
        {
            self.go_to(&folder, None);
        }
        self.settle_in(state, columns, texts);
    }

    /// Which row the cursor is on.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.top().cursor
    }

    /// The hint line.
    #[must_use]
    pub fn hint(
        &self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        let rows = self.rows(state, columns, texts);
        let target = rows
            .get(self.cursor())
            .map_or(Target::Nothing, |r| r.target.clone());
        // A KEY THAT IS ONLY HERE BECAUSE OF THE ROW UNDER THE CURSOR reads as
        // one (`Register::Contextual`): the owner asked for the passing guests to
        // look apart from the keys that are always on the line.
        // A PAGE OF THE FILES FOR ANOTHER PURPOSE — a lane, a file to open —
        // takes its pick with `RET`: "add to the queue" and "play now" taught
        // there would be keys saying what they do not do.
        let queueing = !matches!(self.page(), PageId::Files(p) if p != Purpose::Queue);
        let here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(&target, state, self.page()))
            .filter(|(_, id, _)| queueing || !matches!(*id, "player-add" | "player-play-now"))
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        crate::chrome::hint_of_marked(&here, |_| None, texts)
    }

    /// A click on row `n`, at `column` (a field's `at`); `true` when the
    /// door should run what `RET` does — the second click on the same place,
    /// or a click on a control beside a title, which acts at once.
    pub fn clicked_at(&mut self, n: usize, column: u16, rows: &[Row]) -> bool {
        // THE REST OF A WRAPPED LABEL is its row: a click there is a click
        // on the row it continues.
        let n = Row::owner(rows, n);
        let Some(row) = rows.get(n) else {
            return false;
        };
        let cell = row
            .cells
            .iter()
            .position(|c| column >= c.from && column < c.from + c.width);
        // A CONTROL ON A TITLE'S LINE acts on the click: a title is not a
        // place the cursor stops, so there is no first click to spend on it.
        if row.heading {
            if let Some(c) = cell {
                self.clicked_control = Some(row.cells[c].target.clone());
                return true;
            }
            return false;
        }
        // A CONTROL ON A ROW — a group's triangle, the glyph that plays a
        // medium too — acts on the click as well, and the cursor comes to its
        // row: it is a button, and the row it sits on is what it is about.
        if let Some(c) = cell.filter(|&c| row.cells[c].control) {
            self.clicked_control = Some(row.cells[c].target.clone());
            let top = self.top_mut();
            top.moved = true;
            top.cursor = n;
            return true;
        }
        // THE REST OF A ROW WITH A CONTROL is the row: the second click on it
        // is what `RET` does, wherever on it it lands.
        let grid = row.cells.iter().any(|c| !c.control);
        let top = self.top_mut();
        let again = top.cursor == n && (!grid || cell == Some(top.cell));
        top.moved = true;
        top.cursor = n;
        if let Some(c) = cell {
            top.cell = c;
        }
        again
    }

    /// A lane's BAND on row `n` let go where it was pressed: the first click
    /// CHOOSES the row — the cursor comes to it — and a second, on the row the
    /// cursor is already on, opens the lane's page, as a second click on any
    /// row does what `RET` does (invariant 19). `true`: open it.
    pub fn band_clicked(&mut self, n: usize) -> bool {
        let again = self.top().cursor == n;
        self.grabbed(n);
        again
    }

    /// A press on the bar of row `n`: the row becomes the cursor's, and the
    /// door sets the value where the press landed (`player_rows::slid`) — a
    /// bar acts on the press, as the window's time bar does, and holding
    /// drags it.
    pub fn grabbed(&mut self, n: usize) {
        let top = self.top_mut();
        top.cursor = n;
        top.cell = 0;
        top.moved = true;
    }

    /// The files, open on `at` for `purpose` — what a test of a door stands
    /// on, with no folder of the machine's read.
    #[cfg(test)]
    pub fn files_at(&mut self, at: &std::path::Path, purpose: Purpose) {
        self.left_at = Some((0, at.to_path_buf()));
        self.push(PageId::Files(purpose), None);
    }

    /// A click on row `n` — on the entry of a grid the cursor is on, else at
    /// the row's first column: never on a control at its head or its end,
    /// which acts at once.
    #[cfg(test)]
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        let cell = self.top().cell;
        let column = rows
            .get(n)
            .and_then(|r| {
                let grid: Vec<&crate::player_rows::Cell> =
                    r.cells.iter().filter(|c| !c.control).collect();
                grid.get(cell).or_else(|| grid.first()).map(|c| c.from)
            })
            .unwrap_or(0);
        self.clicked_at(n, column, rows)
    }

    /// The cursor back on a real row after the rows changed.
    pub fn settle_in(&mut self, state: Option<&State>, columns: u16, texts: &ronoctua_i18n::Texts) {
        self.remember_the_folds(state, columns, texts);
        if self.page() == PageId::Tracks(TrackKind::Subtitles) {
            self.beside.look(state, self.wake.as_ref());
        }
        // THE SHEET OF THE FILE SHOWN, while a page shows it: asked again
        // when the file or its generation moved, off the loop.
        if matches!(
            self.page(),
            PageId::Home
                | PageId::Cover
                | PageId::Streams
                | PageId::Chapters
                | PageId::EveryTag
                | PageId::Files(Purpose::Cover)
                | PageId::Suggestions
                | PageId::Candidate(_)
                | PageId::FindLyrics
                | PageId::Field(_)
        ) {
            self.meta.look(state, self.wake.as_ref());
        }
        // THE LOOKUPS' PAGES, while a page shows one: asked again when its
        // desk's generation moved, off the loop — and the cover of the
        // suggestion on screen fetched and decoded.
        let (desks, candidate) = match self.page() {
            PageId::Home | PageId::Suggestions => (vec![DeskKind::Suggestions], None),
            PageId::Candidate(id) => (vec![DeskKind::Suggestions], Some(id)),
            PageId::FindSubtitles => (vec![DeskKind::Subtitles], None),
            PageId::FindLyrics => (vec![DeskKind::Lyrics], None),
            // A DIRECTORY'S PAGE reads the desks its lists are on.
            _ => (self.directory_desks(), None),
        };
        self.found
            .look(&desks, candidate, state, self.wake.as_ref());
        // THE LISTS KEPT are the ones this door's stack shows: a page left
        // is a list nobody reads.
        let listed = self.listed_on_the_stack();
        self.found.keep_listings(&listed);
        self.heard_the_lists(state);
        if matches!(
            self.page(),
            PageId::Favourites
                | PageId::Recent
                | PageId::Radio
                | PageId::Tv
                | PageId::Stations
                | PageId::TvChannels
                | PageId::Theme(_)
        ) {
            self.directories.picks.look(self.wake.as_ref());
        }
        self.prefs_look();
        if matches!(
            self.page(),
            PageId::Internet | PageId::Suggestions | PageId::FindSubtitles
        ) {
            self.kept.look(self.wake.as_ref());
        }
        // A DRAFT'S QUESTION ANOTHER DOOR ANSWERED — or whose draft was
        // written, kept for later or thrown away — goes: answering it here
        // would ask about a draft that is not there, or is not saved.
        if let Some(id) = self.asking.as_ref().and_then(|c| c.draft)
            && !state.is_some_and(|s| s.drafts.iter().any(|d| d.id == id && d.saved))
            && let Some(q) = self.asking.take()
        {
            let top = self.top_mut();
            (top.cursor, top.cell) = q.back;
            top.moved = true;
        }
        // A DRAFT SAVED IN THE EDITOR is asked about in whichever door is
        // open — "no" first; the door that opened the editor may be gone.
        if self.asking.is_none()
            && let Some(s) = state
            && let Some(d) = self.meta.draft_to_ask(s)
        {
            self.ask_about_the_draft(crate::player_metadata::draft_question(&d, texts));
        }
        let rows = self.rows(state, columns, texts);
        self.clamp(&rows);
        // AMONG THE FILES the cursor lands on the folder just left, or on the
        // first entry: the rows above them are the same in every folder, and
        // the entries are what a person came to walk.
        if !self.top().moved && self.browsing() {
            let came = self.came_from.take();
            // THE FOLDER JUST LEFT, or else the first real entry — `..` is
            // the way back, not what a person opened the folder for, and it
            // is landed on only when there is nothing else.
            let wanted: [&dyn Fn(&Target) -> bool; 3] = [
                &|t| matches!((t, came.as_deref()), (Target::Folder(p), Some(c)) if p == c),
                &|t| came.is_none() && matches!(t, Target::Folder(_) | Target::File(_)),
                &|t| matches!(t, Target::Up),
            ];
            // THE ENTRIES ARE CELLS of a grid now: they are looked for in
            // each, and the cursor lands on the row AND the column.
            for is_it in wanted {
                for (i, r) in rows.iter().enumerate().filter(|(_, r)| !r.heading) {
                    if let Some(c) = r.cells.iter().position(|c| is_it(&c.target)) {
                        let top = self.top_mut();
                        top.cursor = i;
                        top.cell = c;
                        return;
                    }
                }
            }
        }
        let top = self.top();
        if !top.moved
            && rows.get(top.cursor).is_some_and(|r| !r.target.acts())
            && let Some(i) = rows.iter().position(|r| r.stops() && r.target.acts())
        {
            self.top_mut().cursor = i;
        }
    }

    /// The cursor stays on its row by identity.
    pub fn follow(
        &mut self,
        spot: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let rows = self.rows(state, columns, texts);
        if let Some(i) = rows.iter().position(|r| r.spot == spot) {
            self.top_mut().cursor = i;
        }
        self.clamp(&rows);
    }

    fn clamp(&mut self, rows: &[Row]) {
        let top = self.top_mut();
        if rows.is_empty() {
            top.cursor = 0;
            return;
        }
        top.cursor = top.cursor.min(rows.len() - 1);
        // ON THE REST OF A WRAPPED LABEL: back on the row it continues.
        top.cursor = Row::owner(rows, top.cursor);
        if rows[top.cursor].heading {
            let after = (top.cursor..rows.len()).find(|&i| rows[i].stops());
            let before = (0..=top.cursor).rev().find(|&i| rows[i].stops());
            if let Some(i) = after.or(before) {
                top.cursor = i;
            }
        }
    }

    fn walk(&mut self, rows: &[Row], down: bool) {
        let top = self.top_mut();
        let mut at = top.cursor;
        loop {
            if down {
                if at + 1 >= rows.len() {
                    return;
                }
                at += 1;
            } else {
                if at == 0 {
                    return;
                }
                at -= 1;
            }
            if rows[at].stops() {
                top.cursor = at;
                top.moved = true;
                top.cell = top.cell.min(rows[at].cells.len().saturating_sub(1));
                return;
            }
        }
    }

    /// Across a grid: `←`/`→` move between its columns, and past the foot
    /// of one column `↓` goes on at the head of the next — the order the
    /// names are read in, which is down each column (`Builder::grid`).
    fn across(&mut self, rows: &[Row], press: Press) -> bool {
        let top = self.top_mut();
        let Some(row) = rows.get(top.cursor) else {
            return false;
        };
        if row.cells.len() < 2 && !matches!(press, Press::Up | Press::Down) {
            return false;
        }
        // A GRID IS ITS ENTRIES: a row's controls — a triangle, a glyph at
        // its end — make no row one.
        let grid: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.cells.iter().any(|c| !c.control) && !r.heading)
            .map(|(i, _)| i)
            .collect();
        let (Some(&first), Some(&last)) = (grid.first(), grid.last()) else {
            return false;
        };
        if !grid.contains(&top.cursor) {
            return false;
        }
        match press {
            Press::Less if top.cell > 0 => top.cell -= 1,
            Press::More if top.cell + 1 < row.cells.len() => top.cell += 1,
            Press::Down if top.cursor == last && top.cell + 1 < rows[first].cells.len() => {
                top.cursor = first;
                top.cell += 1;
            }
            Press::Up if top.cursor == first && top.cell > 0 => {
                top.cell -= 1;
                // THE FOOT OF THE COLUMN BEFORE, which may be shorter: the
                // last row that has an entry in it.
                let cell = top.cell;
                top.cursor = grid
                    .iter()
                    .rev()
                    .copied()
                    .find(|&i| rows[i].cells.len() > cell)
                    .unwrap_or(first);
            }
            _ => return false,
        }
        top.moved = true;
        true
    }

    /// A key.
    pub fn key(
        &mut self,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let asked_before = self.asking.is_some();
        let errand = self.keyed(press, state, columns, texts);
        // WHAT THIS VIEW SHOWS of `player.json` it changed at once; the file,
        // once the door wrote it, is read back — a write that was refused is
        // not shown as done ([`View::errand_carried`]).
        if errand.writes_prefs() {
            self.prefs_written();
        }
        // A QUESTION THAT JUST OPENED has its cursor on "no", the first
        // answer drawn — at once, and not on the next key or state: a
        // question about something that cannot be undone is born on its no,
        // and a stopped player sends no state to settle it.
        if !asked_before && self.asking.is_some() {
            self.settle_in(state, columns, texts);
        }
        // AND THE LISTS A PAGE THAT CAME UP OWES THE PLAYER, after it.
        self.owing(errand)
    }

    /// What a key does — [`View::key`]'s, before a question it opens is
    /// settled.
    fn keyed(
        &mut self,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        // A QUESTION ON SCREEN takes every key until it is answered — `Esc`
        // is its "no" before it is a way back.
        if self.asking.is_some() {
            return self.answering(press, state, columns, texts);
        }
        self.settle_in(state, columns, texts);
        let rows = self.rows(state, columns, texts);
        // A CONTROL BESIDE A TITLE, clicked: what it does, now.
        if press == Press::Enter
            && let Some(target) = self.clicked_control.take()
        {
            return self.act(&target, press, state, columns, texts);
        }
        match press {
            // `Esc` GIVES EVERY ROW BACK first, where a group of the globe's
            // marks narrowed them — and then goes back one page.
            Press::Escape if self.top().only.is_some() => {
                self.top_mut().only = None;
                return Errand::Nothing;
            }
            // `Esc` GOES BACK ONE PAGE, the cursor on the row that opened
            // it, and closes the door only from the home.
            Press::Escape => {
                return if self.back(state, columns, texts) {
                    Errand::Nothing
                } else {
                    Errand::Close
                };
            }
            Press::Browse => {
                if !self.browsing() {
                    self.open(PageId::Files(Purpose::Queue), true, state, columns, texts);
                }
                return Errand::Nothing;
            }
            Press::Go(page) => {
                if self.page() != page {
                    self.open(page, true, state, columns, texts);
                }
                return Errand::Nothing;
            }
            Press::Tracks => return self.walk_the_media(state, columns, texts),
            Press::Clear => return self.ask_to_clear(state, texts),
            Press::SaveSet => return self.save(Saved::Set, state, texts),
            Press::SaveQueue => return self.save(Saved::Queue, state, texts),
            Press::Quieter | Press::Louder => {
                return volume_step(state, press == Press::Louder, texts);
            }
            Press::Swap => return button_errand("player-swap", state, texts),
            Press::EveryScreen => return Errand::EveryScreen,
            Press::Metadata => return self.show_the_metadata(state, columns, texts),
            Press::Search => return self.search_here(),
            Press::Globe(turn) => {
                self.globe_turned(turn);
                return Errand::Nothing;
            }
            Press::Parent => {
                if let Some(at) = self.top().files.as_ref().map(|f| f.listing.at.clone())
                    && let Some(up) = at.parent()
                {
                    self.go_to(up, Some(&at));
                    self.settle_in(state, columns, texts);
                }
                return Errand::Nothing;
            }
            Press::Up | Press::Down => {
                if !self.across(&rows, press) {
                    self.walk(&rows, press == Press::Down);
                }
                return Errand::Nothing;
            }
            // `←`/`→` ON A GRID move between its columns; elsewhere they turn
            // the value of the row, below.
            Press::Less | Press::More if self.across(&rows, press) => return Errand::Nothing,
            Press::Toggle => return Errand::Tell(toggle_ask(state)),
            Press::Stop => return Errand::Tell(Ask::Stop),
            Press::Only => return Errand::Show(Show::Only),
            Press::Maximize => return Errand::Show(Show::Maximized),
            Press::Pip => return Errand::Show(Show::Small),
            Press::Mute => return Errand::Tell(Ask::Mute),
            Press::Next => return Errand::Tell(Ask::Next),
            Press::Previous => return Errand::Tell(Ask::Previous),
            Press::Less
            | Press::More
            | Press::Enter
            | Press::Remove
            | Press::Add
            | Press::PlayNow
            | Press::Fold
            | Press::Earlier
            | Press::Later
            | Press::Rename
            | Press::Shown
            | Press::Favourite => {}
        }
        let target = rows
            .get(self.cursor())
            .map_or(Target::Nothing, |r| r.target.clone());
        self.act(&target, press, state, columns, texts)
    }

    /// What a press does on a target — a row's, a grid cell's, or a
    /// control's beside a title.
    fn act(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let target = target.clone();
        // A NEW FOLDER PUTS THE CURSOR BACK ON ZERO, which is its heading;
        // settling it here is what lands it on an entry. The doors never
        // settle after a key, and a cursor on a heading teaches no key of
        // the level — the photograph showed exactly that.
        if target.among_the_files() {
            let errand = self.among_the_files(&target, press, state, columns, texts);
            self.settle_in(state, columns, texts);
            return errand;
        }
        if press == Press::Enter {
            match target {
                // A PAGE, over this one.
                Target::Go(page) => {
                    self.open(page, false, state, columns, texts);
                    return Errand::Nothing;
                }
                Target::ClearQueue => return self.ask_to_clear(state, texts),
                Target::Swap => return button_errand("player-swap", state, texts),
                Target::Quit => return self.ask_to_quit(texts),
                Target::Restart => return self.ask_to_restart(texts),
                Target::Rename => return self.ask_a_name(&Target::Nothing, state, texts),
                Target::SaveSet => return self.save(Saved::Set, state, texts),
                Target::SaveQueue => return self.save(Saved::Queue, state, texts),
                // A LANE'S OWN PAGE, from its row — a subtitle's, a lane
                // playing together, one under a set in the queue.
                Target::Subtitle(id) | Target::Lane(id) => {
                    self.open(PageId::Lane(id), false, state, columns, texts);
                    return Errand::Nothing;
                }
                // WHEN IT BEGINS, TYPED: the line opens holding when it
                // begins now — and where it stops, and its name, the same.
                Target::LaneStart(id) => return self.ask_a_start(id, state, texts),
                Target::LaneUntil(id) => return self.ask_an_end(id, state, texts),
                Target::LaneTitle(id) => return self.ask_a_title(id, state, texts),
                // TAKEN OUT, from its own page: the page goes with it, back
                // to the row that opened it.
                Target::LaneRemove(id) => {
                    self.back(state, columns, texts);
                    return Errand::Tell(Ask::Lane {
                        lane: id,
                        change: LaneChange::Remove,
                    });
                }
                // A SUGGESTION'S OWN PAGE, from its row — whose label is the
                // suggestion's name.
                Target::Candidate(id) => {
                    self.open(PageId::Candidate(id), false, state, columns, texts);
                    return Errand::Nothing;
                }
                Target::EveryScreen => return Errand::EveryScreen,
                // THE SOUND OUTPUT a lane goes to: a row each, `RET` sends it
                // there.
                Target::LaneSink(id, ref device) => {
                    return Errand::Tell(Ask::Lane {
                        lane: id,
                        change: LaneChange::Sink {
                            device: device.clone(),
                        },
                    });
                }
                // AN AMBIENT SOUND, on or off — on, it plays at once; one past
                // the machine's `at_once` is refused before it is sent.
                Target::Sound(ref id) => {
                    return state.map_or(Errand::Nothing, |s| {
                        crate::player_ambience::turned(id, s, texts)
                    });
                }
                Target::Silence => {
                    return Errand::TellSaying(
                        Ask::AmbientSilence,
                        texts.t(ronoctua_i18n::key::PLAYER_AMBIENCE_SILENCED),
                    );
                }
                _ => {}
            }
        }
        // THE DIRECTORIES' rows: a list asked or opened, the search, a row
        // that plays and its star (`crate::player_directory`).
        if let Some(errand) = self.directory_turned(&target, press, state, columns, texts) {
            return errand;
        }
        // A SUBTITLE FILE BESIDE THE FILM, laid over its picture — `RET`, and
        // `a` as on every file.
        if let (Target::Beside(path), Press::Enter | Press::Add) = (&target, press) {
            return state.map_or(Errand::Nothing, |s| {
                crate::player_subtitles::lay_over(&path.to_string_lossy(), s, texts)
            });
        }
        // WHICH GROUP OF THE QUEUE IS OPEN IS THIS VIEW'S: its triangle and
        // `TAB` turn it, `→` opens it and `←` folds it
        // (`crate::player_queue`).
        match (&target, press) {
            (Target::Fold(id), Press::Enter) | (Target::Group(id), Press::Fold) => {
                self.fold(*id, None, state, columns, texts);
                return Errand::Nothing;
            }
            (Target::Group(id), Press::Less | Press::More) => {
                self.fold(*id, Some(press == Press::More), state, columns, texts);
                return Errand::Nothing;
            }
            // A NAME, for the entry under the cursor — or, anywhere else, for
            // what plays.
            (_, Press::Rename) => return self.ask_a_name(&target, state, texts),
            // ONE OF THE FILE'S MEDIA, TOO: a lane of its own beside the one
            // in use — its glyph, and `a` on its row.
            (Target::Also { kind, id }, Press::Enter)
            | (Target::Track { kind, id: Some(id) }, Press::Add) => {
                return state.map_or(Errand::Nothing, |s| {
                    crate::player_tracks::also(*kind, *id, s, texts)
                });
            }
            _ => {}
        }
        // THE SCREENS A PICTURE GOES TO are the ones THIS DOOR knows by name
        // (`View::screens`): only the view has them.
        if let Some(s) = state {
            let known = &self.screens;
            let here = self.here.as_deref();
            match (&target, press) {
                (Target::LaneEveryScreen(id), Press::Enter | Press::Less | Press::More) => {
                    return crate::player_lane::every_screen_turned(s, *id, known, here, texts);
                }
                (Target::LaneScreen(id, c), Press::Enter | Press::Less | Press::More) => {
                    return crate::player_lane::screen_turned(s, *id, c, known, texts);
                }
                _ => {}
            }
            // THE LAYOUT PAGE'S rows: the ring, a picture, its two cells.
            if let Some(errand) = crate::player_layout::turn(&target, press, s) {
                return errand;
            }
        }
        // THE INTERNET PAGE'S switches and keys — the machine's, there with
        // no player running.
        if let Some(errand) = self.internet_turned(&target, press, texts) {
            return errand;
        }
        // THE LOOKUPS' rows: who is asked, the searches, what they found.
        if let Some(s) = state {
            if let Some(errand) = crate::player_suggest::turn(self, &target, press, s, texts) {
                return errand;
            }
            if let Some(turned) = crate::player_find::turn(self, &target, press, s, texts) {
                return self.turned(turned);
            }
        }
        // THE METADATA'S rows: its ring, its fields, its cover.
        if let Some(s) = state
            && let Some(turned) =
                crate::player_metadata::turn(&mut self.meta, &target, press, s, texts)
        {
            if let crate::player_metadata::Turned::Page(page) = turned {
                self.open(page, false, state, columns, texts);
                return Errand::Nothing;
            }
            return self.turned(turned);
        }
        // THE TWO SWITCHES ARE THIS VIEW'S: a ring of two, any key turns it.
        // Only the switch turned is written, merged with the file: this
        // view's copy is what it SHOWS, and read again when the file moves.
        let switch = match target {
            Target::LyricsOnline if press != Press::Remove => {
                Some(Switch::LyricsOnline(!self.prefs.lyrics_online))
            }
            Target::Sponsors if press != Press::Remove => {
                Some(Switch::Sponsors(!self.prefs.sponsorblock))
            }
            Target::Skip(category) if press != Press::Remove => Some(Switch::Skip {
                category: (*category).to_owned(),
                on: !self.prefs.skipped.iter().any(|c| c == category),
            }),
            _ => None,
        };
        if let Some(switch) = switch {
            switch.apply(&mut self.prefs);
            self.prefs_written();
            return Errand::Keep(switch);
        }
        turn(&target, press, state, self.outline.as_deref())
    }

    /// A press on a row of the files.
    #[allow(clippy::too_many_lines)]
    fn among_the_files(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use ronoctua_i18n::key;
        let purpose = match self.page() {
            PageId::Files(purpose) => purpose,
            _ => return Errand::Nothing,
        };
        let Some(open) = self.top().files.as_ref().map(|f| f.listing.at.clone()) else {
            return Errand::Nothing;
        };
        let named = |p: &std::path::Path| file_name(p);
        match (target, press) {
            (Target::Place, Press::Less | Press::More | Press::Enter) => {
                let places = self.places();
                let n = places.len();
                let now = self.top().files.as_ref().map_or(0, |f| f.place);
                let to = if press == Press::Less {
                    (now + n - 1) % n
                } else {
                    (now + 1) % n
                };
                if let Some(f) = self.top_mut().files.as_mut() {
                    f.place = to;
                }
                self.go_to(&places[to].1, None);
                Errand::Nothing
            }
            (Target::Up, Press::Enter) => {
                if let Some(up) = open.parent() {
                    self.go_to(up, Some(&open));
                }
                Errand::Nothing
            }
            // THE TYPED PATH OR ADDRESS is a question of the door's own line,
            // for the same purpose the files are open for — and the files stay
            // open under it.
            (Target::Typed, Press::Enter) => Errand::Ask(Question::plain(
                match purpose {
                    Purpose::Queue => ENQUEUE,
                    Purpose::Lane => TOGETHER,
                    Purpose::Open => OPEN_SAVED,
                    Purpose::Subtitle => SUBTITLE_FILE,
                    Purpose::Cover => crate::player_question::COVER_FILE,
                },
                false,
            )),
            (Target::Folder(p), Press::Enter) => {
                self.go_to(&p.clone(), None);
                Errand::Nothing
            }
            // TOGETHER, a file is a lane of what plays: chosen, it plays with
            // it on one clock, and the files close — the choice was made, and
            // the page that asked for it is back, on the row that asked.
            (Target::File(p), Press::Enter | Press::Add | Press::PlayNow)
                if purpose == Purpose::Lane =>
            {
                self.back(state, columns, texts);
                let source = p.to_string_lossy().into_owned();
                Errand::TellSaying(
                    crate::player_question::together(source),
                    texts.t_with(key::PLAYER_TOGETHER_ADDED, &[("name", &named(p))]),
                )
            }
            // A SUBTITLE FILE, over the picture: a lane of its own — the
            // subtitles' page is back, on the row that asked for it.
            (Target::File(p), Press::Enter | Press::Add | Press::PlayNow)
                if purpose == Purpose::Subtitle =>
            {
                let path = p.to_string_lossy().into_owned();
                let said = state.map_or(Errand::Nothing, |s| {
                    crate::player_subtitles::lay_over(&path, s, texts)
                });
                if matches!(said, Errand::TellSaying(..)) {
                    self.back(state, columns, texts);
                }
                said
            }
            // A PICTURE FOR THE COVER, put in the file shown: the cover's
            // page is back, on the row that asked for it.
            (Target::File(p), Press::Enter | Press::Add | Press::PlayNow)
                if purpose == Purpose::Cover =>
            {
                let said = state
                    .and_then(|s| crate::player_metadata::cover_picked(&self.meta, s, p, texts));
                if matches!(said, Some(Errand::TellSaying(..))) {
                    self.back(state, columns, texts);
                }
                said.unwrap_or(Errand::Nothing)
            }
            // ONE OF THE PLAYER'S OWN FILES, opened: a set plays, a queue
            // takes the queue's place after a question.
            (Target::File(p), Press::Enter | Press::Add | Press::PlayNow)
                if purpose == Purpose::Open =>
            {
                let p = p.clone();
                self.open_saved(&p, state, columns, texts)
            }
            (Target::Folder(_) | Target::PlayFolder | Target::EnqueueFolder, _)
                if purpose != Purpose::Queue =>
            {
                Errand::Nothing
            }
            // A FILE, INTO THE QUEUE — `RET` and a double click — at its end.
            (Target::File(p), Press::Enter | Press::Add) => Errand::TellSaying(
                put(p.to_string_lossy().into_owned(), false),
                texts.t_with(key::PLAYER_ADDED, &[("name", &named(p))]),
            ),
            // `P`: THE FILE NOW, right after what plays — the queue kept, as
            // a file opened from outside the player is (the owner's rule).
            (Target::File(p), Press::PlayNow) => Errand::TellSaying(
                put(p.to_string_lossy().into_owned(), true),
                texts.t_with(key::PLAYER_PLAYING_NOW, &[("name", &named(p))]),
            ),
            // A FOLDER WHOLE, played now or added: ONE location, which the
            // player makes a playlist named after the folder. What it plays
            // of it is the player's walk; a folder with nothing to play is
            // said here, before it is sent.
            (Target::Folder(_), Press::PlayNow)
            | (Target::PlayFolder, Press::Enter | Press::PlayNow)
            | (Target::Folder(_), Press::Add)
            | (Target::EnqueueFolder | Target::PlayFolder, Press::Add)
            | (Target::EnqueueFolder, Press::Enter) => {
                let folder = match target {
                    Target::Folder(f) => f.clone(),
                    _ => open.clone(),
                };
                if crate::player_browse::gather(&folder).is_empty() {
                    return Errand::Said(texts.t(key::PLAYER_BROWSE_NOTHING_TO_PLAY));
                }
                let play = press == Press::PlayNow
                    || matches!((target, press), (Target::PlayFolder, Press::Enter));
                Errand::TellSaying(
                    put(folder.to_string_lossy().into_owned(), play),
                    texts.t_with(
                        if play {
                            key::PLAYER_PLAYING_NOW
                        } else {
                            key::PLAYER_ADDED
                        },
                        &[("name", &named(&folder))],
                    ),
                )
            }
            _ => Errand::Nothing,
        }
    }

    /// What a press on a row of the metadata or the lookups turned into: an
    /// errand, a question on the door's line — the view keeps what it is
    /// about — or a yes-or-no question on the page, "no" first.
    fn turned(&mut self, turned: crate::player_metadata::Turned) -> Errand {
        use crate::player_metadata::Turned;
        match turned {
            Turned::Errand(errand) => errand,
            Turned::Asked(typing, question) => {
                self.typing = Some(typing);
                Errand::Ask(question)
            }
            Turned::Question(sentence, yes) => self.ask(sentence, yes, Errand::Nothing),
            // A PAGE is opened where the rows are at hand ([`View::keyed`]).
            Turned::Page(_) => Errand::Nothing,
        }
    }

    /// A yes-or-no question on the page about something that cannot be
    /// undone — born on its "no", which does nothing.
    pub(crate) fn ask_about(&mut self, sentence: String, yes: Errand) -> Errand {
        self.ask(sentence, yes, Errand::Nothing)
    }

    /// The picture this view holds under `key` — the cover of the file the
    /// Metadata shows, or a suggested one: what a door draws over a line of
    /// a cover (`player_rows::CoverRow`).
    #[must_use]
    pub fn picture(&self, s: &State, key: &str) -> Option<&crate::player_find::Picture> {
        self.meta
            .cover(s)
            .filter(|(k, _)| k == key)
            .or_else(|| self.found.picture_by_key(key))
    }

    /// Puts a question on screen; `no` is where the cursor starts, and what
    /// `Esc` answers — a question that appears on its own answers no.
    fn ask(&mut self, sentence: String, yes: Errand, no: Errand) -> Errand {
        let top = self.top_mut();
        let back = (top.cursor, top.cell);
        top.cursor = 0;
        top.cell = 0;
        top.moved = false;
        self.asking = Some(Choice {
            sentence,
            back,
            yes: Box::new(yes),
            no: Box::new(no),
            draft: None,
            quoted: Vec::new(),
        });
        Errand::Nothing
    }

    /// The question a SAVED DRAFT puts ([`crate::player_metadata::DraftQuestion`]):
    /// asked as every question is, and withdrawn with its draft.
    fn ask_about_the_draft(&mut self, q: crate::player_metadata::DraftQuestion) {
        self.ask(q.sentence, q.yes, q.no);
        if let Some(c) = self.asking.as_mut() {
            c.draft = Some(q.id);
            c.quoted = q.quoted;
        }
    }

    /// A key while a question is on screen.
    fn answering(
        &mut self,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        self.settle_in(state, columns, texts);
        let rows = self.rows(state, columns, texts);
        let answer = match press {
            Press::Up | Press::Down => {
                self.walk(&rows, press == Press::Down);
                return Errand::Nothing;
            }
            Press::Escape => false,
            Press::Enter => match rows.get(self.cursor()).map(|r| &r.target) {
                Some(Target::Answer(yes)) => *yes,
                _ => return Errand::Nothing,
            },
            _ => return Errand::Nothing,
        };
        let Some(q) = self.asking.take() else {
            return Errand::Nothing;
        };
        let top = self.top_mut();
        (top.cursor, top.cell) = q.back;
        top.moved = true;
        if answer { *q.yes } else { *q.no }
    }

    /// `c`, or the glyph beside the queue's title: a question first — a
    /// queue built by hand is not something one key should take away.
    fn ask_to_clear(&mut self, state: Option<&State>, texts: &ronoctua_i18n::Texts) -> Errand {
        use ronoctua_i18n::key;
        if state.is_none() || self.outline.as_ref().is_none_or(|o| o.entries.is_empty()) {
            return Errand::Said(texts.t(key::PLAYER_QUEUE_EMPTY));
        }
        self.ask(
            texts.t(key::PLAYER_ASK_CLEAR),
            Errand::TellSaying(Ask::Clear, texts.t(key::PLAYER_QUEUE_CLEARED)),
            Errand::Nothing,
        )
    }

    /// `k`, and the transport's glyph of the media in the file: the page of
    /// the first kind there is a choice of — and, on one of those pages
    /// already, the NEXT, in its place: `Esc` goes back to where `k` was
    /// first pressed, not through every kind it walked. With nothing to
    /// choose it says so.
    fn walk_the_media(
        &mut self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let on = match self.page() {
            PageId::Tracks(kind) => Some(kind),
            _ => None,
        };
        let Some(next) = state.and_then(|s| crate::player_tracks::next_page(on, s)) else {
            return Errand::Said(texts.t(ronoctua_i18n::key::PLAYER_TRACKS_NONE));
        };
        if on == Some(next) {
            return Errand::Nothing;
        }
        if on.is_some()
            && let Some(left) = self.levels.pop()
        {
            self.push(PageId::Tracks(next), left.opened_from);
            self.settle_in(state, columns, texts);
        } else {
            self.open(PageId::Tracks(next), false, state, columns, texts);
        }
        Errand::Nothing
    }

    /// An answer the view's door typed came back as `errand`: something to
    /// play together, typed on the page of the files that picks one, closes
    /// that page as a pick does — the choice was made, and the page that
    /// asked for it is back, on the row that asked.
    fn answer_came(
        &mut self,
        errand: &Errand,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        if matches!(errand, Errand::TellSaying(Ask::AddLane { .. }, _))
            && matches!(
                self.page(),
                PageId::Files(Purpose::Lane | Purpose::Subtitle)
            )
        {
            self.back(state, columns, texts);
        }
    }

    /// Closing the player everywhere, a question first — the cursor on its
    /// "no", drawn first: what plays stops, and every door of it goes.
    fn ask_to_quit(&mut self, texts: &ronoctua_i18n::Texts) -> Errand {
        self.ask(
            texts.t(ronoctua_i18n::key::PLAYER_ASK_QUIT),
            Errand::QuitEverywhere,
            Errand::Nothing,
        )
    }

    /// Starting an older player again, a question first — the cursor on its
    /// "no": what plays stops, and the queue is kept.
    fn ask_to_restart(&mut self, texts: &ronoctua_i18n::Texts) -> Errand {
        self.ask(
            texts.t(ronoctua_i18n::key::PLAYER_ASK_RESTART),
            Errand::Restart,
            Errand::Nothing,
        )
    }

    /// `LEADER p d`: THE METADATA — the home, back from any page, the cursor
    /// on the section's first row. With nothing playing there is none, and
    /// that is said.
    fn show_the_metadata(
        &mut self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        if !state.is_some_and(loaded) {
            return Errand::Said(texts.t(ronoctua_i18n::key::PLAYER_NOTHING));
        }
        self.levels.truncate(1);
        self.settle_in(state, columns, texts);
        let rows = self.rows(state, columns, texts);
        let Some(at) = rows
            .iter()
            .position(|r| r.stops() && r.spot.starts_with("meta-"))
        else {
            return Errand::Nothing;
        };
        let top = self.top_mut();
        top.cursor = at;
        top.cell = 0;
        top.moved = true;
        Errand::Nothing
    }

    /// `r`, and the row that renames what plays: a question on the door's
    /// line, holding the name the entry has now — the one under the cursor,
    /// or what plays.
    fn ask_a_name(
        &mut self,
        target: &Target,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use ronoctua_i18n::key;
        let id = match target {
            Target::Entry(id) | Target::Group(id) => Some(*id),
            _ => state.and_then(|s| s.current),
        };
        let Some(id) = id else {
            return Errand::Said(texts.t(key::PLAYER_NOTHING));
        };
        let Some(was) = self
            .outline
            .as_deref()
            .and_then(|o| node_of(o, id))
            .map(|n| name_of(&n.name, texts, NAME_IN_A_SENTENCE))
        else {
            return Errand::Said(texts.t(key::PLAYER_TROUBLE_NO_SUCH_ENTRY));
        };
        self.typing = Some(Typing::Rename {
            id,
            was: was.clone(),
        });
        Errand::Ask(Question::holding(crate::player_question::RENAME, was))
    }

    /// SAVING, its first question: the name — for what plays, the name it has
    /// now; for the queue, none. With nothing to save it says so.
    fn save(
        &mut self,
        saved: Saved,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use crate::player_question::{SAVE_QUEUE, SAVE_SET};
        use ronoctua_i18n::key;
        let question = match saved {
            Saved::Set => {
                let Some(s) = state.filter(|s| loaded(s)) else {
                    return Errand::Said(texts.t(key::PLAYER_NOTHING));
                };
                let named = s
                    .current
                    .zip(self.outline.as_deref())
                    .and_then(|(id, o)| node_of(o, id))
                    .map(|n| name_of(&n.name, texts, NAME_IN_A_SENTENCE))
                    .or_else(|| s.title.clone())
                    .filter(|n| !n.trim().is_empty());
                named.map_or_else(
                    || Question::plain(SAVE_SET, false),
                    |n| Question::holding(SAVE_SET, n),
                )
            }
            Saved::Queue => {
                if self.outline.as_ref().is_none_or(|o| o.entries.is_empty()) {
                    return Errand::Said(texts.t(key::PLAYER_QUEUE_EMPTY));
                }
                Question::plain(SAVE_QUEUE, false)
            }
        };
        self.typing = Some(Typing::SaveName(saved));
        Errand::Ask(question)
    }

    /// What was typed for one of the player's questions, IN THIS VIEW — the
    /// ONE function both doors hand every answer to while the view is open.
    ///
    /// The view's own questions — a new name, the name and the folder a file
    /// is saved under, a set or a queue typed to open — are answered here,
    /// where it is known what they were about ([`Typing`]); every other one
    /// is `player_question::answered_saying`'s. An empty answer is never
    /// mind, as it is for every question.
    pub fn answered(
        &mut self,
        id: &str,
        said: &str,
        pairing: &mut crate::player_machines::Pairing,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use crate::player_question::{
            RENAME, SAVE_QUEUE, SAVE_QUEUE_FOLDER, SAVE_SET, SAVE_SET_FOLDER,
        };
        let asked_before = self.asking.is_some();
        let errand = match id {
            RENAME => self.renamed(said, texts),
            SAVE_SET | SAVE_QUEUE => self.named(said, texts),
            SAVE_SET_FOLDER | SAVE_QUEUE_FOLDER => self.foldered(said, texts),
            crate::player_question::LANE_START => self.started(said, state, texts),
            crate::player_question::LANE_UNTIL => self.ended_at(said, state, texts),
            crate::player_question::LANE_TITLE => self.lane_titled(said, texts),
            // THE LOOKUPS' QUESTIONS: other words for a search, a language
            // to search in, a key, the account.
            crate::player_question::FIND_WORDS => match (self.typing.take(), state) {
                (Some(Typing::Words(kind)), Some(s)) => {
                    let words = said.trim();
                    if words.is_empty() {
                        Errand::Nothing
                    } else {
                        self.query_of(kind, Some(words.to_owned()), s, texts)
                            .map_or(Errand::Nothing, |q| self.asking(q, texts))
                    }
                }
                _ => Errand::Nothing,
            },
            crate::player_question::FIND_LANGUAGE => {
                self.typing = None;
                crate::player_find::language_typed(&mut self.finding, said, texts)
            }
            // A DIRECTORY'S SEARCH: the list of what it names, opened.
            id if crate::player_directory::asks(id) => {
                self.searched(id, said, state, columns, texts)
            }
            crate::player_question::KEY_OPENSUBTITLES
            | crate::player_question::KEY_TMDB
            | crate::player_question::KEY_SUBDL => match self.typing.take() {
                Some(Typing::Key(k)) => self.key_typed(k, said),
                _ => Errand::Nothing,
            },
            crate::player_question::ACCOUNT_USER => match self.typing.take() {
                Some(Typing::AccountUser) => self.account_user_typed(said),
                _ => Errand::Nothing,
            },
            crate::player_question::ACCOUNT_PASSWORD => match self.typing.take() {
                Some(Typing::AccountPassword(user)) => self.account_password_typed(user, said),
                _ => Errand::Nothing,
            },
            // A FIELD OF THE FILE SHOWN, or a tag by its own name, typed:
            // checked, then written — a new tag's name asks its value next.
            id if crate::player_metadata::typed_here(id) => match self.typing.take() {
                Some(Typing::Meta(typed)) => {
                    let turned = crate::player_metadata::typed(typed, said, texts);
                    self.turned(turned)
                }
                other => {
                    self.typing = other;
                    Errand::Nothing
                }
            },
            // A PICTURE FOR THE COVER, typed on the page that picks one.
            crate::player_question::COVER_FILE => match (said.trim(), state) {
                ("", _) | (_, None) => Errand::Nothing,
                (typed, Some(s)) => {
                    let picture = std::path::PathBuf::from(crate::player_question::expand(typed));
                    let said = crate::player_metadata::cover_picked(&self.meta, s, &picture, texts);
                    if matches!(said, Some(Errand::TellSaying(..)))
                        && self.page() == PageId::Files(Purpose::Cover)
                    {
                        self.back(state, columns, texts);
                    }
                    said.unwrap_or(Errand::Nothing)
                }
            },
            OPEN_SAVED => match said.trim() {
                "" => Errand::Nothing,
                typed => {
                    let path = std::path::PathBuf::from(crate::player_question::expand(typed));
                    self.open_saved(&path, state, columns, texts)
                }
            },
            // A SUBTITLE FILE TYPED on the page that picks one: laid over the
            // picture whatever its name says — the person said it is one.
            SUBTITLE_FILE => match (said.trim(), state) {
                ("", _) | (_, None) => Errand::Nothing,
                (typed, Some(s)) => crate::player_subtitles::lay_over(
                    &crate::player_question::expand(typed),
                    s,
                    texts,
                ),
            },
            _ => crate::player_question::answered_saying(id, said, pairing, texts),
        };
        self.answer_came(&errand, state, columns, texts);
        // A QUESTION OF THE VIEW'S THAT JUST OPENED is born on its no, as one
        // a key opens is (`View::key`).
        if !asked_before && self.asking.is_some() {
            self.settle_in(state, columns, texts);
        }
        // AND A LIST A PAGE THAT CAME UP OWES — a search typed opens one.
        self.owing(errand)
    }

    /// The door's line was let go: the view's question with it.
    pub fn let_go(&mut self) {
        self.typing = None;
    }

    /// WHEN LANE `id` BEGINS, asked on the door's line — holding when it
    /// begins now, as a person writes it, so a small change is a small edit.
    fn ask_a_start(
        &mut self,
        id: LaneId,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(lane) = state.and_then(|s| lane_of(s, id)) else {
            return Errand::Said(texts.t(ronoctua_i18n::key::PLAYER_PAGE_GONE));
        };
        self.typing = Some(Typing::LaneStart(id));
        Errand::Ask(Question::holding(
            crate::player_question::LANE_START,
            crate::player_lane::start_said(lane.start_seconds, texts),
        ))
    }

    /// When a lane begins, typed: `m:ss`, `h:mm:ss`, seconds, or a move from
    /// where it begins — `+1.5`, `-0,3` (`player_timeline::parse_start`).
    /// Nothing typed is never mind; anything else that is no time is refused
    /// in a sentence that quotes it.
    fn started(
        &mut self,
        said: &str,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use crate::player_timeline::{Refusal, parse_start};
        let Some(Typing::LaneStart(id)) = self.typing.take() else {
            return Errand::Nothing;
        };
        let Some(lane) = state.and_then(|s| lane_of(s, id)) else {
            return Errand::Said(texts.t(ronoctua_i18n::key::PLAYER_PAGE_GONE));
        };
        match parse_start(said, lane.start_seconds) {
            Ok(seconds) => Errand::Tell(Ask::Lane {
                lane: id,
                change: LaneChange::Start { seconds },
            }),
            Err(Refusal::Empty) => Errand::Nothing,
            Err(Refusal::NotATime) => Errand::Said(texts.t_with(
                ronoctua_i18n::key::PLAYER_LANE_START_REFUSED,
                &[("said", said.trim())],
            )),
        }
    }

    /// WHERE LANE `id` STOPS IN ITS FILE, asked on the door's line — holding
    /// where it stops now, its file's end included, as a person writes it.
    fn ask_an_end(
        &mut self,
        id: LaneId,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some((s, lane)) = state.and_then(|s| lane_of(s, id).map(|l| (s, l))) else {
            return Errand::Said(texts.t(ronoctua_i18n::key::PLAYER_PAGE_GONE));
        };
        self.typing = Some(Typing::LaneUntil(id));
        let now = crate::player_lane::until_now(s, lane);
        Errand::Ask(match now {
            Some(u) => Question::holding(
                crate::player_question::LANE_UNTIL,
                crate::player_lane::start_said(u, texts),
            ),
            None => Question::plain(crate::player_question::LANE_UNTIL, false),
        })
    }

    /// Where a lane stops, typed: a time in its file (`m:ss`, `h:mm:ss`,
    /// seconds) or a move from where it stops now (`+1.5`, `-0,3`). Nothing
    /// typed is never mind; a time that is none, or one at or before where it
    /// begins, is refused in a sentence.
    fn ended_at(
        &mut self,
        said: &str,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use crate::player_timeline::{Refusal, parse_start};
        let Some(Typing::LaneUntil(id)) = self.typing.take() else {
            return Errand::Nothing;
        };
        let Some((s, lane)) = state.and_then(|s| lane_of(s, id).map(|l| (s, l))) else {
            return Errand::Said(texts.t(ronoctua_i18n::key::PLAYER_PAGE_GONE));
        };
        let now = crate::player_lane::until_now(s, lane).unwrap_or(lane.from_seconds);
        match parse_start(said, now) {
            Ok(until) if until > lane.from_seconds => Errand::Tell(Ask::Lane {
                lane: id,
                change: LaneChange::Trim {
                    from_seconds: lane.from_seconds,
                    until_seconds: Some(until),
                },
            }),
            Ok(_) => Errand::Said(texts.t_with(
                ronoctua_i18n::key::PLAYER_LANE_UNTIL_TOO_SOON,
                &[(
                    "from",
                    &crate::player_lane::start_said(lane.from_seconds, texts),
                )],
            )),
            Err(Refusal::Empty) => Errand::Nothing,
            Err(Refusal::NotATime) => Errand::Said(texts.t_with(
                ronoctua_i18n::key::PLAYER_LANE_START_REFUSED,
                &[("said", said.trim())],
            )),
        }
    }

    /// A NAME FOR LANE `id`, asked on the door's line — holding the one it
    /// has: a small change is a small edit.
    fn ask_a_title(
        &mut self,
        id: LaneId,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some((s, lane)) = state.and_then(|s| lane_of(s, id).map(|l| (s, l))) else {
            return Errand::Said(texts.t(ronoctua_i18n::key::PLAYER_PAGE_GONE));
        };
        let was = crate::player_queue::lane_name(lane, &s.lanes, texts, NAME_IN_A_SENTENCE);
        self.typing = Some(Typing::LaneTitle {
            id,
            was: was.clone(),
        });
        Errand::Ask(Question::holding(crate::player_question::LANE_TITLE, was))
    }

    /// A lane's name, typed: unchanged, nothing changes; EMPTIED, it takes its
    /// file's name back — the queue's rule for an entry's name.
    fn lane_titled(&mut self, said: &str, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(Typing::LaneTitle { id, was }) = self.typing.take() else {
            return Errand::Nothing;
        };
        let name = said.trim();
        if name == was.trim() {
            return Errand::Nothing;
        }
        let ask = Ask::Lane {
            lane: id,
            change: LaneChange::Title {
                title: (!name.is_empty()).then(|| name.to_owned()),
            },
        };
        if name.is_empty() {
            return Errand::TellSaying(
                ask,
                texts.t_with(
                    ronoctua_i18n::key::PLAYER_RENAMED_BACK,
                    &[("was", was.as_str())],
                ),
            );
        }
        Errand::TellSaying(
            ask,
            texts.t_with(ronoctua_i18n::key::PLAYER_RENAMED, &[("name", name)]),
        )
    }

    /// A new name, typed: the entry it was asked for is renamed — and the
    /// name it already had changes nothing. EMPTIED, the entry gets its own
    /// name back — the file's, the sources' joined — which it had no other
    /// way to get once renamed (`Esc` is never mind; a playlist, which has
    /// no name but the one it is given, is refused by the player).
    fn renamed(&mut self, said: &str, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(Typing::Rename { id, was }) = self.typing.take() else {
            return Errand::Nothing;
        };
        let name = said.trim();
        if name == was.trim() {
            return Errand::Nothing;
        }
        if name.is_empty() {
            return Errand::TellSaying(
                Ask::Rename {
                    id,
                    name: String::new(),
                },
                texts.t_with(
                    ronoctua_i18n::key::PLAYER_RENAMED_BACK,
                    &[("was", was.as_str())],
                ),
            );
        }
        Errand::TellSaying(
            Ask::Rename {
                id,
                name: name.to_owned(),
            },
            texts.t_with(ronoctua_i18n::key::PLAYER_RENAMED, &[("name", name)]),
        )
    }

    /// The name a file is saved under, typed: the folder is asked next,
    /// holding the music folder's path. A name that cannot be one file's is
    /// refused in a sentence.
    fn named(&mut self, said: &str, texts: &ronoctua_i18n::Texts) -> Errand {
        use crate::player_question::{SAVE_QUEUE_FOLDER, SAVE_SET_FOLDER};
        let Some(Typing::SaveName(saved)) = self.typing.take() else {
            return Errand::Nothing;
        };
        let Some(name) = file_stem_of(said, saved) else {
            return Errand::Nothing;
        };
        if name.contains(['/', '\0']) || name == "." || name == ".." {
            return Errand::Said(texts.t(ronoctua_i18n::key::PLAYER_SAVE_BAD_NAME));
        }
        self.typing = Some(Typing::SaveFolder(saved, name));
        let music = self.places()[0].1.clone();
        Errand::Ask(Question::holding(
            match saved {
                Saved::Set => SAVE_SET_FOLDER,
                Saved::Queue => SAVE_QUEUE_FOLDER,
            },
            shown_path(&music),
        ))
    }

    /// The folder, typed: the file is written there — after a question, "no"
    /// first, when one of that name is already there. A folder that is not
    /// one is refused in a sentence.
    fn foldered(&mut self, said: &str, texts: &ronoctua_i18n::Texts) -> Errand {
        use ronoctua_i18n::key;
        let Some(Typing::SaveFolder(saved, name)) = self.typing.take() else {
            return Errand::Nothing;
        };
        let said = said.trim();
        if said.is_empty() {
            return Errand::Nothing;
        }
        let folder = std::path::PathBuf::from(crate::player_question::expand(said));
        let shown = shown_path(&folder);
        if !folder.is_dir() {
            return Errand::Said(texts.t_with(key::PLAYER_SAVE_NO_FOLDER, &[("folder", &shown)]));
        }
        let file = format!("{name}.{}", saved.extension());
        let at = folder.join(&file);
        let to = at.to_string_lossy().into_owned();
        let ask = match saved {
            Saved::Set => Ask::SaveSet {
                to,
                name: Some(name),
            },
            Saved::Queue => Ask::SaveQueue {
                to,
                name: Some(name),
            },
        };
        let go = Errand::TellSaying(
            ask,
            texts.t_with(key::PLAYER_SAVING, &[("name", &file), ("folder", &shown)]),
        );
        // A FILE THAT IS THERE is written over only after a question — it
        // cannot be undone, and it is born on its "no".
        if at.exists() {
            return self.ask(
                texts.t_with(
                    key::PLAYER_ASK_OVERWRITE,
                    &[("name", &file), ("folder", &shown)],
                ),
                go,
                Errand::Nothing,
            );
        }
        go
    }

    /// One of the player's own files, opened — picked on its page, or typed:
    /// a SET plays now, right after what plays, the queue kept; a QUEUE takes
    /// the queue's place, after a question ("no" first) while the queue holds
    /// anything — the one place the queue is replaced. The page that picked
    /// it closes: the choice was made.
    fn open_saved(
        &mut self,
        path: &std::path::Path,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use ronoctua_i18n::key;
        use ronoctua_player::own::{OwnKind, kind_by_name};
        let name = file_name(path);
        if self.page() == PageId::Files(Purpose::Open) {
            self.back(state, columns, texts);
        }
        let location = path.to_string_lossy().into_owned();
        match kind_by_name(path) {
            Some(OwnKind::Set) => Errand::TellSaying(
                put(location, true),
                texts.t_with(key::PLAYER_PLAYING_NOW, &[("name", &name)]),
            ),
            Some(OwnKind::Queue) => {
                let open = Errand::TellSaying(
                    Ask::Put {
                        what: vec![Thing::Location(location)],
                        after: None,
                        into: None,
                        play: false,
                        replace: true,
                    },
                    texts.t_with(key::PLAYER_QUEUE_OPENED, &[("name", &name)]),
                );
                if self.outline.as_ref().is_some_and(|o| !o.entries.is_empty()) {
                    self.ask(
                        texts.t_with(key::PLAYER_ASK_REPLACE_QUEUE, &[("name", &name)]),
                        open,
                        Errand::Nothing,
                    )
                } else {
                    open
                }
            }
            None => Errand::Said(texts.t_with(key::PLAYER_OPEN_NOT_OURS, &[("name", &name)])),
        }
    }

    /// The rows of a page of the files: the folder (its place a ring on
    /// `←`/`→`), a path or an address to type, the folder whole, and the
    /// entries — `..` first — in columns.
    fn files_rows(files: &Files, purpose: Purpose, b: &mut Builder<'_>) {
        use ronoctua_i18n::key;
        let l = &files.listing;
        b.path_pair(
            b.t(key::PLAYER_BROWSE_FOLDER),
            shown_path(&l.at),
            Target::Place,
            "browse-folder",
        );
        b.pair(
            b.t(key::PLAYER_BROWSE_TYPE),
            String::new(),
            Target::Typed,
            "browse-type",
        );
        if purpose == Purpose::Queue {
            b.pair(
                b.t(key::PLAYER_BROWSE_PLAY_FOLDER),
                String::new(),
                Target::PlayFolder,
                "browse-play-folder",
            );
            b.pair(
                b.t(key::PLAYER_BROWSE_ENQUEUE_FOLDER),
                String::new(),
                Target::EnqueueFolder,
                "browse-enqueue-folder",
            );
        }
        if purpose == Purpose::Open {
            b.inert(crate::chrome::AIR);
            b.note(key::PLAYER_ABOUT_OPEN);
        }
        b.inert(crate::chrome::AIR);
        if let Some(why) = &l.refused {
            let said = b
                .texts
                .t_with(key::PLAYER_BROWSE_REFUSED, &[("why", why.as_str())]);
            b.warning(&said);
        }
        // `..` FIRST, and always — the way back is where the eye starts.
        let mut entries: Vec<(char, String, Target, String)> = Vec::new();
        if l.at.parent().is_some() {
            entries.push((
                tokens::icon::FOLDER,
                "..".to_owned(),
                Target::Up,
                "browse-..".to_owned(),
            ));
        }
        for f in &l.folders {
            entries.push((
                tokens::icon::FOLDER,
                file_name(f),
                Target::Folder(f.clone()),
                format!("browse-{}", f.display()),
            ));
        }
        for (f, glyph) in l.files.iter().zip(&l.glyphs) {
            entries.push((
                *glyph,
                file_name(f),
                Target::File(f.clone()),
                format!("browse-{}", f.display()),
            ));
        }
        b.grid(&entries);
        if l.refused.is_none() && l.folders.is_empty() && l.files.is_empty() {
            b.note(match purpose {
                Purpose::Open => key::PLAYER_BROWSE_EMPTY_SAVED,
                Purpose::Cover => key::PLAYER_BROWSE_EMPTY_PICTURES,
                Purpose::Queue | Purpose::Lane | Purpose::Subtitle => key::PLAYER_BROWSE_EMPTY,
            });
        }
    }

    /// Every row of the page on screen, in reading order — a page above the
    /// home under its title, the page's own key.
    #[must_use]
    pub fn rows(
        &self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let (left, room) = crate::power::extent(columns);
        let top = self.top();
        Builder::build(left, room, top.cursor, top.cell, texts, |b| {
            self.page_rows(state, b);
        })
    }

    /// The rows of the page on screen, written with `b` — once to measure
    /// and once to place ([`Builder::build`]).
    fn page_rows(&self, state: Option<&State>, b: &mut Builder<'_>) {
        use ronoctua_i18n::key;
        let top = self.top();
        // A QUESTION ON SCREEN is all that is drawn: it waits for an answer.
        if let Some(q) = &self.asking {
            b.heading(key::PLAYER_HEADING_QUESTION);
            for line in crate::chrome::note_lines(&q.sentence, b.room) {
                b.inert(vec![Builder::piece(
                    b.left,
                    line,
                    tokens::color::TEXT_SECONDARY,
                )]);
            }
            b.pair(
                b.t(key::PLAYER_ANSWER_NO),
                String::new(),
                Target::Answer(false),
                "no",
            );
            b.pair(
                b.t(key::PLAYER_ANSWER_YES),
                String::new(),
                Target::Answer(true),
                "yes",
            );
            // WHAT IT WOULD WRITE, under the answers — which stay in sight —
            // wrapped and whole: a person says yes to words they read.
            if !q.quoted.is_empty() {
                b.heading_within(b.t(key::PLAYER_DRAFT_SAYS));
                for said in &q.quoted {
                    let lines = crate::chrome::note_lines(said, b.room);
                    for line in lines {
                        b.inert(vec![Builder::piece(
                            b.left,
                            line,
                            crate::chrome::Kind::Fact.tones(false).1,
                        )]);
                    }
                }
            }
            return;
        }
        let page = top.page;
        // A LIST'S PAGE is titled by what it lists — a country, a genre, what
        // was searched for — and by its own key while it lists nothing.
        if let Some(title) = self.listing_title(b.texts) {
            b.heading_said(title);
        } else if page != PageId::Home {
            b.heading(page.key());
        }
        // A PAGE WHOSE SUBJECT IS GONE stays, and says so — never blank.
        match page.subject(state) {
            Subject::NotRunning => {
                b.note(key::PLAYER_NOT_RUNNING);
                return;
            }
            Subject::Gone => {
                b.note(key::PLAYER_PAGE_GONE);
                return;
            }
            Subject::Here => {}
        }
        match (page, state) {
            (PageId::Home, _) => self.home(state, b),
            (PageId::Files(purpose), _) => {
                if let Some(files) = &top.files {
                    Self::files_rows(files, purpose, b);
                }
            }
            (PageId::Internet, _) => self.internet(b),
            (PageId::Together, Some(s)) => crate::player_home::together(s, b),
            (PageId::Machines, Some(s)) => crate::player_machines::machines(b, s),
            (PageId::Tracks(TrackKind::Subtitles), Some(s)) => {
                crate::player_subtitles::page(s, &self.beside, b);
            }
            (PageId::Tracks(kind), Some(s)) => crate::player_tracks::page(kind, s, b),
            (PageId::Ambience, Some(s)) => crate::player_ambience::page(s, b),
            (PageId::AmbienceCredits, Some(s)) => crate::player_ambience::credits(s, b),
            (PageId::Lane(id), Some(s)) => crate::player_lane::page(id, s, &self.screens, b),
            (PageId::Timeline, Some(s)) => crate::player_lane::timeline(s, b),
            (PageId::Layout, Some(s)) => {
                crate::player_layout::page(s, (self.here.as_deref(), self.pictures_room), b);
            }
            (PageId::Cover, Some(s)) => crate::player_metadata::cover_page(&self.meta, s, b),
            (PageId::Streams, Some(s)) => crate::player_metadata::streams_page(&self.meta, s, b),
            (PageId::Chapters, Some(s)) => {
                crate::player_metadata::chapters_page(&self.meta, s, b);
            }
            (PageId::EveryTag, Some(s)) => {
                crate::player_metadata::every_tag_page(&self.meta, s, b);
            }
            (PageId::Suggestions, Some(s)) => crate::player_suggest::page(self, s, b),
            (PageId::Candidate(id), Some(s)) => {
                crate::player_suggest::candidate_page(self, id, s, b);
            }
            (PageId::FindSubtitles, Some(s)) => crate::player_find::subtitles_page(self, s, b),
            (PageId::FindLyrics, Some(s)) => crate::player_find::lyrics_page(self, s, b),
            (PageId::Languages, _) => crate::player_find::languages_page(self, b),
            (PageId::Field(field), Some(s)) => {
                crate::player_metadata::field_page(&self.meta, field, s, b);
            }
            // THE DIRECTORIES: world radio and TV, their lists, the themes,
            // the favourites and the played lately.
            (
                PageId::Radio
                | PageId::RadioGenres
                | PageId::RadioLanguages
                | PageId::Stations
                | PageId::Tv
                | PageId::TvChannels
                | PageId::Theme(_)
                | PageId::Favourites
                | PageId::Recent,
                Some(s),
            ) => self.directory_rows(page, s, b),
            (PageId::Channels, _) => self.directory_rows(page, &State::default(), b),
            (
                PageId::Together
                | PageId::Machines
                | PageId::Tracks(_)
                | PageId::Ambience
                | PageId::AmbienceCredits
                | PageId::Lane(_)
                | PageId::Timeline
                | PageId::Layout
                | PageId::Cover
                | PageId::Streams
                | PageId::Chapters
                | PageId::EveryTag
                | PageId::Suggestions
                | PageId::Candidate(_)
                | PageId::FindSubtitles
                | PageId::FindLyrics
                | PageId::Field(_)
                | PageId::Radio
                | PageId::RadioGenres
                | PageId::RadioLanguages
                | PageId::Stations
                | PageId::Tv
                | PageId::TvChannels
                | PageId::Theme(_)
                | PageId::Favourites
                | PageId::Recent,
                None,
            ) => {}
        }
    }
}

/// Puts one location in the queue: at its end, or — `play` — right after
/// what plays, and plays it, the queue kept (what opening a file from outside
/// does; starting the queue over is only the player's own "open queue").
#[must_use]
pub(crate) fn put(location: String, play: bool) -> Ask {
    Ask::Put {
        what: vec![Thing::Location(location)],
        after: None,
        into: None,
        play,
        replace: false,
    }
}

/// The lane `id` of what plays, when it still has it.
fn lane_of(s: &State, id: LaneId) -> Option<&ronoctua_player::own::Lane> {
    s.lanes.iter().find(|l| l.id == id)
}

/// What a press does on a row. Pure: it reads the state and the outline, and
/// says what to ask.
fn turn(target: &Target, press: Press, state: Option<&State>, outline: Option<&Outline>) -> Errand {
    let forward = !matches!(press, Press::Less);
    let default = State::default();
    let s = state.unwrap_or(&default);
    match (target, press) {
        (Target::Nothing, _) => Errand::Nothing,
        (Target::Pair, Press::Enter) => {
            Errand::Ask(Question::plain(crate::player_question::PAIR, false))
        }
        (Target::Playing, _) => Errand::Tell(Ask::Toggle),
        (Target::Position, Press::Enter) => Errand::Tell(Ask::Seek {
            seconds: JUMP_STEP,
            relative: true,
        }),
        (Target::Position, Press::Less | Press::More) => Errand::Tell(Ask::Seek {
            seconds: if forward { SEEK_STEP } else { -SEEK_STEP },
            relative: true,
        }),
        (Target::Volume, Press::Enter) => Errand::Tell(Ask::Mute),
        (Target::Volume, Press::Less | Press::More) => Errand::Tell(Ask::Volume {
            percent: stepped_volume(s.volume, forward),
        }),
        (Target::Speed, _) => {
            let at = SPEEDS
                .iter()
                .position(|x| (x - s.speed).abs() < 0.01)
                .unwrap_or(2);
            let n = SPEEDS.len();
            Errand::Tell(Ask::Speed {
                factor: SPEEDS[if forward {
                    (at + 1) % n
                } else {
                    (at + n - 1) % n
                }],
            })
        }
        (Target::Repeat, _) => Errand::Tell(Ask::Repeat),
        (Target::Shuffle, _) => Errand::Tell(Ask::Shuffle),
        (Target::Go(PageId::Tracks(TrackKind::Sound)), Press::Less | Press::More) => {
            next_track(s, TrackKind::Sound, forward, false).map_or(Errand::Nothing, Errand::Tell)
        }
        (Target::Go(PageId::Tracks(TrackKind::Subtitles)), Press::Less | Press::More) => {
            next_track(s, TrackKind::Subtitles, forward, true).map_or(Errand::Nothing, Errand::Tell)
        }
        // ONE OF THE FILE'S MEDIA, on its kind's page: its lane plays that
        // track — the file stays one file — and "no subtitles" switches the
        // subtitles' lane off.
        (Target::Track { kind, id }, Press::Enter) => crate::player_tracks::chosen(*kind, *id, s),
        (Target::Entry(id) | Target::Group(id), Press::Enter) => {
            Errand::Tell(Ask::Jump { id: *id })
        }
        (Target::Entry(id) | Target::Group(id), Press::Remove) => {
            Errand::Tell(Ask::Remove { id: *id })
        }
        // ONE PLACE EARLIER OR LATER among its siblings — a file, a
        // playlist, a set alike: `C-↑`/`C-↓` on its row, and the `↑`/`↓` at
        // the end of it.
        (Target::Entry(id) | Target::Group(id), Press::Earlier | Press::Later) => outline
            .and_then(|o| moved(o, *id, press == Press::Later))
            .map_or(Errand::Nothing, Errand::Tell),
        (Target::EntryMove { id, later }, Press::Enter) => outline
            .and_then(|o| moved(o, *id, *later))
            .map_or(Errand::Nothing, Errand::Tell),
        // WHEN A LANE BEGINS, half a second a step — on its row wherever it
        // is, and on its page — never before the set's first second.
        (Target::Lane(id) | Target::LaneStart(id), Press::Less | Press::More) => lane_of(s, *id)
            .map_or(Errand::Nothing, |lane| {
                let step = crate::player_timeline::OFFSET_STEP_SECONDS;
                let by = if forward { step } else { -step };
                Errand::Tell(Ask::Lane {
                    lane: lane.id,
                    change: LaneChange::Start {
                        seconds: (lane.start_seconds + by).max(0.0),
                    },
                })
            }),
        // WHERE IT STOPS IN ITS FILE, half a second a step; `d` gives it its
        // end back.
        (Target::LaneUntil(id), Press::Less | Press::More) => lane_of(s, *id)
            .map_or(Errand::Nothing, |lane| {
                crate::player_lane::until_stepped(s, lane, forward)
            }),
        (Target::LaneUntil(id), Press::Remove) => lane_of(s, *id).map_or(Errand::Nothing, |lane| {
            if lane.until_seconds.is_none() {
                return Errand::Nothing;
            }
            Errand::Tell(Ask::Lane {
                lane: lane.id,
                change: LaneChange::Trim {
                    from_seconds: lane.from_seconds,
                    until_seconds: None,
                },
            })
        }),
        // WHICH PICTURE A SUBTITLE SITS OVER: a ring, any key turns it.
        (Target::LaneOver(id), Press::Enter | Press::Less | Press::More) => {
            crate::player_lane::over_turned(s, *id, forward)
        }
        // ITS OWN VOLUME: the arrows a step, `RET` its mute — the home's
        // volume row's rule.
        (Target::LaneVolume(id), Press::Less | Press::More) => {
            lane_of(s, *id).map_or(Errand::Nothing, |lane| {
                let most = ronoctua_player::own::VOLUME_MOST_PERCENT;
                let now = lane.volume_percent.min(most);
                Errand::Tell(Ask::Lane {
                    lane: lane.id,
                    change: LaneChange::Volume {
                        percent: if forward {
                            now.saturating_add(VOLUME_STEP).min(most)
                        } else {
                            now.saturating_sub(VOLUME_STEP)
                        },
                    },
                })
            })
        }
        (Target::LaneVolume(id), Press::Enter)
        | (Target::LaneMute(id), Press::Enter | Press::Less | Press::More) => lane_of(s, *id)
            .map_or(Errand::Nothing, |lane| {
                Errand::Tell(Ask::Lane {
                    lane: lane.id,
                    change: LaneChange::Mute { muted: !lane.muted },
                })
            }),
        // WHETHER ITS PICTURE IS SHOWN: the arrangement, whole, with the
        // lane moved in or out of what is hidden — or, for a sound, of the
        // covers shown.
        (Target::LaneShown(id), Press::Enter | Press::Less | Press::More) => {
            crate::player_lane::shown_turned(s, *id).map_or(Errand::Nothing, Errand::Tell)
        }
        (Target::Lane(lane) | Target::Subtitle(lane), Press::Remove) => Errand::Tell(Ask::Lane {
            lane: *lane,
            change: LaneChange::Remove,
        }),
        // SHOWN OR HIDDEN: `v` on a lane's row, a click on a subtitle's chip,
        // and any key on the switch of its page — the lane as it is now,
        // turned.
        (Target::Subtitle(id) | Target::Lane(id), Press::Shown)
        | (Target::Shown(id), Press::Enter)
        | (Target::LaneOn(id), Press::Enter | Press::Less | Press::More | Press::Shown) => s
            .lanes
            .iter()
            .find(|l| l.id == *id)
            .map_or(Errand::Nothing, |lane| {
                Errand::Tell(Ask::Lane {
                    lane: lane.id,
                    change: LaneChange::Enabled { on: !lane.enabled },
                })
            }),
        // WHERE A SUBTITLE'S LINES SIT: the foot of the picture or its top, a
        // ring of two — either arrow turns it, on its row and on its page.
        (Target::Subtitle(id), Press::Less | Press::More)
        | (Target::LaneStack(id), Press::Enter | Press::Less | Press::More) => s
            .lanes
            .iter()
            .find(|l| l.id == *id)
            .map_or(Errand::Nothing, |lane| {
                use ronoctua_player::own::Stack;
                Errand::Tell(Ask::Lane {
                    lane: lane.id,
                    change: LaneChange::Stack {
                        stack: match lane.stack {
                            Stack::Bottom => Stack::Top,
                            Stack::Top => Stack::Bottom,
                        },
                    },
                })
            }),
        // AN AMBIENT SOUND: the arrows its own volume, a step at a time, on
        // or off (`RET` turns it on or off, `View::act`).
        (Target::Sound(id), Press::Less | Press::More) => crate::player_ambience::sound(s, id)
            .map_or(Errand::Nothing, |sound| {
                Errand::Tell(Ask::AmbientVolume {
                    id: sound.id.clone(),
                    percent: stepped_volume(sound.volume_percent, forward),
                })
            }),
        // THE ONES THAT ARE ON, HELD — or let go on: a switch.
        (Target::Hold, Press::Enter | Press::Less | Press::More) => {
            Errand::Tell(Ask::AmbientToggle)
        }
        // A MACHINE: `RET` joins it, or leaves it when joined; the arrows turn
        // what this machine plays of it; `d` forgets the pairing.
        (Target::Host(h), Press::Enter) => {
            if s.joined.as_ref().is_some_and(|j| &j.host == h) {
                Errand::Tell(Ask::Leave)
            } else {
                Errand::Tell(Ask::Join {
                    host: h.clone(),
                    role: Role::Both,
                })
            }
        }
        (Target::Host(h), Press::Less | Press::More) => {
            let now = s
                .joined
                .as_ref()
                .filter(|j| &j.host == h)
                .map_or(Role::Both, |j| j.role);
            let role = if forward {
                now.next()
            } else {
                now.next().next()
            };
            Errand::Tell(Ask::Join {
                host: h.clone(),
                role,
            })
        }
        (Target::Host(h) | Target::Guest(h), Press::Remove) => {
            Errand::Tell(Ask::Forget { name: h.clone() })
        }
        (Target::Invite, Press::Enter) => Errand::Tell(Ask::Invite),
        _ => Errand::Nothing,
    }
}

/// Where a door hears how an ask it told off the loop ended ([`Told`]).
pub type ToldSender = smithay_client_toolkit::reexports::calloop::channel::Sender<Told>;

/// Carries out what is the same through every door — telling the player — and
/// gives back what the door should say NOW. What only a door can do — closing,
/// a stage, the swap, a question, a pairing — comes back untouched as `Err`.
///
/// # Off the loop
///
/// An ask is told on a thread of its own ([`tell_then`]): a door's loop is
/// the desktop's, and the player may take a moment to answer — a folder put
/// in the queue is walked before the answer. What the ask says NOW is the
/// sentence of what was done; a refusal, when it comes, is sent down `told`
/// and said in its place, in the door that asked. With no `told` (a test's
/// door) the answer is only logged.
///
/// # Errors
///
/// The errand is one the DOOR carries out.
pub fn carry_out(
    errand: Errand,
    texts: &ronoctua_i18n::Texts,
    told: Option<&ToldSender>,
) -> Result<Option<String>, Errand> {
    let tell_off_the_loop = |ask: Ask| {
        let told = told.cloned();
        // A DIRECTORY'S LIST comes back with the list it was for: its page
        // takes a refusal in place of "asking".
        let listing = match &ask {
            Ask::Find { query } if query.directory().is_some() => Some(query.clone()),
            _ => None,
        };
        tell_then(ask, move |answer| {
            let answer = match listing {
                Some(query) => Told::Listing {
                    query,
                    answer: Box::new(answer),
                },
                None => answer,
            };
            if let Some(told) = told {
                let _ = told.send(answer);
            } else if answer != Told::Taken {
                tracing::info!(?answer, "the player's answer, with no door to say it");
            }
        });
    };
    match errand {
        Errand::Nothing => Ok(None),
        // ONE, THEN THE OTHER: what the first says, unless the second says
        // something — a page's lists asked after what opened it say nothing.
        Errand::Then(first, then) => {
            let said = carry_out(*first, texts, told)?;
            Ok(carry_out(*then, texts, told)?.or(said))
        }
        // A PAIRING IS THE DOOR'S TOO: it is sent off the loop
        // (`player_machines::pair_then`), because the player answers it only
        // after a round trip to the other machine.
        Errand::Close
        | Errand::Show(_)
        | Errand::Swap
        | Errand::QuitEverywhere
        | Errand::EveryScreen
        | Errand::Ask(_)
        | Errand::Pair { .. } => Err(errand),
        Errand::Said(sentence) => Ok(Some(sentence)),
        // THE SENTENCE NOW; a refusal, when it comes, is said in its place.
        Errand::TellSaying(ask, sentence) => {
            tell_off_the_loop(ask);
            Ok(Some(sentence))
        }
        Errand::Tell(ask) => {
            tell_off_the_loop(ask);
            Ok(None)
        }
        Errand::Restart => {
            restart(told.cloned());
            Ok(Some(texts.t(ronoctua_i18n::key::PLAYER_RESTARTING)))
        }
        // THE CONSENT, merged into what is on disk — and then what the press
        // asked, after it: the daemon reads the file when it asks.
        Errand::Let {
            services,
            directories,
            on,
            then,
        } => {
            crate::daemon::about_to_act("the player's consents, written");
            match let_merged(&services, &directories, on) {
                Ok(()) => carry_out(*then, texts, told),
                Err(why) => Ok(Some(why.sentence(texts))),
            }
        }
        // A KEY OF THE PERSON'S, kept private from its first byte and tried
        // at once — both OFF THE LOOP, on a thread of their own: the key is
        // synced to the disk before it is renamed into place, and a disk
        // that takes its time must not hold the desktop. What is said now is
        // that it is being tried; a key that could not be kept, or the
        // service's answer, is said when it comes.
        Errand::KeepKey { key, secret } => {
            crate::daemon::about_to_act("a key of the person's, kept");
            Ok(Some(key_kept_and_tried(
                key.service(),
                move || ronoctua_lookup::keys::keep(key, &secret),
                told.cloned(),
                texts,
            )))
        }
        Errand::KeepAccount { user, password } => {
            crate::daemon::about_to_act("the person's OpenSubtitles account, kept");
            let account = ronoctua_lookup::keys::Account { user, password };
            Ok(Some(key_kept_and_tried(
                ronoctua_lookup::Service::OpenSubtitles,
                move || ronoctua_lookup::keys::keep_account(&account),
                told.cloned(),
                texts,
            )))
        }
        Errand::ForgetKey(key) => {
            crate::daemon::about_to_act("a key of the person's, forgotten");
            let told = told.cloned();
            let born = std::thread::Builder::new()
                .name("player-key-forget".to_owned())
                .spawn(move || {
                    if let Err(t) = ronoctua_lookup::keys::forget(key)
                        && let Some(told) = told
                    {
                        let _ = told.send(Told::KeyTrouble(t));
                    }
                });
            if let Err(e) = born {
                tracing::error!(error = %e, "could not start the thread that forgets a key");
            }
            Ok(Some(texts.t(ronoctua_i18n::key::PLAYER_KEY_FORGOTTEN)))
        }
        // ONE SWITCH, merged into what is on disk — the same rule as the
        // consents: this door's copy is what it shows, never what it writes.
        Errand::Keep(switch) => Ok({
            crate::daemon::about_to_act("the player's preferences, written");
            match kept_merged(&switch) {
                // REREAD BY A PLAYER THAT RUNS, and none started for it: the
                // rows of the preferences are there with no player too, and a
                // preference changed after closing the player everywhere
                // brought it back. One that starts later reads them itself.
                Ok(()) => {
                    tell_if_running(&Ask::Reload);
                    None
                }
                Err(why) => Some(why.sentence(texts)),
            }
        }),
    }
}

/// `switch` written into `player.json` MERGED with what is on disk: read as
/// it is there, changed only in what was pressed, and written back whole. A
/// file that will not parse is refused, never written over.
fn kept_merged(switch: &Switch) -> Result<(), NotLet> {
    let store = ronoctua_player::prefs::store();
    let mut p = store.parsed().ok_or(NotLet::WillNotParse)?;
    switch.apply(&mut p);
    store.write(&p).map_err(NotLet::Unwritable)
}

/// Why the consents could not be written.
#[derive(Debug)]
enum NotLet {
    /// `player.json` is there and will not parse: writing the whole of it
    /// back would replace what is in it.
    WillNotParse,
    /// It could not be written.
    Unwritable(std::io::Error),
}

impl NotLet {
    /// In a sentence.
    fn sentence(&self, texts: &ronoctua_i18n::Texts) -> String {
        match self {
            Self::WillNotParse => texts.t(ronoctua_i18n::key::PLAYER_PREFS_WILL_NOT_PARSE),
            Self::Unwritable(e) => texts.t_with(
                ronoctua_i18n::key::PLAYER_TROUBLE_WRITE,
                &[("why", &e.to_string())],
            ),
        }
    }
}

/// The services and directories let — or no longer — written into
/// `player.json` MERGED with what is on disk: read as it is there, changed
/// only in what was pressed, and written back whole. A file that will not
/// parse is refused, never written over.
fn let_merged(
    services: &[ronoctua_lookup::Service],
    directories: &[ronoctua_player::find::Directory],
    on: bool,
) -> Result<(), NotLet> {
    let store = ronoctua_player::prefs::store();
    let mut p = store.parsed().ok_or(NotLet::WillNotParse)?;
    p.let_all(services, directories, on);
    store.write(&p).map_err(NotLet::Unwritable)
}

/// Keeps a key (`keep`) and has the player try it for `service`, off the
/// loop — on a thread of its own, never the teller's: the key is synced to
/// the disk, a service takes seconds to answer, and a pause pressed meanwhile
/// must not wait for either. A key that could not be kept, or the service's
/// answer, is said down `told` when it comes; what is said now is that it is
/// being tried.
fn key_kept_and_tried(
    service: ronoctua_lookup::Service,
    keep: impl FnOnce() -> Result<(), ronoctua_lookup::keys::KeyTrouble> + Send + 'static,
    told: Option<ToldSender>,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let born = std::thread::Builder::new()
        .name("player-key-check".to_owned())
        .spawn(move || {
            use ronoctua_player::wire::Say;
            if let Err(t) = keep() {
                if let Some(told) = told {
                    let _ = told.send(Told::KeyTrouble(t));
                }
                return;
            }
            let answer = match ronoctua_player::client::key_check(service) {
                Ok(Say::Done) => Told::KeyTaken(service),
                Ok(Say::Refused(t)) => Told::Refused(t),
                Ok(Say::Farewell) => Told::NotRunning,
                Ok(_) => Told::Unheard,
                Err(e) if e.is::<ronoctua_player::client::NotInstalled>() => Told::NotInstalled,
                Err(_) => Told::NotRunning,
            };
            if let Some(told) = told {
                let _ = told.send(answer);
            }
        });
    if let Err(e) = born {
        tracing::error!(error = %e, "could not start the thread that keeps and tries a key");
    }
    texts.t_with(
        ronoctua_i18n::key::PLAYER_KEY_CHECKING,
        &[("service", &texts.t(service_key(service)))],
    )
}

/// Tells the player `ask` OFF THE LOOP, starting it when nobody answers, and
/// hands `done` how it ended — never on a door's loop, which is the
/// desktop's ([`carry_out`]).
///
/// # In the order they were asked
///
/// Every ask goes down ONE LINE to one teller thread ([`teller`]), which
/// tells them one at a time and waits for each word before the next: the
/// player serves each connection on a thread of its own, behind a lock that
/// is not first come first served, so a thread per ask let a seek dragged
/// along the bar land an earlier place after the last one, and two things
/// put in the queue swap. A setting told again before its turn came —
/// the position, a volume — is told once, at its newest ([`supersedes`]):
/// a drag is sixty asks a second, and the player needs the last.
///
/// A PUT has a line of its own: the player walks the folder or the playlist
/// before it answers, outside its lock, and a pause pressed meanwhile does
/// not wait for that walk. Two puts still keep their order.
pub fn tell_then(ask: Ask, done: impl FnOnce(Told) + Send + 'static) {
    crate::daemon::about_to_act("the player's daemon, told");
    let walks = matches!(ask, Ask::Put { .. });
    let telling = Telling {
        ask,
        done: Box::new(done),
    };
    match teller(walks) {
        Some(line) => {
            if let Err(e) = line.send(telling) {
                tracing::error!(error = %e, "the thread that tells the player is gone");
            }
        }
        None => tracing::error!("no thread tells the player; the ask is not sent"),
    }
}

/// One ask waiting its turn, and who hears how it ended.
struct Telling {
    ask: Ask,
    done: Box<dyn FnOnce(Told) + Send>,
}

/// The line to this process's teller — the walks' ([`tell_then`]) or every
/// other ask's — started the first time something is told. `None` when its
/// thread could not be started.
fn teller(walks: bool) -> Option<&'static std::sync::mpsc::Sender<Telling>> {
    type Line = std::sync::OnceLock<Option<std::sync::mpsc::Sender<Telling>>>;
    static ASKS: Line = std::sync::OnceLock::new();
    static WALKS: Line = std::sync::OnceLock::new();
    let (cell, name) = if walks {
        (&WALKS, "player-walks")
    } else {
        (&ASKS, "player-tell")
    };
    cell.get_or_init(|| {
        let (line, asks) = std::sync::mpsc::channel::<Telling>();
        let born = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                let mut found = None;
                tell_in_order(&asks, |ask| told(&mut found, ask));
            });
        match born {
            Ok(_) => Some(line),
            Err(e) => {
                tracing::error!(error = %e, "could not start the thread that tells the player");
                None
            }
        }
    })
    .as_ref()
}

/// The teller's loop: the asks in the order they came, each `tell`ed and its
/// `done` handed the answer — but one a later ask [`supersedes`] is dropped
/// unsaid. Ends when every door's line is gone.
fn tell_in_order(asks: &std::sync::mpsc::Receiver<Telling>, mut tell: impl FnMut(&Ask) -> Told) {
    let mut waiting: std::collections::VecDeque<Telling> = std::collections::VecDeque::new();
    loop {
        if waiting.is_empty() {
            match asks.recv() {
                Ok(t) => waiting.push_back(t),
                Err(_) => return,
            }
        }
        waiting.extend(asks.try_iter());
        let Some(first) = waiting.pop_front() else {
            continue;
        };
        if waiting
            .front()
            .is_some_and(|next| supersedes(&next.ask, &first.ask))
        {
            continue;
        }
        let answer = tell(&first.ask);
        (first.done)(answer);
    }
}

/// Whether `later`, told right after `earlier`, makes it pointless: both set
/// the same thing to a value of their own — the position (a move BY some
/// seconds is no value, and is never dropped), the player's volume, one
/// lane's, one ambient sound's.
fn supersedes(later: &Ask, earlier: &Ask) -> bool {
    match (earlier, later) {
        (
            Ask::Seek { .. },
            Ask::Seek {
                relative: false, ..
            },
        )
        | (Ask::Volume { .. }, Ask::Volume { .. }) => true,
        (
            Ask::Lane {
                lane: a,
                change: LaneChange::Volume { .. },
            },
            Ask::Lane {
                lane: b,
                change: LaneChange::Volume { .. },
            },
        ) => a == b,
        (Ask::AmbientVolume { id: a, .. }, Ask::AmbientVolume { id: b, .. }) => a == b,
        _ => false,
    }
}

/// How long a teller waits for the player's word: an ask it answers under
/// its lock is answered at once, and three seconds is a player that does not
/// answer; what it WALKS OR WRITES first — a folder or a playlist put, a set
/// or a queue saved — takes as long as the disk, a network folder included.
fn patience(ask: &Ask) -> std::time::Duration {
    match ask {
        Ask::Put { .. } | Ask::SaveSet { .. } | Ask::SaveQueue { .. } => {
            std::time::Duration::from_secs(60)
        }
        _ => std::time::Duration::from_secs(3),
    }
}

/// Tells the player one thing and waits for its word — a teller's, on its
/// own thread. `found`: the player's socket once somebody answered there —
/// the teller finds the player ONCE, and only when a connection fails is it
/// looked for, and started, again.
fn told(found: &mut Option<std::path::PathBuf>, ask: &Ask) -> Told {
    use ronoctua_player::client;
    use ronoctua_player::wire::Say;
    use std::io::ErrorKind;
    let io_kind = |e: &anyhow::Error, kinds: &[ErrorKind]| {
        e.chain().any(|c| {
            c.downcast_ref::<std::io::Error>()
                .is_some_and(|io| kinds.contains(&io.kind()))
        })
    };
    let timed_out = |e: &anyhow::Error| io_kind(e, &[ErrorKind::WouldBlock, ErrorKind::TimedOut]);
    // NOBODY AT THE SOCKET — the only failure worth a second try: the ask
    // never left, and telling it again cannot tell it twice.
    let nobody =
        |e: &anyhow::Error| io_kind(e, &[ErrorKind::ConnectionRefused, ErrorKind::NotFound]);
    // THE PLACE IT ANSWERED LAST, and then — nobody there any more — the
    // player looked for, and started, again.
    let mut tries = found.take().into_iter().map(Ok).collect::<Vec<_>>();
    if tries.is_empty() {
        tries.push(client::ensure());
    }
    let mut again = true;
    while let Some(path) = tries.pop() {
        let path = match path {
            Ok(p) => p,
            Err(e) if e.is::<client::NotInstalled>() => return Told::NotInstalled,
            Err(_) => return Told::NotRunning,
        };
        match client::ask_at(&path, ask, patience(ask)) {
            Ok(Some(Say::Farewell)) => return Told::NotRunning,
            Ok(said) => {
                *found = Some(path);
                return match said {
                    Some(Say::Refused(t)) => Told::Refused(t),
                    Some(Say::Draft { path, .. }) => Told::Draft(path.into()),
                    Some(_) => Told::Taken,
                    None => Told::Unheard,
                };
            }
            Err(e) if timed_out(&e) => {
                *found = Some(path);
                return Told::Working;
            }
            Err(e) if again && nobody(&e) => {
                again = false;
                tries.push(client::ensure());
            }
            Err(_) => return Told::NotRunning,
        }
    }
    Told::NotRunning
}

/// Starts the player again, off the loop: the one that runs is older than
/// this door (`client::restart`: MPRIS `Quit`, then this build's). What
/// went wrong is said down `told` as the player not running — which, then,
/// it is not; every door's watch finds the new one by itself.
pub fn restart(told: Option<ToldSender>) {
    crate::daemon::about_to_act("the player's daemon, started again");
    let born = std::thread::Builder::new()
        .name("player-restart".to_owned())
        .spawn(move || {
            if let Err(e) = ronoctua_player::client::restart() {
                tracing::warn!(error = %e, "the player could not be started again");
                if let Some(told) = told {
                    let _ = told.send(Told::NotRunning);
                }
            }
        });
    if let Err(e) = born {
        tracing::error!(error = %e, "could not start the thread that restarts the player");
    }
}

/// Tells the player one thing ONLY IF IT RUNS — never started to be told:
/// closing it everywhere must not bring it up to close it — and does not
/// wait for its word (`client::tell_if_running`): the two things told this
/// way, closing and rereading the preferences, are never refused, and a
/// loop that waited would freeze for as long as a busy player took. What
/// could not be written is in the log; nothing is said, because nothing
/// was refused.
pub fn tell_if_running(ask: &Ask) {
    crate::daemon::about_to_act("the player's daemon, told if it runs");
    if let Err(e) = ronoctua_player::client::tell_if_running(ask) {
        tracing::warn!(error = %e, "the player answered and could not be told");
    }
}

/// The player's doors that are PROCESSES — the column, a stage's window and
/// every other window of it — ended, whether or not a daemon was there to
/// say farewell: closing the player everywhere with nothing running still
/// closes them. Never this process: a door ends itself.
pub fn end_the_doors() {
    crate::daemon::about_to_act("the player's column and windows, ended");
    crate::right_column::end(crate::right_column::Tenant::Player);
    crate::player_stage::end();
    crate::player_stage::end_every_window();
}

/// A DOOR'S HOLD ON THE SOUND OUTPUTS (`Ask::Sinks`): while it lives, the
/// player reads the outputs a lane can be sent to and says them in its state
/// (`State::sinks`); dropped, the thread that holds the connection lets go and
/// the player stops reading — a door holds it only while a row of the outputs
/// is on screen (`player_lane::sinks_in_sight`).
///
/// OFF THE LOOP: connecting is a thread's, which then reads the connection
/// until EITHER END lets go — the door dropping the hold shuts it down, and a
/// player that went away closes it. A hold whose connection ended says so
/// ([`SinksHeld::ended`]): a player started again — after a crash, or asked
/// to — never heard the old one, and the door holds again.
#[derive(Debug)]
pub struct SinksHeld {
    /// The connection, once the thread made it; and whether the hold was
    /// let go, or ended.
    shared: std::sync::Arc<std::sync::Mutex<Hold>>,
}

/// What a hold's door and its thread share.
#[derive(Debug, Default)]
struct Hold {
    /// The connection, for the door to shut down when it lets go.
    stream: Option<std::os::unix::net::UnixStream>,
    /// The door let go.
    released: bool,
    /// The connection ended — or was never made: no player ran.
    ended: bool,
}

impl SinksHeld {
    /// Whether the hold's connection ENDED — or was never made — while the
    /// door still holds it: the player it was made with is gone, and one
    /// that runs now reads no outputs for this door until it holds again.
    #[must_use]
    pub fn ended(&self) -> bool {
        self.shared.lock().map_or(true, |h| h.ended)
    }
}

/// The door lets go: the connection is shut down, and the thread reading it
/// ends — the player stops reading the outputs when the last hold goes.
impl Drop for SinksHeld {
    fn drop(&mut self) {
        if let Ok(mut h) = self.shared.lock() {
            h.released = true;
            if let Some(s) = h.stream.take() {
                let _ = s.shutdown(std::net::Shutdown::Both);
            }
        }
    }
}

/// Holds the sound outputs until the answer is dropped ([`SinksHeld`]). A
/// player that is not running is not started for it: the rows say the default
/// output alone, the hold says it [`ended`](SinksHeld::ended), and the door
/// holds again once a player speaks.
#[must_use]
pub fn hold_sinks() -> SinksHeld {
    crate::daemon::about_to_act("the player's sound outputs, held");
    let shared = std::sync::Arc::new(std::sync::Mutex::new(Hold::default()));
    let theirs = shared.clone();
    let born = std::thread::Builder::new()
        .name("player-sinks".to_owned())
        .spawn(move || {
            let reader = match ronoctua_player::client::hold_sinks()
                .and_then(|s| Ok((s.try_clone()?, s)))
            {
                Ok((reader, stream)) => {
                    let Ok(mut h) = theirs.lock() else {
                        return;
                    };
                    if h.released {
                        return;
                    }
                    h.stream = Some(stream);
                    reader
                }
                Err(e) => {
                    tracing::debug!(error = %e, "no outputs to hold: the player is not running");
                    if let Ok(mut h) = theirs.lock() {
                        h.ended = true;
                    }
                    return;
                }
            };
            // UNTIL EITHER END LETS GO: the player sends nothing on it, so a
            // read returns only at its end — the door's shutdown, or the
            // player's.
            let mut reader = reader;
            let mut buf = [0_u8; 64];
            while matches!(std::io::Read::read(&mut reader, &mut buf), Ok(n) if n > 0) {}
            if let Ok(mut h) = theirs.lock() {
                h.ended = true;
                h.stream = None;
            }
        });
    if let Err(e) = born {
        tracing::error!(error = %e, "could not start the thread that holds the outputs");
        if let Ok(mut h) = shared.lock() {
            h.ended = true;
        }
    }
    SinksHeld { shared }
}

impl Saved {
    /// The extension of its file.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Set => ronoctua_player::own::SET_EXTENSION,
            Self::Queue => ronoctua_player::own::QUEUE_EXTENSION,
        }
    }
}

/// The name typed for a file of `saved`, as the stem of its file: trimmed,
/// and without the extension when it was typed too. `None` for nothing typed.
fn file_stem_of(said: &str, saved: Saved) -> Option<String> {
    let said = said.trim();
    let dot = format!(".{}", saved.extension());
    let stem = match said.len().checked_sub(dot.len()) {
        Some(at) if said.is_char_boundary(at) && said[at..].eq_ignore_ascii_case(&dot) => {
            said[..at].trim_end()
        }
        _ => said,
    };
    (!stem.is_empty()).then(|| stem.to_owned())
}

/// A folder's name, or the whole path for the root.
fn file_name(p: &std::path::Path) -> String {
    p.file_name().map_or_else(
        || p.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// A path as a person reads it: under home, from `~`.
fn shown_path(p: &std::path::Path) -> String {
    if let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from)
        && let Ok(rest) = p.strip_prefix(&home)
    {
        return if rest.as_os_str().is_empty() {
            "~".to_owned()
        } else {
            format!("~/{}", rest.display())
        };
    }
    p.to_string_lossy().into_owned()
}

/// Which press a command id is — the one map every door reaches.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "player-up" => Press::Up,
        "player-down" => Press::Down,
        "player-less" => Press::Less,
        "player-more" => Press::More,
        // AN ENTRY OF THE QUEUE, one place earlier or later.
        "player-earlier" => Press::Earlier,
        "player-later" => Press::Later,
        "player-enter" => Press::Enter,
        "player-toggle" => Press::Toggle,
        "player-stop" => Press::Stop,
        "player-next" => Press::Next,
        "player-previous" => Press::Previous,
        "player-remove" => Press::Remove,
        "player-browse" => Press::Browse,
        "player-machines" => Press::Go(PageId::Machines),
        "player-add" => Press::Add,
        "player-parent" => Press::Parent,
        "player-play-now" => Press::PlayNow,
        "player-clear" => Press::Clear,
        "player-quieter" => Press::Quieter,
        "player-louder" => Press::Louder,
        "player-only" => Press::Only,
        "player-maximize" => Press::Maximize,
        "player-pip" => Press::Pip,
        "player-mute" => Press::Mute,
        "player-swap" => Press::Swap,
        "player-tracks" => Press::Tracks,
        "player-fold" => Press::Fold,
        // THE SAME PRESS AS ADDING A FILE: what `a` does depends on the row
        // it is pressed on — a file goes in the queue, a medium of the file
        // plays too (`Target::Also`).
        "player-also" => Press::Add,
        "player-rename" => Press::Rename,
        "player-save-set" => Press::SaveSet,
        "player-save-queue" => Press::SaveQueue,
        "player-subtitles" => Press::Go(PageId::Tracks(TrackKind::Subtitles)),
        "player-ambience" => Press::Go(PageId::Ambience),
        "player-timeline" => Press::Go(PageId::Timeline),
        "player-layout" => Press::Go(PageId::Layout),
        "player-every-screen" => Press::EveryScreen,
        "player-metadata" => Press::Metadata,
        "player-suggestions" => Press::Go(PageId::Suggestions),
        "player-find-subtitles" => Press::Go(PageId::FindSubtitles),
        "player-find-lyrics" => Press::Go(PageId::FindLyrics),
        "player-internet" => Press::Go(PageId::Internet),
        "player-lane-shown" => Press::Shown,
        // THE DIRECTORIES — `LEADER p r`, `p v`, `p g` — and their keys.
        "player-radio" => Press::Go(PageId::Radio),
        "player-tv" => Press::Go(PageId::Tv),
        "player-channels" => Press::Go(PageId::Channels),
        "player-search" => Press::Search,
        "player-favourite" => Press::Favourite,
        "player-nearer" => Press::Globe(crate::player_globe::Turn::Nearer),
        "player-further" => Press::Globe(crate::player_globe::Turn::Further),
        "player-world" => Press::Globe(crate::player_globe::Turn::Whole),
        _ => return None,
    })
}

/// A key, as the view's press — the ONE reader every door calls: the window
/// and the column with `door::keys::key_of`, the footer with the key its own
/// reader built.
///
/// `Esc` is every level's way back, and the arrows, `RET`, `SPC` and `DEL`
/// are read by their symbol whatever is held, as both doors always read them.
/// A LETTER is read by its whole notation: `C-s` is not `s`. The footer used
/// to drop every modifier and answer a letter held with `Ctrl` as the bare
/// letter, while the window read xkb's control character and answered
/// nothing — one key, two answers.
///
/// A KEY THE TABLE NAMES WITH ITS MODIFIERS — `C-<up>`, which moves an entry
/// of the queue — is read by its whole notation first; an arrow held with a
/// modifier the table does not name is still the arrow.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    use ronoctua_keymap::{Key, Sym};
    match key.sym {
        Sym::Esc => Some(Press::Escape),
        Sym::Char(_) => pressed(&key.notation()),
        other => pressed(&key.notation()).or_else(|| pressed(&Key::simple(other).notation())),
    }
}

/// Which press a notation is, through [`OF_THE_VIEW`].
#[must_use]
pub fn pressed(notation: &str) -> Option<Press> {
    OF_THE_VIEW
        .iter()
        .find(|(k, _, _)| *k == notation)
        .and_then(|(_, id, _)| press_of(id))
}

/// States and outlines as the player sends them — what the doors' tests
/// stand on: the state with its lanes and sources, and the queue's outline
/// apart, as protocol 2 says them.
#[cfg(test)]
pub(crate) mod fixture {
    use ronoctua_player::own::{Arrangement, Form, Lane, LaneId, LaneKind, Screens};
    use ronoctua_player::queue::{EntryId, Name, Node, NodeKind, Outline};
    use ronoctua_player::wire::{LaneState, LaneStatus, PROTOCOL, Source, State, Track, TrackKind};

    /// A track of `kind`, chosen or not.
    #[must_use]
    pub fn track(id: i64, kind: TrackKind, title: Option<&str>, selected: bool) -> Track {
        Track {
            id,
            kind,
            title: title.map(ToOwned::to_owned),
            lang: None,
            codec: None,
            selected,
            albumart: false,
            default: false,
            forced: false,
            external: false,
        }
    }

    /// A file that plays ALONE, as the player says it: entry 1 current, its
    /// source's tracks, and its own lanes — its picture (1) when it has one
    /// that is no cover, its sound (2), its subtitles (3, on only while one
    /// is chosen) — each playing the track the source chose.
    #[must_use]
    pub fn alone(location: &str, tracks: Vec<Track>) -> State {
        let mut lanes = Vec::new();
        let mut lane_states = Vec::new();
        for (id, kind, lane_kind) in [
            (1, TrackKind::Video, LaneKind::Video),
            (2, TrackKind::Audio, LaneKind::Audio),
            (3, TrackKind::Sub, LaneKind::Subtitle),
        ] {
            let of_kind: Vec<&Track> = tracks
                .iter()
                .filter(|t| t.kind == kind && !t.albumart)
                .collect();
            let Some(first) = of_kind.first() else {
                continue;
            };
            let chosen = of_kind.iter().find(|t| t.selected).map(|t| t.id);
            let mut lane = Lane::new(LaneId(id), lane_kind, location);
            lane.enabled = kind != TrackKind::Sub || chosen.is_some();
            lane.track = Some(chosen.unwrap_or(first.id));
            if lane.enabled {
                lane_states.push(LaneState {
                    lane: lane.id,
                    duration_seconds: None,
                    status: LaneStatus::Playing,
                    picture: kind == TrackKind::Video,
                    drift_ms: 0,
                    master: kind == TrackKind::Audio,
                    size: (kind == TrackKind::Video).then_some((1920, 1080)),
                    cover: false,
                });
            }
            lanes.push(lane);
        }
        State {
            protocol: PROTOCOL,
            revision: 1,
            current: Some(EntryId(1)),
            location: Some(location.to_owned()),
            playing: true,
            volume: 100,
            speed: 1.0,
            lanes,
            lane_states,
            sources: vec![Source {
                location: location.to_owned(),
                tracks,
            }],
            ..State::default()
        }
    }

    /// A SET that plays, as the daemon says one: `lanes`, each as long as
    /// `lengths` says (by the lane's place; `None` unknown, a stream), the
    /// set `span` seconds long and its clock at `at`, entry 1 current — and
    /// the queue's outline, whose one entry is that set.
    #[must_use]
    pub fn set(
        lanes: Vec<Lane>,
        lengths: &[Option<f64>],
        span: Option<f64>,
        at: f64,
    ) -> (State, Box<Outline>) {
        let lane_states = lanes
            .iter()
            .zip(lengths.iter().chain(std::iter::repeat(&None)))
            .filter(|(lane, _)| lane.enabled)
            .map(|(lane, length)| LaneState {
                lane: lane.id,
                duration_seconds: *length,
                status: LaneStatus::Playing,
                picture: lane.kind == LaneKind::Video,
                drift_ms: 0,
                master: lane.kind == LaneKind::Audio,
                size: (lane.kind == LaneKind::Video).then_some((1920, 1080)),
                cover: false,
            })
            .collect();
        let count = u16::try_from(lanes.len()).unwrap_or(u16::MAX);
        let mut sources: Vec<String> = Vec::new();
        for lane in &lanes {
            if !sources.contains(&lane.source) {
                sources.push(lane.source.clone());
            }
        }
        let state = State {
            duration: span,
            position: at,
            lanes,
            lane_states,
            arrangement: Some(ronoctua_player::own::Arrangement::default()),
            ..idle()
        };
        let state = State {
            current: Some(EntryId(1)),
            playing: true,
            ..state
        };
        let outline = Box::new(Outline {
            revision: 1,
            entries: vec![Node {
                id: EntryId(1),
                name: Name::Together(sources),
                kind: NodeKind::Set,
                children: Vec::new(),
                lanes: count,
            }],
        });
        (state, outline)
    }

    /// Two films and a song with its cover: the first film on every screen,
    /// the second only on `DP-2`, the cover on `eDP-1`.
    #[must_use]
    pub fn routed() -> State {
        let mut one = Lane::new(LaneId(1), LaneKind::Video, "/films/one.mkv");
        one.title = Some("One".into());
        let mut two = Lane::new(LaneId(2), LaneKind::Video, "/films/two.mkv");
        two.screens = Screens::Only(vec!["DP-2".into()]);
        let mut song = Lane::new(LaneId(3), LaneKind::Audio, "/music/song.flac");
        song.screens = Screens::Only(vec!["eDP-1".into()]);
        let words = Lane::new(LaneId(4), LaneKind::Subtitle, "/films/one.srt");
        let (mut s, _) = set(
            vec![one, two, song, words],
            &[Some(60.0), Some(60.0), Some(60.0), Some(60.0)],
            Some(60.0),
            0.0,
        );
        for st in &mut s.lane_states {
            match st.lane.0 {
                1 | 2 => {
                    st.picture = true;
                    st.size = Some((1920, 1080));
                }
                3 => {
                    st.picture = true;
                    st.cover = true;
                    st.size = Some((500, 500));
                }
                _ => {}
            }
        }
        s.arrangement = Some(Arrangement {
            form: Form::SideBySide,
            order: Vec::new(),
            hidden: Vec::new(),
            covers: vec![LaneId(3)],
        });
        s
    }

    /// A player that answers with nothing loaded — of THIS protocol: a
    /// `State::default()` says protocol 0, an older player.
    #[must_use]
    pub fn idle() -> State {
        State {
            protocol: PROTOCOL,
            volume: 100,
            speed: 1.0,
            ..State::default()
        }
    }

    /// An outline of these entries, flat, their ids 1, 2, 3… in order.
    #[must_use]
    pub fn outline(names: Vec<Name>) -> Box<Outline> {
        Box::new(Outline {
            revision: 1,
            entries: names
                .into_iter()
                .zip(1..)
                .map(|(name, id)| Node {
                    id: EntryId(id),
                    name,
                    kind: NodeKind::Single,
                    children: Vec::new(),
                    lanes: 0,
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player_rows::{Slid, slid, slider_at};
    use fixture::{alone, idle, outline, track};
    use ronoctua_player::wire::TrackKind as Kind;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    /// FAILS IF: the swap's glyph stops sitting on the "Now" title in the
    /// footer — where there is no transport to carry it — drawn as the
    /// window it opens, or starts sitting there in a door whose transport
    /// already carries it; or a click on it, or `w`, stops being the swap.
    #[test]
    fn the_swap_sits_on_the_now_title_in_the_footer_only() {
        let t = texts();
        let state = playing();
        let now = t.t(ronoctua_i18n::key::PLAYER_HEADING_NOW);
        let title = |v: &View| {
            v.rows(Some(&state), 80, &t)
                .into_iter()
                .enumerate()
                .find(|(_, r)| r.fields.first().is_some_and(|f| f.text == now))
                .expect("the Now title")
        };
        let mut footer = View::blank(None).in_the_footer();
        let (n, row) = title(&footer);
        let cell = row.cells.first().expect("a control on the title");
        assert_eq!(cell.target, Target::Swap);
        assert!(
            row.fields
                .iter()
                .any(|f| f.at == cell.from && f.text == tokens::icon::PLAYER_WINDOW.to_string()),
            "the glyph is not the window's"
        );
        let rows = footer.rows(Some(&state), 80, &t);
        assert!(footer.clicked_at(n, cell.from, &rows), "a click on it acts");
        assert_eq!(footer.key(Press::Enter, Some(&state), 80, &t), Errand::Swap);
        assert_eq!(footer.key(Press::Swap, Some(&state), 80, &t), Errand::Swap);
        assert_eq!(pressed("w"), Some(Press::Swap));
        // A DOOR WITH A TRANSPORT: the title is only a title.
        let window = View::blank(None);
        assert!(title(&window).1.cells.is_empty());
        // AND WITH NO PLAYER RUNNING the footer's title carries it too.
        let (_, idle) = View::blank(None)
            .in_the_footer()
            .rows(None, 80, &t)
            .into_iter()
            .enumerate()
            .find(|(_, r)| r.fields.first().is_some_and(|f| f.text == now))
            .expect("the Now title");
        assert_eq!(idle.cells.first().map(|c| &c.target), Some(&Target::Swap));
    }

    /// A song playing alone, 1:15 into an hour and two minutes, with two
    /// subtitles — the Portuguese on — and the queue of two it is the first of.
    fn playing() -> State {
        let mut en = track(1, Kind::Sub, None, false);
        en.lang = Some("en".into());
        let mut pt = track(2, Kind::Sub, None, true);
        pt.lang = Some("pt".into());
        State {
            position: 75.0,
            duration: Some(3725.0),
            volume: 80,
            tailnet: Some("here".into()),
            ..alone(
                "/music/one.flac",
                vec![track(1, Kind::Audio, None, true), en, pt],
            )
        }
    }

    /// The queue `playing` is the first of: the file by its name, and a song
    /// with a title.
    fn the_queue() -> Box<ronoctua_player::queue::Outline> {
        outline(vec![
            Name::Location("/music/one.flac".into()),
            Name::Titled("Two".into()),
        ])
    }

    /// A view that knows the queue.
    fn queued(v: View) -> View {
        let mut v = v;
        v.outline_arrived(Some(the_queue()));
        v
    }

    #[test]
    fn a_clock_has_hours_only_from_an_hour_on() {
        assert_eq!(clock(75.4), "1:15");
        assert_eq!(clock(3725.0), "1:02:05");
        assert_eq!(clock(-3.0), "0:00");
    }

    #[test]
    fn the_cursor_is_born_on_a_row_that_acts() {
        let t = texts();
        let s = playing();
        let mut v = View::blank(None);
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(rows[v.cursor()].target.acts(), "{:?}", rows[v.cursor()]);
    }

    /// FAILS IF: an entry of the queue stops being its row BY ITS ID — the
    /// queue drawn from the outline the door keeps — or a click stops playing
    /// it, `d` taking it off, or the arrows moving it among its siblings.
    #[test]
    fn every_entry_of_the_queue_is_its_id_and_a_click_plays_it() {
        let t = texts();
        let s = playing();
        let mut v = queued(View::blank(None));
        let rows = v.rows(Some(&s), 80, &t);
        let second = rows
            .iter()
            .position(|r| r.target == Target::Entry(EntryId(2)))
            .expect("the second entry is a row");
        assert!(
            rows[second].fields.iter().any(|f| f.text == "Two"),
            "{:?}",
            rows[second]
        );
        assert!(
            rows.iter()
                .any(|r| r.fields.iter().any(|f| f.text == "one.flac")),
            "a file with no title is its file's name"
        );
        assert!(!v.clicked(second, &rows), "the first click selects");
        assert!(v.clicked(second, &rows), "the second acts");
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Jump { id: EntryId(2) })
        );
        assert_eq!(
            v.key(Press::Remove, Some(&s), 80, &t),
            Errand::Tell(Ask::Remove { id: EntryId(2) })
        );
        // `C-↑` MOVES IT FIRST; `C-↓` goes nowhere past the last.
        assert_eq!(
            v.key(Press::Earlier, Some(&s), 80, &t),
            Errand::Tell(Ask::Move {
                id: EntryId(2),
                after: None,
                into: None
            })
        );
        assert_eq!(v.key(Press::Later, Some(&s), 80, &t), Errand::Nothing);
        // WITH NO OUTLINE HEARD the queue says it is empty, and asks nothing.
        let bare = View::blank(None);
        assert!(
            !bare
                .rows(Some(&s), 80, &t)
                .iter()
                .any(|r| matches!(r.target, Target::Entry(_)))
        );
    }

    /// FAILS IF: an entry moves to anywhere but beside its siblings — back,
    /// after the one before the one before it, or first in its own playlist
    /// — or past either end.
    #[test]
    fn an_entry_moves_among_its_siblings() {
        use ronoctua_player::queue::{Node, NodeKind, Outline};
        let node = |id: u64, children: Vec<Node>| Node {
            id: EntryId(id),
            name: Name::Titled(id.to_string()),
            kind: if children.is_empty() {
                NodeKind::Single
            } else {
                NodeKind::Playlist
            },
            children,
            lanes: 0,
        };
        let o = Outline {
            revision: 1,
            entries: vec![
                node(1, Vec::new()),
                node(
                    2,
                    vec![
                        node(3, Vec::new()),
                        node(4, Vec::new()),
                        node(5, Vec::new()),
                    ],
                ),
            ],
        };
        let to = |id: u64, forward: bool| moved(&o, EntryId(id), forward);
        let ask = |id: u64, after: Option<u64>, into: Option<u64>| {
            Some(Ask::Move {
                id: EntryId(id),
                after: after.map(EntryId),
                into: into.map(EntryId),
            })
        };
        assert_eq!(to(3, true), ask(3, Some(4), None));
        assert_eq!(to(4, false), ask(4, None, Some(2)), "first in its playlist");
        assert_eq!(to(5, false), ask(5, Some(3), None));
        assert_eq!(to(2, false), ask(2, None, None), "first in the queue");
        assert_eq!(to(5, true), None, "past the last");
        assert_eq!(to(1, false), None, "before the first");
        assert_eq!(to(9, true), None, "an entry the outline does not hold");
    }

    /// FAILS IF: the home's arrows on the subtitles stop turning through
    /// "none" — the lane switched off — or ask anything but the lane of the
    /// subtitles for a track; or a sound with no choice turns at all.
    #[test]
    fn the_subtitle_ring_goes_through_none() {
        let s = playing();
        let lane = LaneId(3);
        assert_eq!(
            next_track(&s, TrackKind::Subtitles, true, true),
            Some(Ask::Lane {
                lane,
                change: LaneChange::Enabled { on: false }
            })
        );
        assert_eq!(
            next_track(&s, TrackKind::Subtitles, false, true),
            Some(Ask::Track {
                lane,
                track: Some(1)
            })
        );
        assert_eq!(
            next_track(&s, TrackKind::Sound, true, false),
            None,
            "no choice, no ring"
        );
    }

    #[test]
    fn the_volume_stops_at_both_ends() {
        let mut s = playing();
        s.volume = VOLUME_MOST;
        assert_eq!(
            turn(&Target::Volume, Press::More, Some(&s), None),
            Errand::Tell(Ask::Volume {
                percent: VOLUME_MOST
            })
        );
        s.volume = 2;
        assert_eq!(
            turn(&Target::Volume, Press::Less, Some(&s), None),
            Errand::Tell(Ask::Volume { percent: 0 })
        );
    }

    /// A view with the files open on `at`, the cursor where it lands.
    fn among(at: &std::path::Path) -> View {
        let mut v = View::blank(None);
        v.files_at(at, Purpose::Queue);
        v.settle_in(None, 80, &texts());
        v
    }

    /// A library of its own per test: the tests run at once, and one
    /// removing a shared folder pulls it from under another.
    fn a_library(test: &str) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("ronoctua-library-{}-{test}", std::process::id()));
        for name in [
            "one.flac",
            "Album/01.ogg",
            "Album/02.ogg",
            "Album/cover.jpg",
        ] {
            let p = root.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"").unwrap();
        }
        root
    }

    fn on(v: &View) -> Target {
        v.rows(None, 80, &texts())[v.cursor()].target.clone()
    }

    /// FAILS IF: the files stop being walked the way a person walks them —
    /// into a folder, back up onto it, a file added or played alone, and a
    /// folder queued whole.
    #[test]
    fn the_files_are_walked_played_and_queued() {
        let t = texts();
        let root = a_library("walked");
        let album = root.join("Album");
        let song = album.join("01.ogg").to_string_lossy().into_owned();
        let mut v = among(&root);
        // THE CURSOR LANDS ON THE FIRST ENTRY, the folder — not on `..`.
        assert_eq!(on(&v), Target::Folder(album.clone()));
        // `a` QUEUES IT WHOLE — ONE location, which the player makes a
        // playlist named after it — and says so, by its name.
        let Errand::TellSaying(ask, said) = v.key(Press::Add, None, 80, &t) else {
            panic!("a folder was not queued");
        };
        assert_eq!(ask, put(album.to_string_lossy().into_owned(), false));
        assert!(said.contains("Album"), "{said}");
        // `RET` ENTERS IT, and the cursor is on its first file.
        assert_eq!(v.key(Press::Enter, None, 80, &t), Errand::Nothing);
        assert_eq!(on(&v), Target::File(album.join("01.ogg")));
        // `RET` ON A FILE ADDS IT at the end; `P` PLAYS IT NOW, right after
        // what plays — the queue kept.
        let Errand::TellSaying(ask, _) = v.key(Press::Enter, None, 80, &t) else {
            panic!("RET did not add");
        };
        assert_eq!(ask, put(song.clone(), false));
        let Errand::TellSaying(ask, _) = v.key(Press::PlayNow, None, 80, &t) else {
            panic!("P did not play");
        };
        assert_eq!(
            ask,
            Ask::Put {
                what: vec![Thing::Location(song)],
                after: None,
                into: None,
                play: true,
                replace: false,
            },
            "playing now must never start the queue over"
        );
        // `DEL` GOES UP, and lands on the folder it came from.
        v.key(Press::Parent, None, 80, &t);
        assert_eq!(on(&v), Target::Folder(album));
        // `Esc` GOES BACK ONE LEVEL, and only the second one closes.
        assert_eq!(v.key(Press::Escape, None, 80, &t), Errand::Nothing);
        assert!(!v.browsing());
        assert_eq!(v.key(Press::Escape, None, 80, &t), Errand::Close);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: the entries stop being laid out in columns where there is
    /// room, or `..` stops being the first of them, or `→` stops crossing.
    #[test]
    fn the_entries_are_a_grid_with_the_way_back_first() {
        let t = texts();
        let root = a_library("grid");
        let mut v = among(&root);
        let rows = v.rows(None, 200, &t);
        let first = rows
            .iter()
            .find(|r| !r.cells.is_empty() && !r.heading)
            .unwrap();
        assert_eq!(first.cells[0].target, Target::Up);
        assert!(first.cells.len() > 1, "a wide door gave one column");
        // NARROW, one column.
        let rows = v.rows(None, 30, &t);
        assert!(rows.iter().all(|r| r.cells.len() <= 1 || r.heading));
        // `→` MOVES ACROSS on a wide door.
        v.settle_in(None, 200, &t);
        let before = v.rows(None, 200, &t)[v.cursor()].target.clone();
        v.key(Press::More, None, 200, &t);
        let after = v.rows(None, 200, &t)[v.cursor()].target.clone();
        assert_ne!(before, after);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: clearing the queue stops asking first, or clears an empty
    /// queue with a question about nothing.
    #[test]
    fn clearing_the_queue_asks_first() {
        let t = texts();
        let mut v = queued(View::blank(None));
        let state = playing();
        assert_eq!(v.key(Press::Clear, Some(&state), 80, &t), Errand::Nothing);
        assert!(v.asking.is_some());
        v.key(Press::Down, Some(&state), 80, &t);
        assert!(matches!(
            v.key(Press::Enter, Some(&state), 80, &t),
            Errand::TellSaying(Ask::Clear, _)
        ));
        let mut v = View::blank(None);
        assert!(matches!(
            v.key(Press::Clear, Some(&idle()), 80, &t),
            Errand::Said(_)
        ));
    }

    /// FAILS IF: the files open with the cursor on their heading — where the
    /// hint line teaches none of the level's keys, as the photograph of the
    /// first form showed — or `Esc` from them lands anywhere but on the row
    /// that opened them.
    #[test]
    fn the_files_open_with_the_cursor_on_an_entry() {
        let t = texts();
        let root = a_library("open");
        let mut v = View::blank(None);
        v.left_at = Some((0, root.clone()));
        v.key(Press::Browse, None, 80, &t);
        assert_eq!(on(&v), Target::Folder(root.join("Album")));
        // AND `Esc` LANDS BACK ON THE ROW THAT OPENED THEM.
        v.key(Press::Escape, None, 80, &t);
        assert_eq!(on(&v), Target::Go(PageId::Files(Purpose::Queue)));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: a folder with nothing to play is handed to the player as an
    /// empty order instead of said.
    #[test]
    fn a_folder_with_nothing_to_play_says_so() {
        let t = texts();
        let root = std::env::temp_dir().join(format!("ronoctua-silent-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("notes.txt"), b"").unwrap();
        let mut v = among(&root);
        let rows = v.rows(None, 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::PlayFolder)
            .unwrap();
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, None, 80, &t),
            Errand::Said(t.t(ronoctua_i18n::key::PLAYER_BROWSE_NOTHING_TO_PLAY))
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn with_no_player_the_rows_still_open_something() {
        let t = texts();
        let mut v = View::blank(None);
        v.settle_in(None, 80, &t);
        let rows = v.rows(None, 80, &t);
        // THE FILES ARE THE FIRST THING OFFERED: with nothing loaded, finding
        // something to play is what the view is opened for.
        assert_eq!(
            rows[v.cursor()].target,
            Target::Go(PageId::Files(Purpose::Queue))
        );
    }

    /// FAILS IF: `Esc` stops going back ONE page with the cursor on the row
    /// that opened it — found by its identity, so the rows that moved under
    /// the page meanwhile do not move it — or closes the door from a page,
    /// or goes back while a question is open instead of answering it "no".
    #[test]
    fn esc_goes_back_one_page_to_the_row_that_opened_it() {
        let t = texts();
        let mut s = playing();
        s.sources[0].tracks.retain(|t| t.kind != Kind::Sub);
        s.lanes
            .retain(|l| l.kind != ronoctua_player::own::LaneKind::Subtitle);
        let mut v = queued(View::blank(None));
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let machines = rows
            .iter()
            .position(|r| r.target == Target::Go(PageId::Machines))
            .expect("a row for the machines");
        v.clicked(machines, &rows);
        assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), Errand::Nothing);
        assert_eq!(v.page(), PageId::Machines);
        // A QUESTION ON THE PAGE: `Esc` is its no, and the page stays.
        let rows = v.rows(Some(&s), 80, &t);
        let pair = rows
            .iter()
            .position(|r| r.target == Target::Pair)
            .expect("the pairing row");
        v.clicked(pair, &rows);
        v.key(Press::Clear, Some(&s), 80, &t);
        assert!(v.asking.is_some());
        assert_eq!(v.key(Press::Escape, Some(&s), 80, &t), Errand::Nothing);
        assert!(v.asking.is_none());
        assert_eq!(v.page(), PageId::Machines);
        // THE ROWS UNDER THE PAGE MOVE: the file now offers a choice, and
        // the home grows a section above the row that opened the page.
        let moved = playing();
        let before = v.levels[0].cursor;
        assert_eq!(v.key(Press::Escape, Some(&moved), 80, &t), Errand::Nothing);
        assert_eq!(v.page(), PageId::Home);
        let rows = v.rows(Some(&moved), 80, &t);
        assert_eq!(
            rows[v.cursor()].target,
            Target::Go(PageId::Machines),
            "the cursor is not on the row that opened the page"
        );
        assert_ne!(v.cursor(), before, "the rows did not move: a weak test");
        // FROM THE HOME, `Esc` closes the door.
        assert_eq!(v.key(Press::Escape, Some(&moved), 80, &t), Errand::Close);
    }

    /// FAILS IF: a page exists that no row opens — a page reachable only by
    /// a command is one the pointer never reaches (invariant 19) — or a row
    /// that opens a page is labelled by another key than the one the page
    /// is titled by.
    #[test]
    fn every_page_is_reached_by_a_row() {
        let t = texts();
        // A FILE WITH A CHOICE OF EVERY KIND: two sounds, two pictures, and
        // the subtitles `playing` has.
        let mut tracks: Vec<Track> = playing().sources[0]
            .tracks
            .iter()
            .filter(|t| t.kind == Kind::Sub)
            .cloned()
            .collect();
        for (id, kind) in [
            (1, Kind::Audio),
            (2, Kind::Audio),
            (1, Kind::Video),
            (2, Kind::Video),
        ] {
            let mut t = track(id, kind, None, id == 1);
            t.codec = Some(format!("{kind:?}-{id}"));
            tracks.push(t);
        }
        // AND THE AMBIENT SOUNDS OF THE MACHINE, which the credits are of.
        let s = State {
            tailnet: Some("here".into()),
            ambience: crate::player_ambience::tests::rain().ambience,
            ..alone("/v/film.mkv", tracks)
        };
        // NO FOLDER OF THIS MACHINE'S is read for the pages of the files.
        let nowhere = std::env::temp_dir().join(format!("ronoctua-no-home-{}", std::process::id()));
        let mut reached = vec![PageId::Home];
        let mut queue = vec![PageId::Home];
        while let Some(page) = queue.pop() {
            let mut v = View::blank(Some(nowhere.clone()));
            // AND WHAT THE FILE SAYS OF ITSELF, as a door's thread leaves it:
            // the Metadata's pages are reached from its rows.
            v.meta.given(
                "/v/film.mkv",
                0,
                crate::player_metadata::Got::Sheet(Box::new(
                    crate::player_metadata::tests::song_sheet("/v/film.mkv"),
                )),
            );
            // AND WHAT THE INTERNET SUGGESTED FOR IT: a suggestion's page is
            // reached from its row.
            v.found
                .given(crate::player_suggest::tests::desk_for("/v/film.mkv", None));
            // AND THE WORLD'S RADIO AND TV, their countries listed: a
            // country's page is reached from its row.
            v.found.given(crate::player_globe::tests::countries_desk());
            v.found
                .given(crate::player_globe::tests::tv_countries_desk());
            if page != PageId::Home {
                v.push(page, None);
            }
            for r in v.rows(Some(&s), 80, &t) {
                // A LANE'S PAGE, by `RET` on the lane's own row — whose label
                // is the lane's name, not the page's.
                let to = match r.target {
                    Target::Go(to) => {
                        // THE ROW SAYS THE PAGE'S OWN KEY.
                        assert_eq!(
                            r.fields.first().map(|f| f.text.clone()),
                            Some(t.t(to.key())),
                            "{to:?}"
                        );
                        to
                    }
                    Target::Subtitle(id) | Target::Lane(id) => PageId::Lane(id),
                    // A SUGGESTION'S PAGE, by `RET` on its row — whose label
                    // is the suggestion's name.
                    Target::Candidate(id) => PageId::Candidate(id),
                    // A LIST OF A DIRECTORY'S, by `RET` on its row — whose
                    // label is what it lists: a country, a genre.
                    Target::Opens(ronoctua_player::find::Query::Tv(_)) => PageId::TvChannels,
                    Target::Opens(_) => PageId::Stations,
                    // A SEVERAL-LINE FIELD'S PAGE, by `RET` on the field's
                    // row — whose label is the field's, as the page's title.
                    Target::Tag(field) | Target::TagFixed(field)
                        if field.lines() == ronoctua_tags::Lines::Many =>
                    {
                        PageId::Field(field)
                    }
                    _ => continue,
                };
                if !reached.contains(&to) {
                    reached.push(to);
                    queue.push(to);
                }
            }
        }
        for page in PageId::PAGES {
            assert!(reached.contains(&page), "{page:?} is reached by no row");
        }
        // AND EACH PAGE IS TITLED BY THE SAME KEY.
        for page in PageId::PAGES {
            let mut v = View::blank(Some(nowhere.clone()));
            v.push(page, None);
            assert_eq!(
                v.rows(Some(&s), 80, &t)
                    .first()
                    .and_then(|r| r.fields.first())
                    .map(|f| f.text.clone()),
                Some(t.t(page.key())),
                "{page:?}"
            );
        }
    }

    /// FAILS IF: `LEADER p m`'s press stops opening the machines page, or
    /// opens a second one over the first.
    #[test]
    fn a_command_opens_its_page_once() {
        let t = texts();
        let s = playing();
        let mut v = View::blank(None);
        assert_eq!(
            press_of("player-machines"),
            Some(Press::Go(PageId::Machines))
        );
        v.key(Press::Go(PageId::Machines), Some(&s), 80, &t);
        v.key(Press::Go(PageId::Machines), Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Machines);
        assert_eq!(v.levels.len(), 2);
        assert_eq!(v.key(Press::Escape, Some(&s), 80, &t), Errand::Nothing);
        assert_eq!(v.page(), PageId::Home);
    }

    /// FAILS IF: a page already in the stack is stacked again — `LEADER p m`
    /// and `k` pressed in turn piled the same two pages up without end — or a
    /// page opened BY NAME comes back, on `Esc`, to wherever the cursor
    /// happened to be instead of the page's own row.
    #[test]
    fn a_page_is_stacked_once_and_a_named_one_comes_back_to_its_row() {
        let t = texts();
        let s = playing();
        let mut v = View::blank(None);
        v.settle_in(Some(&s), 80, &t);
        let first = v.rows(Some(&s), 80, &t)[v.cursor()].spot.clone();
        assert_ne!(first, PageId::Machines.spot());
        for _ in 0..3 {
            v.key(Press::Go(PageId::Machines), Some(&s), 80, &t);
            v.key(Press::Tracks, Some(&s), 80, &t);
        }
        v.key(Press::Go(PageId::Machines), Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Machines);
        assert_eq!(v.levels.len(), 2, "pages piled up");
        assert_eq!(v.key(Press::Escape, Some(&s), 80, &t), Errand::Nothing);
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(rows[v.cursor()].spot, PageId::Machines.spot());
        // `k` IS PRESSED WHERE THE PERSON STANDS, and comes back there.
        let at = rows
            .iter()
            .position(|r| r.spot == "volume")
            .expect("a volume row");
        v.clicked(at, &rows);
        v.key(Press::Tracks, Some(&s), 80, &t);
        v.key(Press::Escape, Some(&s), 80, &t);
        assert_eq!(v.rows(Some(&s), 80, &t)[v.cursor()].spot, "volume");
    }

    /// FAILS IF: a door that draws no rows — the window's stage, the
    /// footer's picture alone, the column's bar — lets a key act on the row
    /// under a cursor nobody sees, or opens a page behind the picture; or a
    /// key of everywhere stops acting there.
    #[test]
    fn a_key_that_needs_the_rows_is_classed_for_a_door_without_them() {
        for (notation, id, at) in OF_THE_VIEW {
            let press = pressed(notation).expect(notation);
            let everywhere = *at == Where::Anywhere && !matches!(*id, "player-up" | "player-down");
            assert_eq!(press.unseen() == Unseen::Acts, everywhere, "{notation}");
        }
        for press in [
            Press::Tracks,
            Press::Browse,
            Press::Clear,
            Press::Go(PageId::Machines),
        ] {
            assert_eq!(press.unseen(), Unseen::OpensThere, "{press:?}");
        }
        for press in [Press::Enter, Press::Remove, Press::Less, Press::Down] {
            assert_eq!(press.unseen(), Unseen::ShowsTheRows, "{press:?}");
        }
        assert_eq!(Press::Escape.unseen(), Unseen::Acts);
    }

    /// FAILS IF: a key the service took stops being said in the door that
    /// kept it, naming the service.
    #[test]
    fn a_key_taken_is_said_naming_its_service() {
        let t = texts();
        assert_eq!(
            told_sentence(&Told::KeyTaken(ronoctua_lookup::Service::Tmdb), false, &t),
            Some(t.t_with(
                ronoctua_i18n::key::PLAYER_KEY_ACCEPTED,
                &[("service", "TMDB")]
            ))
        );
    }

    /// FAILS IF: a label of a page is cut instead of wrapped in a narrow
    /// door, the values of a section leave their one column, the cursor stops
    /// on the rest of a wrapped label, or a click there is not a click on
    /// its row.
    #[test]
    fn a_long_label_wraps_under_its_row_and_the_values_keep_one_column() {
        let t = ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR");
        let s = playing();
        let mut v = View::blank(None);
        v.key(Press::Go(PageId::Internet), Some(&s), 80, &t);
        let narrow = tokens::space::COLUMN_SMALL;
        let rows = v.rows(Some(&s), narrow, &t);
        let pairs: Vec<&Row> = rows
            .iter()
            .filter(|r| {
                matches!(
                    r.target,
                    Target::LyricsOnline | Target::Sponsors | Target::Skip(_)
                )
            })
            .collect();
        assert!(
            pairs.iter().any(|r| r.continues),
            "nothing wrapped at {narrow}"
        );
        let columns: std::collections::BTreeSet<u16> = pairs
            .iter()
            .filter(|r| !r.continues)
            .filter_map(|r| r.fields.get(1).map(|f| f.at))
            .collect();
        assert_eq!(columns.len(), 1, "values in {columns:?}");
        for r in &rows {
            for f in &r.fields {
                assert!(!f.text.contains('…'), "cut: {:?}", f.text);
            }
        }
        // THE HOST IS IN THE LABEL a person taps to consent.
        let lyrics: String = rows
            .iter()
            .filter(|r| r.target == Target::LyricsOnline)
            .filter_map(|r| r.fields.first().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            lyrics.contains(ronoctua_player::prefs::LRCLIB_HOST),
            "{lyrics}"
        );
        // WHERE THE ROW HOLDS BOTH, nothing wraps: the section's values move
        // past its widest label instead.
        // (The switches asked when a person presses carry their hosts on a
        // line of their own by design — `Builder::pair_under` — and are not
        // what this asks.)
        let wide = v.rows(Some(&s), 100, &t);
        assert!(
            !wide.iter().any(|r| r.continues
                && matches!(
                    r.target,
                    Target::LyricsOnline | Target::Sponsors | Target::Skip(_)
                )),
            "wrapped at 100 columns"
        );
        let column = wide
            .iter()
            .find(|r| r.target == Target::Sponsors)
            .and_then(|r| r.fields.get(1))
            .map(|f| f.at);
        let (left, room) = crate::power::extent(100);
        assert!(
            column > Some(crate::chrome::value_column(left, room)),
            "the values stayed at the column every view has: {column:?}"
        );
        // THE CURSOR NEVER STOPS ON THE REST OF A LABEL.
        for _ in 0..rows.len() {
            v.key(Press::Down, Some(&s), narrow, &t);
            assert!(
                rows[v.cursor()].stops(),
                "the cursor on line {}",
                v.cursor()
            );
        }
        // A CLICK THERE IS A CLICK ON ITS ROW.
        let rest = rows
            .iter()
            .position(|r| r.continues)
            .expect("a wrapped line");
        v.clicked(rest, &rows);
        assert_eq!(v.cursor(), Row::owner(&rows, rest));
        assert!(v.cursor() < rest);
    }

    /// FAILS IF: the position's time is cut in a narrow door — `12:…` reads
    /// as a wrong time — instead of the bar before it giving way.
    #[test]
    fn the_position_s_time_is_read_whole_and_the_bar_gives_way() {
        let t = ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR");
        let s = playing();
        let v = View::blank(None);
        for columns in [
            tokens::space::COLUMN_SMALL,
            tokens::space::COLUMN_MEDIUM,
            80,
        ] {
            let rows = v.rows(Some(&s), columns, &t);
            let position = rows
                .iter()
                .find(|r| r.spot == "position")
                .expect("a position row");
            let value = &position.fields[1].text;
            assert!(value.ends_with("1:15 / 1:02:05"), "{columns}: {value:?}");
        }
    }

    /// FAILS IF: two tracks of one kind are drawn under the same name — three
    /// English sounds read `en`, `en`, `en` — or a track with a name of its
    /// own is given more than it needs.
    #[test]
    fn two_tracks_never_read_the_same() {
        let english = |id: i64, codec: &str| {
            let mut t = track(id, Kind::Audio, None, false);
            t.lang = Some("en".into());
            t.codec = Some(codec.into());
            t
        };
        let three = [english(1, "dts"), english(2, "ac3"), english(3, "ac3")];
        let kin: Vec<&Track> = three.iter().collect();
        let names: Vec<String> = three.iter().map(|x| track_name(x, &kin)).collect();
        let unique: std::collections::BTreeSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), 3, "{names:?}");
        assert_eq!(names[0], "en · dts");
        let one = english(1, "dts");
        assert_eq!(track_name(&one, &[&one]), "en");
    }

    /// FAILS IF: something typed on the page that picks what to play
    /// together plays and leaves that page open, where a pick closes it and
    /// goes back to the page that asked.
    #[test]
    fn what_is_typed_to_play_together_closes_the_picker_as_a_pick_does() {
        let t = texts();
        let s = playing();
        let mut v = View::blank(None);
        v.key(Press::Go(PageId::Together), Some(&s), 80, &t);
        v.files_at(std::path::Path::new("/nowhere"), Purpose::Lane);
        assert_eq!(v.page(), PageId::Files(Purpose::Lane));
        let mut pairing = crate::player_machines::Pairing::default();
        let errand = crate::player_question::answered_saying(
            crate::player_question::TOGETHER,
            "/v/b.mkv",
            &mut pairing,
            &t,
        );
        v.answer_came(&errand, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Together);
        // NEVER MIND keeps the files open.
        v.files_at(std::path::Path::new("/nowhere"), Purpose::Lane);
        v.answer_came(&Errand::Nothing, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Files(Purpose::Lane));
    }

    /// FAILS IF: the typed path or address row stops being a question of the
    /// door's own line — for the purpose the files are open for — or closes
    /// the files under it, or an answer stops becoming the same order in
    /// whichever door typed it.
    #[test]
    fn a_typed_path_is_asked_with_the_files_still_open() {
        use crate::player_machines::Pairing;
        use crate::player_question::answered;
        let t = texts();
        let root = a_library("typed");
        let mut v = among(&root);
        let rows = v.rows(None, 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Typed)
            .expect("the typed row");
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, None, 80, &t),
            Errand::Ask(Question::plain(ENQUEUE, false))
        );
        assert!(v.browsing(), "the files closed under the question");
        let mut pairing = Pairing::default();
        assert_eq!(
            answered(ENQUEUE, "https://example.org/a", &mut pairing),
            Errand::Tell(put("https://example.org/a".into(), false))
        );
        assert_eq!(answered(ENQUEUE, "   ", &mut pairing), Errand::Nothing);
        assert_eq!(
            answered(TOGETHER, "/v/b.mkv", &mut pairing),
            Errand::Tell(crate::player_question::together("/v/b.mkv".into()))
        );
        // TOGETHER, the same row asks the other question.
        v.back(None, 80, &t);
        v.files_at(&root, Purpose::Lane);
        v.settle_in(None, 80, &t);
        let rows = v.rows(None, 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Typed)
            .expect("the typed row");
        v.clicked(at, &rows);
        assert!(matches!(
            v.key(Press::Enter, None, 80, &t),
            Errand::Ask(Question { id: TOGETHER, .. })
        ));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: the position and the volume stop being bars a press sets —
    /// the bar found where the value was drawn, the press read along it, and
    /// the order the same arithmetic as the window's time bar and track.
    #[test]
    fn the_position_and_the_volume_are_bars_a_press_sets() {
        let t = texts();
        let s = playing();
        let v = View::blank(None);
        let rows = v.rows(Some(&s), 80, &t);
        let position = rows
            .iter()
            .position(|r| r.target == Target::Position)
            .expect("the position row");
        let bar = rows[position].slider.expect("the position is a bar");
        assert_eq!(bar.of, Slid::Position);
        assert_eq!(
            Some(bar.from),
            rows[position].fields.get(1).map(|f| f.at),
            "the bar is not where the value was drawn"
        );
        let middle = f64::from(bar.from) + f64::from(bar.cells) / 2.0;
        let found = slider_at(&rows, position, middle).expect("a press on the bar");
        assert_eq!(
            slid(found.of, found.along(middle), Some(&s)),
            Errand::Tell(Ask::Seek {
                seconds: 3725.0 / 2.0,
                relative: false
            })
        );
        // OFF THE BAR, it is the row's click, and not the bar's.
        assert_eq!(slider_at(&rows, position, f64::from(bar.from) - 1.0), None);
        // A DRAG PAST ITS END keeps the end.
        assert_eq!(bar.along(f64::from(bar.from + bar.cells) + 40.0), 1.0);
        let volume = rows
            .iter()
            .position(|r| r.target == Target::Volume)
            .expect("the volume row");
        let bar = rows[volume].slider.expect("the volume is a bar");
        assert_eq!(
            slid(bar.of, 1.0, Some(&s)),
            Errand::Tell(Ask::Volume {
                percent: VOLUME_MOST
            })
        );
        // A STREAM HAS NO LENGTH, and its position is no bar.
        let mut live = playing();
        live.duration = None;
        let rows = v.rows(Some(&live), 80, &t);
        assert!(
            rows.iter()
                .find(|r| r.target == Target::Position)
                .is_some_and(|r| r.slider.is_none())
        );
        assert_eq!(slid(Slid::Position, 0.5, Some(&live)), Errand::Nothing);
    }

    /// FAILS IF: the view stops being able to stop — the key the window's
    /// transport glyph runs — or a chord is read as the letter under it.
    #[test]
    fn stop_is_a_key_of_the_view_and_a_chord_is_not_its_letter() {
        use ronoctua_keymap::{Key, Mods, Sym};
        let t = texts();
        let s = playing();
        let mut v = View::blank(None);
        let stop = press_of_key(Key::simple(Sym::Char('s')));
        assert_eq!(stop, Some(Press::Stop));
        assert_eq!(
            v.key(Press::Stop, Some(&s), 80, &t),
            Errand::Tell(Ask::Stop)
        );
        let chord = Key {
            sym: Sym::Char('s'),
            mods: Mods {
                ctrl: true,
                ..Mods::default()
            },
        };
        assert_eq!(press_of_key(chord), None, "C-s was read as s");
        assert_eq!(press_of_key(Key::simple(Sym::Esc)), Some(Press::Escape));
    }

    #[test]
    fn every_key_of_the_table_is_a_press() {
        for (k, id, _) in OF_THE_VIEW {
            assert!(pressed(k).is_some(), "{k} → {id}");
        }
    }

    /// The row `target` is on — or the entry of a grid — clicked twice: what
    /// `RET` does next is its.
    fn chosen(v: &mut View, s: Option<&State>, target: &Target) {
        let t = texts();
        let rows = v.rows(s, 80, &t);
        let in_a_grid = rows.iter().enumerate().find_map(|(at, r)| {
            r.cells
                .iter()
                .find(|c| !c.control && &c.target == target)
                .map(|c| (at, c.from))
        });
        let (at, column) = in_a_grid.unwrap_or_else(|| {
            let at = rows
                .iter()
                .position(|r| &r.target == target)
                .unwrap_or_else(|| panic!("no row for {target:?}"));
            (at, 0)
        });
        v.clicked_at(at, column, &rows);
        v.clicked_at(at, column, &rows);
    }

    /// FAILS IF: renaming stops asking on the door's line HOLDING the name
    /// the entry has — the one under the cursor with `r`, what plays from its
    /// row — or an answer left as it was, or that nobody asked for, renames
    /// anything; or an answer EMPTIED stops giving the entry its own name
    /// back.
    #[test]
    fn renaming_asks_holding_the_name_it_has() {
        use crate::player_question::RENAME;
        let t = texts();
        let s = playing();
        let mut v = queued(View::blank(None));
        let mut pairing = crate::player_machines::Pairing::default();
        chosen(&mut v, Some(&s), &Target::Rename);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Ask(Question::holding(RENAME, "one.flac".into()))
        );
        assert_eq!(
            v.answered(RENAME, " one.flac ", &mut pairing, Some(&s), 80, &t),
            Errand::Nothing,
            "the name it had, kept"
        );
        v.key(Press::Enter, Some(&s), 80, &t);
        assert_eq!(
            v.answered(RENAME, "Morning", &mut pairing, Some(&s), 80, &t),
            Errand::TellSaying(
                Ask::Rename {
                    id: EntryId(1),
                    name: "Morning".into()
                },
                t.t_with(ronoctua_i18n::key::PLAYER_RENAMED, &[("name", "Morning")])
            )
        );
        // `r` ON AN ENTRY: that one.
        assert_eq!(pressed("r"), Some(Press::Rename));
        let rows = v.rows(Some(&s), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Entry(EntryId(2)))
            .expect("the second entry");
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Rename, Some(&s), 80, &t),
            Errand::Ask(Question::holding(RENAME, "Two".into()))
        );
        // EMPTIED: its own name back, which nothing else gives it.
        assert_eq!(
            v.answered(RENAME, "  ", &mut pairing, Some(&s), 80, &t),
            Errand::TellSaying(
                Ask::Rename {
                    id: EntryId(2),
                    name: String::new()
                },
                t.t_with(ronoctua_i18n::key::PLAYER_RENAMED_BACK, &[("was", "Two")])
            ),
        );
        assert_eq!(
            v.answered(RENAME, "Unasked", &mut pairing, Some(&s), 80, &t),
            Errand::Nothing,
            "an answer to a question the view did not ask"
        );
    }

    /// FAILS IF: saving what plays or the queue stops going through the
    /// door's line — the name first (holding what plays' name), then the
    /// folder (holding the music folder) — or writes over a file that is
    /// there without a question born on its "no"; or the file stops opening
    /// again from its page: a set plays after what plays, the queue kept, and
    /// a queue takes the queue's place only after a question of its own.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn saving_and_opening_go_through_the_ask() {
        use crate::player_question::{SAVE_QUEUE, SAVE_QUEUE_FOLDER, SAVE_SET, SAVE_SET_FOLDER};
        use ronoctua_i18n::key;
        let t = texts();
        let s = playing();
        let root = a_library("saved");
        let music = root.join("Music");
        std::fs::create_dir_all(&music).unwrap();
        let mut v = queued(View::blank(Some(root.clone())));
        let mut pairing = crate::player_machines::Pairing::default();
        let mut answer = |v: &mut View, id: &str, said: &str| {
            v.answered(id, said, &mut pairing, Some(&s), 80, &t)
        };
        // THE ROW: the name, holding what plays' own.
        chosen(&mut v, Some(&s), &Target::SaveSet);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Ask(Question::holding(SAVE_SET, "one.flac".into()))
        );
        // THE NAME, its extension typed too: the folder next, holding the
        // music folder.
        assert_eq!(
            answer(&mut v, SAVE_SET, " Film night.RSET "),
            Errand::Ask(Question::holding(SAVE_SET_FOLDER, shown_path(&music)))
        );
        let to = music.join("Film night.rset");
        let saving = Errand::TellSaying(
            Ask::SaveSet {
                to: to.to_string_lossy().into_owned(),
                name: Some("Film night".into()),
            },
            t.t_with(
                key::PLAYER_SAVING,
                &[("name", "Film night.rset"), ("folder", &shown_path(&music))],
            ),
        );
        assert_eq!(answer(&mut v, SAVE_SET_FOLDER, &shown_path(&music)), saving);
        // A FILE ALREADY THERE: a question, born on its "no" — and "yes"
        // writes over it.
        std::fs::write(&to, b"{}").unwrap();
        assert!(matches!(
            v.key(Press::SaveSet, Some(&s), 80, &t),
            Errand::Ask(Question { id: SAVE_SET, .. })
        ));
        answer(&mut v, SAVE_SET, "Film night");
        assert_eq!(
            answer(&mut v, SAVE_SET_FOLDER, &shown_path(&music)),
            Errand::Nothing
        );
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(
            rows[v.cursor()].target,
            Target::Answer(false),
            "born on its no"
        );
        v.key(Press::Down, Some(&s), 80, &t);
        assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), saving);
        // A NAME NO FILE CAN HAVE, and a folder that is not one: said.
        v.key(Press::SaveSet, Some(&s), 80, &t);
        assert_eq!(
            answer(&mut v, SAVE_SET, "a/b"),
            Errand::Said(t.t(key::PLAYER_SAVE_BAD_NAME))
        );
        v.key(Press::SaveSet, Some(&s), 80, &t);
        answer(&mut v, SAVE_SET, "Film night");
        let nowhere = root.join("nowhere");
        assert_eq!(
            answer(&mut v, SAVE_SET_FOLDER, &nowhere.to_string_lossy()),
            Errand::Said(t.t_with(
                key::PLAYER_SAVE_NO_FOLDER,
                &[("folder", &shown_path(&nowhere))]
            ))
        );
        // THE QUEUE: no name to start from.
        chosen(&mut v, Some(&s), &Target::SaveQueue);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Ask(Question::plain(SAVE_QUEUE, false))
        );
        answer(&mut v, SAVE_QUEUE, "Week");
        let week = music.join("Week.rqueue");
        assert!(matches!(
            answer(&mut v, SAVE_QUEUE_FOLDER, &music.to_string_lossy()),
            Errand::TellSaying(Ask::SaveQueue { ref to, .. }, _)
                if *to == week.to_string_lossy()
        ));
        std::fs::write(&week, b"{}").unwrap();
        // OPENED AGAIN, from the page that lists them: the set plays now,
        // after what plays, and the page closes.
        chosen(&mut v, Some(&s), &Target::Go(PageId::Files(Purpose::Open)));
        v.key(Press::Enter, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Files(Purpose::Open));
        let listed: Vec<Target> = v
            .rows(Some(&s), 80, &t)
            .iter()
            .flat_map(|r| r.cells.iter().map(|c| c.target.clone()))
            .filter(|t| matches!(t, Target::File(_)))
            .collect();
        assert_eq!(
            listed,
            [Target::File(to.clone()), Target::File(week.clone())],
            "the page lists the player's own files, and nothing else"
        );
        chosen(&mut v, Some(&s), &Target::File(to.clone()));
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::TellSaying(
                put(to.to_string_lossy().into_owned(), true),
                t.t_with(key::PLAYER_PLAYING_NOW, &[("name", "Film night.rset")])
            )
        );
        assert_eq!(v.page(), PageId::Home, "the page closes on the choice");
        // THE QUEUE: its own question first, born on its "no".
        v.key(Press::Go(PageId::Files(Purpose::Open)), Some(&s), 80, &t);
        chosen(&mut v, Some(&s), &Target::File(week.clone()));
        assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), Errand::Nothing);
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(rows[v.cursor()].target, Target::Answer(false));
        v.key(Press::Down, Some(&s), 80, &t);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::TellSaying(
                Ask::Put {
                    what: vec![Thing::Location(week.to_string_lossy().into_owned())],
                    after: None,
                    into: None,
                    play: false,
                    replace: true,
                },
                t.t_with(key::PLAYER_QUEUE_OPENED, &[("name", "Week.rqueue")])
            )
        );
        // TYPED, on the same page: the same answer.
        v.key(Press::Go(PageId::Files(Purpose::Open)), Some(&s), 80, &t);
        assert!(matches!(
            answer(&mut v, OPEN_SAVED, &to.to_string_lossy()),
            Errand::TellSaying(Ask::Put { play: true, .. }, _)
        ));
        assert_eq!(
            answer(&mut v, OPEN_SAVED, &root.join("one.flac").to_string_lossy()),
            Errand::Said(t.t_with(key::PLAYER_OPEN_NOT_OURS, &[("name", "one.flac")]))
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: the teller tells asks out of the order they came — a seek
    /// dragged along the bar would land an earlier place after the last —
    /// drops a setting that nothing set again after it, drops a move BY some
    /// seconds that another move adds to, or tells a volume or a position
    /// once per motion when a newer one is already waiting behind it (a
    /// place said outright makes every move before it pointless); or an ask the player
    /// dropped unheard is said as "not running" to a door whose player is an
    /// older one.
    #[test]
    fn asks_are_told_in_order_and_a_setting_told_again_once() {
        let (line, asks) = std::sync::mpsc::channel::<Telling>();
        let heard = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let seek = |seconds: f64, relative: bool| Ask::Seek { seconds, relative };
        let lane_volume = |lane: u32, percent: u8| Ask::Lane {
            lane: LaneId(lane),
            change: LaneChange::Volume { percent },
        };
        let sent = [
            seek(1.0, false),
            seek(2.0, false),
            seek(3.0, false),
            seek(5.0, true),
            seek(1.0, true),
            seek(6.0, false),
            Ask::Volume { percent: 40 },
            Ask::Volume { percent: 50 },
            lane_volume(1, 10),
            lane_volume(2, 20),
            lane_volume(2, 30),
            Ask::Next,
            Ask::Volume { percent: 60 },
        ];
        for (n, ask) in sent.iter().cloned().enumerate() {
            let heard = heard.clone();
            line.send(Telling {
                ask,
                done: Box::new(move |_| heard.lock().unwrap().push(n)),
            })
            .unwrap();
        }
        drop(line);
        let mut told = Vec::new();
        tell_in_order(&asks, |ask| {
            told.push(ask.clone());
            Told::Taken
        });
        assert_eq!(
            told,
            vec![
                seek(3.0, false),
                seek(5.0, true),
                seek(6.0, false),
                Ask::Volume { percent: 50 },
                lane_volume(1, 10),
                lane_volume(2, 30),
                Ask::Next,
                Ask::Volume { percent: 60 },
            ]
        );
        assert_eq!(*heard.lock().unwrap(), vec![2, 3, 5, 7, 8, 10, 11, 12]);
        // AN OLDER PLAYER drops what it cannot read: its door says that.
        let t = texts();
        assert_eq!(
            told_sentence(&Told::Unheard, true, &t),
            Some(t.t(ronoctua_i18n::key::PLAYER_OLD_DAEMON))
        );
        assert_eq!(
            told_sentence(&Told::Unheard, false, &t),
            Some(t.t(ronoctua_i18n::key::PLAYER_NOT_RUNNING))
        );
        assert_eq!(told_sentence(&Told::Working, false, &t), None);
    }

    /// FAILS IF: two sound outputs of one name read the same on a lane's
    /// page, one with no name reads blank, or one that went away is said by
    /// the sound server's name when this door knew the person's.
    #[test]
    fn a_sound_output_is_said_by_the_name_a_person_knows() {
        use ronoctua_player::wire::{Sink, SinkKind};
        let t = texts();
        let sink = |device: &str, description: &str| Sink {
            device: device.into(),
            description: description.into(),
            kind: SinkKind::Headphones,
        };
        let sinks = vec![
            sink("alsa_output.usb-a", "USB headset"),
            sink("alsa_output.usb-b", "USB headset"),
            sink("alsa_output.pci", ""),
            sink("bluez_output.kitchen", "Kitchen"),
        ];
        let first = sink_label("alsa_output.usb-a", &sinks, &t);
        let second = sink_label("alsa_output.usb-b", &sinks, &t);
        assert_ne!(first, second, "two outputs of one name read the same");
        assert!(first.contains("USB headset") && second.contains("USB headset"));
        assert_eq!(sink_label("alsa_output.pci", &sinks, &t), "alsa_output.pci");
        assert_eq!(sink_label("bluez_output.kitchen", &sinks, &t), "Kitchen");
        // GONE: said as it was seen.
        assert_eq!(
            sink_label("bluez_output.kitchen", &[], &t),
            "bluez_output.kitchen"
        );
        let _ = hearing(ronoctua_player::client::Heard::State(Box::new(State {
            sinks,
            ..State::default()
        })));
        assert_eq!(sink_label("bluez_output.kitchen", &[], &t), "Kitchen");
        assert_eq!(
            trouble_sentence(
                &Trouble::SinkGone {
                    device: "bluez_output.kitchen".into()
                },
                &t
            ),
            t.t_with(
                ronoctua_i18n::key::PLAYER_TROUBLE_SINK_GONE,
                &[("device", "Kitchen")]
            )
        );
    }
}
