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
//! the pages above it, the files (`LEADER p l`), playing together, the
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
//! # The files, through the one picker
//!
//! A PAGE of the view for each purpose (`PageId::Pick`): the system's file
//! picker (`crate::file_picker`) with the kinds of file the purpose takes
//! (`crate::player_kinds`) — into the queue, played together, a set or a
//! queue to open, a subtitle, a cover, where a set or the queue is saved. The
//! picker takes every key while its page is up and answers with a
//! [`crate::file_picker::Picked`], which the view carries out
//! ([`View::picked`]); `Esc` goes back to the page that opened it.
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

use crate::player_catalog::ListingKey;
use crate::player_page::{Level, Subject};
pub use crate::player_page::{PageId, TrackKind};
use crate::player_question::Question;
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
    // A ROW THAT PLAYS from a directory is added to the queue with `a`, and
    // played now with `P`. The files have their own keys, the picker's.
    ("a", "player-add", Where::OnAPlayable),
    ("P", "player-play-now", Where::OnAPlayable),
    ("c", "player-clear", Where::Opening),
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
    // THE SEEKS BESIDE THE CLOCK — `j` ten seconds back, `l` ten ahead —
    // while what plays has a position: the glyphs around the transport's
    // clock, pressed by a key (`player_media::clock_group`).
    ("j", "player-rewind", Where::WithAPosition),
    ("l", "player-forward", Where::WithAPosition),
    // `z` FOLDS THE PICTURE away, and back — the glyph on the "Now" title,
    // in the door that shows one ([`View::picture_foldable`]).
    ("z", "player-picture-fold", Where::WithAPictureShown),
    // `C-s` SAVES THE QUEUE AGAIN into the file it was opened from or saved
    // as, once it is no longer what that file says — the glyph on the
    // Queue's title.
    ("C-s", "player-save-queue-again", Where::WithAQueueFile),
    // `O` TURNS THE COUNTRIES' ORDER on a world page: the most first, or A
    // to Z by the name in the person's language — the glyph on their title.
    ("O", "player-countries-order", Where::OnTheCountries),
    // `T` TRANSLATES THE LYRICS under their verses, or stops — the glyph on
    // their title.
    ("T", "player-lyrics-translate", Where::WithLyrics),
    // A LISTING OF THE CATALOGUE (`crate::player_catalog`): its pages — the
    // one before, the one after, the first and the last — the whole screen,
    // the next layout, a letter typed and the next order; and `S` turns the
    // source on its page and on channels by theme.
    ("<prior>", "player-listing-earlier", Where::OnAListing),
    ("<next>", "player-listing-later", Where::OnAListing),
    ("<home>", "player-listing-first", Where::OnAListing),
    ("<end>", "player-listing-last", Where::OnAListing),
    ("F", "player-listing-full", Where::OnAListing),
    ("L", "player-listing-layout", Where::OnAListing),
    ("S", "player-listing-source", Where::OnTheCatalog),
    ("C-l", "player-listing-letter", Where::OnAListing),
    ("O", "player-listing-order", Where::OnAListing),
    // THE RANKING of the video catalogue (`crate::player_ranking`): its
    // pages, the other layout — and `e`, the `⋯` at the end of a video's row,
    // its own page.
    ("<prior>", "player-ranking-earlier", Where::OnTheRanking),
    ("<next>", "player-ranking-later", Where::OnTheRanking),
    ("<home>", "player-ranking-first", Where::OnTheRanking),
    ("<end>", "player-ranking-last", Where::OnTheRanking),
    ("L", "player-ranking-layout", Where::OnTheRanking),
    ("e", "player-ranking-more", Where::OnARanked),
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
    /// Anywhere, a key that OPENS what it shows — a question — which a door
    /// that draws no rows shows them for first (`Press::unseen`).
    Opening,
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
    /// On a row that plays: [`Where::OnAPick`]'s, or a website's address
    /// ([`crate::player_website`]) — what `a` queues and `P` plays now.
    OnAPlayable,
    /// On a page that draws a globe.
    OnTheGlobe,
    /// While what plays has a position to seek in.
    WithAPosition,
    /// While the door shows a picture it can fold away, or has folded one
    /// ([`View::picture_foldable`]).
    WithAPictureShown,
    /// While the queue is no longer what the file it came from says
    /// (`QueueKept::Changed`).
    WithAQueueFile,
    /// On a world page, whose countries have an order to turn.
    OnTheCountries,
    /// While what plays has lyrics.
    WithLyrics,
    /// On a listing of the catalogue.
    OnAListing,
    /// On channels by theme, or a listing of them: the catalogue's source
    /// turns there.
    OnTheCatalog,
    /// On the ranking of the video catalogue (`crate::player_ranking`).
    OnTheRanking,
    /// On a video of the ranking.
    OnARanked,
}

impl Where {
    /// Whether its keys act on WHAT PLAYS and not on the row under the
    /// cursor — so they act where no row is drawn (the window's stage, the
    /// footer's picture alone, the column's bar: [`Press::unseen`]): always,
    /// or while what plays has what they act on.
    #[cfg(test)]
    #[must_use]
    pub const fn of_no_row(self) -> bool {
        matches!(
            self,
            Self::Anywhere
                | Self::WithAPosition
                | Self::WithAPictureShown
                | Self::WithAQueueFile
                | Self::WithLyrics
        )
    }

    /// Whether it holds on the page on screen, WHEN THE PAGE ALONE SAYS —
    /// `None` when the row under the cursor, or what plays, decides. What
    /// tells two entries of [`OF_THE_VIEW`] that share a key apart
    /// ([`pressed_where`]): `O` sorts the countries on a world page and
    /// something else on another. A new `Where` is placed here by the
    /// compiler: the match names every one.
    fn on_the_page(self, view: &View) -> Option<bool> {
        match self {
            Self::OnTheCountries
            | Self::OnADirectory
            | Self::OnTheGlobe
            | Self::OnAListing
            | Self::OnTheCatalog
            | Self::OnTheRanking
            | Self::WithAPictureShown => Some(self.shown(&Target::Nothing, None, view)),
            Self::Anywhere
            | Self::OnAValue
            | Self::OnARow
            | Self::OnRemovable
            | Self::OnAnEntry
            | Self::OnAGroup
            | Self::OnATrack
            | Self::Opening
            | Self::OnAPlayable
            | Self::OnARanked
            | Self::WithAChoice
            | Self::WithAPicture
            | Self::OnALane
            | Self::WithLanes
            | Self::OnAPick
            | Self::WithAPosition
            | Self::WithAQueueFile
            | Self::WithLyrics => None,
        }
    }

    fn shown(self, target: &Target, state: Option<&State>, view: &View) -> bool {
        let page = view.page();
        match self {
            Self::Anywhere => true,
            Self::WithAPosition => state.is_some_and(loaded),
            Self::WithAPictureShown => view.picture_fold.is_some(),
            Self::WithAQueueFile => state.is_some_and(|s| {
                matches!(
                    s.queue_file,
                    ronoctua_player::queue::QueueKept::Changed { .. }
                )
            }),
            Self::OnTheCountries => matches!(page, PageId::Radio | PageId::Tv),
            Self::WithLyrics => state.is_some_and(|s| !s.lyrics.is_empty()),
            Self::OnADirectory => match page {
                PageId::Radio
                | PageId::Stations
                | PageId::RadioGenres
                | PageId::RadioLanguages
                | PageId::Tv
                | PageId::TvChannels => true,
                // THE CATALOGUE is searched from channels by theme and from a
                // listing (`crate::player_catalog`).
                PageId::Channels | PageId::Catalog => true,
                _ => false,
            },
            Self::OnAListing => page == PageId::Catalog,
            Self::OnTheCatalog => matches!(page, PageId::Channels | PageId::Catalog),
            Self::OnTheRanking => page == PageId::Ranking,
            Self::OnARanked => matches!(target, Target::Ranked(_)),
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
                    | Target::Website(_)
            ),
            Self::OnAnEntry => matches!(target, Target::Entry(_) | Target::Group(_)),
            Self::OnAGroup => matches!(target, Target::Group(_)),
            Self::OnATrack => matches!(target, Target::Track { id: Some(_), .. }),
            Self::Opening => true,
            // A ROW THAT PLAYS — from a directory, or a website's address —
            // is added and played as a file is.
            // AND A VIDEO OF THE RANKING (`crate::player_ranking`).
            Self::OnAPlayable => matches!(
                target,
                Target::Pick(..) | Target::Website(_) | Target::Ranked(_)
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
    /// Put the row that plays from a directory in the queue — or, on one of
    /// the file's media, play it too.
    Add,
    /// The row that plays from a directory, now: right after what plays, the
    /// queue kept.
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
    /// A key typed into the TEXT being edited ([`View::edit_text`]) — what
    /// every key is while one is open ([`View::press_of_key`]).
    Text(ronoctua_keymap::Key),
    /// A key the FILE PICKER on screen takes — every letter while its page is
    /// up ([`View::press_of_key`], `FilePicker::claims`).
    Pick(ronoctua_keymap::Key),
    /// Ten seconds back (`j`, the seek before the clock).
    Rewind,
    /// Ten seconds ahead (`l`, the seek after the clock).
    Forward,
    /// The picture folded away, or back (`z`) — the door's to do
    /// ([`Errand::FoldPicture`]).
    FoldPicture,
    /// The queue saved again into the file it came from (`C-s`).
    SaveQueueAgain,
    /// The countries of a world page by how many, or by name (`O`).
    CountriesOrder,
    /// The lyrics translated under their verses, or no longer (`T`).
    Translate,
    /// A key of a LISTING of the catalogue — a page, the whole screen, the
    /// layout, the source, a letter, the order (`crate::player_catalog`).
    Listing(crate::player_catalog::ListingKey),
    /// A key of the RANKING — a page, the layout, a video's own page
    /// (`crate::player_ranking`).
    Ranking(crate::player_ranking::RankingKey),
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
            | Self::Rewind
            | Self::Forward
            | Self::FoldPicture
            | Self::Translate
            | Self::Escape => Unseen::Acts,
            Self::Up
            | Self::Down
            | Self::Less
            | Self::More
            | Self::Enter
            | Self::Remove
            | Self::Add
            | Self::PlayNow
            | Self::Fold
            | Self::Earlier
            | Self::Later
            | Self::Shown
            | Self::Favourite
            | Self::Globe(_)
            | Self::CountriesOrder
            | Self::Text(_)
            | Self::Pick(_)
            | Self::Listing(_)
            | Self::Ranking(_) => Unseen::ShowsTheRows,
            Self::Browse
            | Self::Go(_)
            | Self::Tracks
            | Self::Clear
            | Self::Rename
            | Self::SaveSet
            | Self::SaveQueue
            // SAVING AGAIN may ask — a name for a queue never saved, or
            // whether to write over a file changed on the disk — and a
            // question is drawn on the rows: they come back first.
            | Self::SaveQueueAgain
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
    /// The Streams page: a stream's language, by ffprobe's index — `RET`
    /// asks its code typed (`Edit::Stream`).
    StreamLanguage(u32),
    /// A stream's own name — `RET` asks it typed, `d` takes it out.
    StreamTitle(u32),
    /// Whether a stream is marked the default — a switch.
    StreamDefault(u32),
    /// Whether a stream is marked forced — a switch.
    StreamForced(u32),
    /// The Chapters page: a chapter's title, the chapter by where it starts
    /// — `RET` asks it typed (`Edit::Chapter`).
    ChapterTitle(u64),
    /// A lane's page: which of its file's languages a subtitle lane shows —
    /// a ring of the ones the file carries (a SAMI's classes).
    LaneLanguage(LaneId),
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
    /// A row or a cell of the FILE PICKER on screen (`crate::file_picker`):
    /// a click chooses it there, and `RET` is the picker's.
    Picker(crate::file_picker::Target),
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
    /// Another page of a list of stations, by its number.
    StationsPage(u32),
    /// Something that plays from a directory — a station, a channel, a film,
    /// a live — by its kind and its id: `RET` plays it now, `a` adds it to the
    /// queue, `b` keeps it among the favourites.
    Pick(ronoctua_broadcast::picks::Kind, String),
    /// The star at the head of such a row: a click keeps it among the
    /// favourites, or takes it off.
    Star(ronoctua_broadcast::picks::Kind, String),
    /// The glyph on the "Now" title that folds the picture away, or shows it
    /// again, and `z`.
    FoldPicture,
    /// The glyph on the Queue's title that saves it again into the file it
    /// came from, and `C-s`.
    SaveQueueAgain,
    /// The glyph on a world page's "Countries" that turns their order, and
    /// `O`.
    CountriesOrder,
    /// The glyph on the Lyrics' title that translates them, or stops, and
    /// `T`.
    Translate,
    /// Which language the lyrics are translated into: a ring over the
    /// tongues (`ronoctua_player::tongues`).
    TranslateTo,
    /// The host the lyrics' translation was refused for, not being let:
    /// `RET` lets it, and asks the translation again.
    LetTranslator(String),
    /// A host the queue was refused for, not being let (`Trouble::Off` in
    /// the state's troubles): `RET` lets it.
    LetHost(String),
    /// The Internet page's switch of the lyrics' translator: whether its
    /// host is let (`ronoctua_consent`).
    Translator,
    /// The Internet page's ring of WHO translates the lyrics: MyMemory, DeepL
    /// or a LibreTranslate server (`crate::player_translator`).
    TranslatorWho,
    /// The LibreTranslate server's address — `RET` asks it typed.
    TranslatorAddress,
    /// A translation key of the person's — `RET` asks it masked, `d`
    /// forgets it after a question.
    WordsKey(ronoctua_words::keys::Key),
    /// A several-line field's page: `RET` opens its words here, over the
    /// page, to be changed in this door (`crate::text_area`).
    EditHere(ronoctua_tags::Field),
    /// The website page's two questions: play now (`true`), or add to the
    /// queue (`crate::player_website`).
    WebsiteAsk(bool),
    /// An address played lately: `RET` plays it now, `a` puts it in the
    /// queue, `d` forgets it.
    Website(String),
    /// CHANNELS BY THEME's source — a ring (`crate::player_catalog`).
    CatalogSource,
    /// A source read again (`Ask::Ingest`), by its id.
    CatalogRead(String),
    /// A default source's hosts let — the press is the consent — and read.
    CatalogLet(String),
    /// A listing opened: everything of the source, a theme, a category — in
    /// one of the two files.
    CatalogScope(
        ronoctua_broadcast::catalog::Store,
        ronoctua_broadcast::catalog::Scope,
    ),
    /// The catalogue's search, typed — `/` too.
    CatalogSearch,
    /// The adult catalogue, asked open: the person's password, masked.
    CatalogUnlock,
    /// The adult catalogue shut again.
    CatalogLock,
    /// An item of the catalogue, in its file, by its id: `RET` plays it now,
    /// `a` adds it to the queue.
    CatalogItem(ronoctua_broadcast::catalog::Store, i64),
    /// The listing's letter — a ring the arrows turn, `RET` types one.
    CatalogLetter(Option<char>),
    /// One letter's cell on that row: a click takes it.
    CatalogLetterCell(Option<char>),
    /// The listing's page: the arrows turn it, `RET` types one.
    CatalogPage,
    /// Whether the words search every category's.
    CatalogEveryCategory,
    /// Whether the listing takes every source's.
    CatalogEverySource,
    /// The listing's order — a ring.
    CatalogSort,
    /// How many rows its page holds — a ring.
    CatalogPerPage,
    /// Its country — a ring of the countries it has.
    CatalogCountry,
    /// Its language — a ring of the languages it has.
    CatalogLanguage,
    /// The least height of its pictures — a ring.
    CatalogQuality,
    /// How it is drawn — a ring, and the title's glyph.
    CatalogLayout,
    /// The listing alone on the whole screen — the title's glyph, and `F`.
    CatalogFull,
    /// The whole archive searched for the words, past the catalogue.
    CatalogSearchArchive,
    /// A source added: its link pasted (`crate::player_sources`).
    SourceAdd,
    /// A source added straight to the adult catalogue, while it is open.
    SourceAddAdult,
    /// A source's posters, fetched or not — a switch.
    SourcePosters(String),
    /// A person's source kept in the adult catalogue, or not — a switch.
    SourceAdult(String),
    /// A person's source forgotten, after a question.
    SourceRemove(String),
    /// A VIDEO OF THE RANKING, by its id: `RET` puts it at the queue's end,
    /// once, `P` plays it now (`crate::player_ranking`).
    Ranked(ronoctua_video_metrics::VideoId),
    /// The `⋯` at the end of its row: its own page.
    RankedMore(ronoctua_video_metrics::VideoId),
    /// How the ranking is drawn — a ring, and the title's glyph.
    RankingLayout,
    /// Its page — a ring.
    RankingPage,
    /// A row of a video's own page.
    RankedDo(crate::player_ranking::RankedAct),
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
                | Self::LaneLanguage(_)
                | Self::TranslatorWho
                | Self::StreamDefault(_)
                | Self::StreamForced(_)
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
                | Self::TranslateTo
                | Self::Translator
                // THE CATALOGUE'S RINGS (`crate::player_catalog`).
                | Self::CatalogSource
                | Self::CatalogLetter(_)
                | Self::CatalogPage
                | Self::CatalogEveryCategory
                | Self::CatalogEverySource
                | Self::CatalogSort
                | Self::CatalogPerPage
                | Self::CatalogCountry
                | Self::CatalogLanguage
                | Self::CatalogQuality
                | Self::CatalogLayout
                | Self::SourcePosters(_)
                | Self::SourceAdult(_)
                // THE RANKING'S TWO RINGS (`crate::player_ranking`).
                | Self::RankingLayout
                | Self::RankingPage
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

/// What a choice of the files is for — each a page of the file picker
/// (`PageId::Pick`) and the request it opens with ([`Purpose::request`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Purpose {
    /// Into the queue: `RET` adds, `C-RET` plays now; several marked go in
    /// together, and a folder whole is an extra of the page.
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
    /// Where what plays is saved as a set, and under what name.
    SaveSet,
    /// Where the queue is saved, and under what name.
    SaveQueue,
}

impl Purpose {
    /// The request its page opens the picker with, starting at `start` —
    /// the folder it was left in, else the user directory it is about — and
    /// a save's first name.
    #[must_use]
    pub fn request(
        self,
        start: Option<std::path::PathBuf>,
        name: String,
    ) -> crate::file_picker::Request {
        use crate::file_picker::{
            Alternate, ENQUEUE_FOLDER, Mode, PLAY_FOLDER, Request, Start, Xdg,
        };
        use crate::player_kinds as kinds;
        let (purpose, mode, filters, place) = match self {
            Self::Queue => (
                "player-queue",
                Mode::Open { many: true },
                kinds::FOR_THE_QUEUE,
                Xdg::MUSIC,
            ),
            Self::Lane => (
                "player-lane",
                Mode::Open { many: false },
                kinds::FOR_A_LANE,
                Xdg::VIDEOS,
            ),
            Self::Open => (
                "player-open",
                Mode::Open { many: false },
                kinds::FOR_OPENING,
                Xdg::MUSIC,
            ),
            Self::Subtitle => (
                "player-subtitle",
                Mode::Open { many: false },
                kinds::FOR_A_SUBTITLE,
                Xdg::VIDEOS,
            ),
            Self::Cover => (
                "player-cover",
                Mode::Open { many: false },
                kinds::FOR_A_COVER,
                Xdg::PICTURES,
            ),
            // ONE PURPOSE FOR BOTH SAVES: a set and a queue are kept in the
            // same folder, which is where the other is looked for.
            Self::SaveSet | Self::SaveQueue => (
                "player-save",
                Mode::Save {
                    name,
                    extension: Some(if self == Self::SaveSet {
                        Saved::Set.extension()
                    } else {
                        Saved::Queue.extension()
                    }),
                },
                kinds::FOR_SAVING,
                Xdg::MUSIC,
            ),
        };
        Request {
            purpose,
            mode,
            filters,
            start: start.map_or(Start::Place(place), Start::At),
            // AN ADDRESS PLAYS as a file does, in the queue and together.
            addresses: matches!(self, Self::Queue | Self::Lane),
            extras: if self == Self::Queue {
                &[PLAY_FOLDER, ENQUEUE_FOLDER]
            } else {
                &[]
            },
            alternate: (self == Self::Queue).then_some(Alternate::PlayNow),
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
    /// Who translates the lyrics — a LibreTranslate server's address kept
    /// with it (`crate::player_translator`).
    Translator(ronoctua_words::Provider),
}

impl Errand {
    /// Whether it writes `player.json` — a consent, a switch — alone or
    /// before or after something else.
    #[must_use]
    pub fn writes_prefs(&self) -> bool {
        match self {
            Self::Let { .. } | Self::Keep(_) | Self::LetHosts { .. } => true,
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
            Self::Translator(provider) => {
                if let ronoctua_words::Provider::LibreTranslate { url } = provider {
                    prefs.lyrics_translation.address = Some(url.clone());
                }
                prefs.lyrics_translation.by = provider.clone();
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
    /// THE CLIPBOARD'S TEXT, for the text being edited (`C-v`): the door's
    /// to do — it reads the clipboard off its loop and hands what was
    /// copied to [`View::text_pasted`].
    Paste,
    /// THE PICTURE FOLDED AWAY, or shown again (`z`, the glyph on "Now"):
    /// the door's to do — only a door has a picture, and each keeps its own
    /// fold, never written anywhere.
    FoldPicture,
    /// An address of the website page kept among the ones played lately,
    /// or forgotten — MERGED into its file as it is on disk
    /// (`player_website::keep`).
    Websites(crate::player_website::Change),
    /// LET these hosts, or no longer — in the person's consent file, merged
    /// by host (`ronoctua_consent::let_hosts`) — and then `then`: the
    /// lyrics' translator, from its switch and from the row under a refusal.
    LetHosts {
        /// The hosts, by name.
        hosts: Vec<String>,
        /// Let, or no longer.
        on: bool,
        /// What is done once it is written.
        then: Box<Errand>,
    },
    /// THE PERSON'S SYSTEM PASSWORD, checked for the adult catalogue — the
    /// door's to do, OFF ITS LOOP (PAM waits seconds on a wrong one): how it
    /// ended comes back as [`Told::Unlock`].
    Unlock(ronoctua_consent::keys::Secret),
    /// A TRANSLATION KEY of the person's, kept private from its first byte,
    /// off the loop (`ronoctua_words::keys`).
    KeepWordsKey {
        /// Which.
        key: ronoctua_words::keys::Key,
        /// What was typed.
        secret: ronoctua_consent::keys::Secret,
    },
    /// A translation key forgotten, off the loop.
    ForgetWordsKey(ronoctua_words::keys::Key),
    /// A VIDEO IN THE CHROME KIOSK — the door's to do: the kiosk is the
    /// footer's (`kiosk-open`), which a window and the column ask down its
    /// socket (`crate::player_ranking`).
    Kiosk(String),
    /// A video of the ranking hidden, deleted or downloaded — OFF THE LOOP,
    /// and said when it is done (`Told::Ranking`).
    Ranking(crate::player_ranking::Act),
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
    /// A REFUSAL THAT IS THE PERSON'S TO OVERRULE — already in the queue,
    /// the queue's file changed on the disk — handed to the view with the
    /// ask that says yes: the view asks, "no" first ([`View::told`]), and
    /// its "yes" sends `again`.
    Asks {
        /// The same ask, the refusal overruled: `once` off, `force` on.
        again: Box<Ask>,
        /// Why it was refused.
        trouble: Trouble,
    },
    /// How the person's system password was checked for the adult
    /// catalogue ([`Errand::Unlock`]).
    Unlock(Result<(), ronoctua_pam::Refusal>),
    /// What a video of the ranking hidden, deleted or downloaded came to
    /// ([`Errand::Ranking`]).
    Ranking(crate::player_ranking::Outcome),
}

impl Told {
    /// Whether it is a question for the view ([`Told::Asks`]).
    #[must_use]
    pub const fn asks(&self) -> bool {
        matches!(self, Self::Asks { .. })
    }
}

/// The ask that overrules `trouble` for `ask`, when it is one a person can
/// overrule: a `Put` refused for being queued already, sent again without
/// `once`; saving the queue again over a file that changed, with `force`.
#[must_use]
pub fn overruled(ask: &Ask, trouble: &Trouble) -> Option<Ask> {
    match (ask, trouble) {
        (Ask::Put { once: true, .. }, Trouble::AlreadyQueued { .. }) => {
            let mut again = ask.clone();
            if let Ask::Put { once, .. } = &mut again {
                *once = false;
            }
            Some(again)
        }
        (Ask::SaveQueueAgain { force: false }, Trouble::ChangedOnDisk { .. }) => {
            Some(Ask::SaveQueueAgain { force: true })
        }
        _ => None,
    }
}

/// What a door does with an answer of the thread once its view took its
/// share ([`View::told`]) — one decision, read by the window, the column and
/// the footer alike ([`heard`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    /// The view asks a question on its rows, "no" first: what the order said
    /// it did ("added…", "saved again") is taken off the door's line — it
    /// was not done — and a door that hid its rows (the window's stage, the
    /// footer's picture alone) brings them back, for the question is drawn
    /// there.
    Asked,
    /// Said on the door's line, or nothing.
    Said(Option<String>),
}

/// What a door does with `told`, `took` being whether its view took it
/// ([`Heard`]).
#[must_use]
pub fn heard(took: bool, told: &Told, older: bool, texts: &ronoctua_i18n::Texts) -> Heard {
    if took && told.asks() {
        Heard::Asked
    } else {
        Heard::Said(told_sentence(told, older, texts))
    }
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
        // A QUESTION THE VIEW ASKS; said as the refusal only where no view
        // took it ([`Told::asks`]).
        Told::Asks { trouble, .. } => Some(trouble_sentence(trouble, texts)),
        Told::KeyTrouble(t) => Some(crate::player_internet::key_trouble(*t, texts)),
        Told::Unlock(Ok(())) => Some(texts.t(key::PLAYER_ADULT_OPEN)),
        Told::Unlock(Err(r)) => Some(texts.t(crate::player_catalog::unlock_refused_key(*r))),
        Told::Ranking(outcome) => Some(crate::player_ranking::said(outcome, texts)),
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
    /// A LISTING of the catalogue alone on the whole screen — its rows and
    /// nothing else (`crate::player_catalog`): the window's stage, opened by
    /// the footer and the column already that way.
    Listing,
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
    /// The yes-or-no question on screen, when one is.
    asking: Option<Choice>,
    /// A control on a title's line, clicked: what the next `RET` does.
    clicked_control: Option<Target>,
    /// The home the picker's places are in, instead of this machine's own
    /// folders — a photograph's ([`View::blank`]).
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
    /// The entry a person last moved on the home (`C-↑`/`C-↓`, the arrows at
    /// the end of its row): the home's queue windows keep it in sight beside
    /// what plays, so the next move takes the same entry and the cursor is
    /// never left on another by its place (`player_queue::home_window`). Let
    /// go when what plays moves on.
    pub(crate) home_held: Option<EntryId>,
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
    /// A TEXT OF SEVERAL LINES being edited over the page, in either door
    /// ([`View::edit_text`]): every key is its (`Press::Text`), and the page
    /// draws it until it is left (`crate::text_area`).
    pub(crate) editing: Option<crate::text_area::TextArea>,
    /// THE DOOR'S PICTURE, as the door says it ([`View::picture_foldable`]):
    /// `None` while it shows none it could fold away — the "Now" title
    /// carries no fold glyph then — else whether it is folded.
    pub(crate) picture_fold: Option<bool>,
    /// Whether the person let the lyrics' translator's host
    /// (`ronoctua_consent`): read with the preferences, and turned by the
    /// Internet page's switch.
    pub(crate) translator_let: bool,
    /// THE TRANSLATION KEYS KEPT, by whether their files are there — read
    /// with the preferences (`crate::player_translator`).
    pub(crate) words_kept: Vec<ronoctua_words::keys::Key>,
    /// THE LANGUAGE THE LYRICS WERE LAST TRANSLATED INTO, as the state said
    /// it: while translation is off the state says none, and turning it on
    /// again sends this one — the person's choice, not the interface's
    /// (`View::translate`).
    pub(crate) lyrics_tongue: Option<String>,
    /// The addresses of the website page played lately
    /// (`crate::player_website`).
    pub(crate) websites: crate::player_website::Websites,
    /// THE LOCAL CATALOGUE: channels by theme, the listing, the sources, the
    /// posters and the adult lock (`crate::player_catalog`).
    pub(crate) catalog: crate::player_catalog::Catalog,
    /// THE RANKING of the video catalogue: its page, its layout, the video
    /// whose own page is open and the thumbnails (`crate::player_ranking`).
    pub(crate) ranking: crate::player_ranking::Ranking,
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
    /// A letter of the catalogue's listing (`C-l`).
    CatalogLetter,
    /// A page of it.
    CatalogPage,
    /// The words it is searched by.
    CatalogSearch,
    /// A link pasted to add a source.
    SourceUrl,
    /// A link pasted to add a source straight to the adult catalogue.
    AdultSourceUrl,
    /// The person's system password, masked, for the adult catalogue.
    AdultPassword,
    /// A LibreTranslate server's address, for the lyrics' translation.
    TranslatorAddress,
    /// A translation key, masked.
    WordsKey(ronoctua_words::keys::Key),
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

/// Who said what [`Trouble::Unplayable`] quotes: the `ERROR:` line it keeps
/// is yt-dlp's, the program behind mpv's addresses — a program's NAME, data
/// like a host's, framed by `player-trouble-said`.
const SAID_BY: &str = "yt-dlp";

/// A trouble, in a sentence — one for every variant the player can say.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn trouble_sentence(t: &Trouble, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::{Arg, key};
    let with = |k: &str, args: &[(&str, &str)]| texts.t_with(k, args);
    match t {
        // WHAT THE PROGRAM SAID is data, framed by a key of ours and never
        // taken for a sentence (`Trouble::Unplayable::said`).
        Trouble::Unplayable { why, said } => {
            let unplayable = with(key::PLAYER_TROUBLE_UNPLAYABLE, &[("why", why)]);
            match said {
                Some(text) => format!(
                    "{unplayable} {}",
                    with(
                        key::PLAYER_TROUBLE_SAID,
                        &[("program", SAID_BY), ("text", text)]
                    )
                ),
                None => unplayable,
            }
        }
        Trouble::AlreadyQueued { name, .. } => with(
            key::PLAYER_TROUBLE_ALREADY_QUEUED,
            &[("name", &name_of(name, texts, u16::MAX))],
        ),
        Trouble::ChangedOnDisk { path } => {
            with(key::PLAYER_TROUBLE_CHANGED_ON_DISK, &[("path", path)])
        }
        Trouble::NoOrigin => texts.t(key::PLAYER_TROUBLE_NO_ORIGIN),
        Trouble::NoSound { .. } => texts.t(key::PLAYER_TROUBLE_NO_SOUND),
        Trouble::Translation(r) => with(
            key::PLAYER_TROUBLE_TRANSLATION,
            &[("why", &translation_refusal(r.why, texts))],
        ),
        // WHICH ASK is the log's (the engine's `not_yet`), never the screen's: a
        // kebab-case name is no sentence in any language.
        Trouble::NotYet { .. } => texts.t(key::PLAYER_TROUBLE_NOT_YET),
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
        Trouble::NotAnAddress { address } => {
            with(key::PLAYER_TRANSLATOR_NOT_AN_ADDRESS, &[("typed", address)])
        }
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
        // THE CATALOGUE'S: a source's refusal, the files', the person's
        // sources' file — one sentence per reason (`crate::player_catalog`).
        Trouble::Source { why } => texts.t(crate::player_catalog::source_trouble_key(*why)),
        Trouble::Catalog { why } => texts.t(crate::player_catalog::catalog_trouble_key(*why)),
        Trouble::Sources { why } => texts.t({
            use ronoctua_player::wire::SourcesTrouble as Why;
            match why {
                Why::NoHome => key::PLAYER_TROUBLE_SOURCES_NO_HOME,
                Why::Unreadable => key::PLAYER_TROUBLE_SOURCES_UNREADABLE,
                Why::Unparseable => key::PLAYER_TROUBLE_SOURCES_UNPARSEABLE,
                Why::Unwritable => key::PLAYER_TROUBLE_SOURCES_UNWRITABLE,
            }
        }),
        Trouble::NoSuchSource { .. } => texts.t(key::PLAYER_TROUBLE_NO_SUCH_SOURCE),
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
        Why::NoFieldIn { field, format } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_NO_FIELD_IN,
            &[
                ("format", &texts.t(tag_format_key(*format))),
                ("field", &texts.t(crate::player_metadata::field_key(*field))),
            ],
        ),
        Why::NoPicturesIn { format } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_NO_PICTURES_IN,
            &[("format", &texts.t(tag_format_key(*format)))],
        ),
        Why::StreamsOnlyInMatroska { container } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_STREAMS_ONLY_MATROSKA,
            &[("container", &texts.t(container_key(*container)))],
        ),
        Why::ChaptersOnlyInMatroska { container } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_CHAPTERS_ONLY_MATROSKA,
            &[("container", &texts.t(container_key(*container)))],
        ),
        Why::AttachmentIsNotAStream => texts.t(key::PLAYER_TROUBLE_TAG_ATTACHMENT),
        Why::RemuxWouldDrop => texts.t(key::PLAYER_TROUBLE_TAG_REMUX_WOULD_DROP),
        Why::TagReadOnly { format } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_READ_ONLY,
            &[("format", &texts.t(tag_format_key(*format)))],
        ),
        Why::OnlyTheFrontCover => texts.t(key::PLAYER_TROUBLE_TAG_ONLY_THE_FRONT_COVER),
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

/// A kind of tag, as a sentence names it — its key.
#[must_use]
pub const fn tag_format_key(format: ronoctua_tags::TagFormat) -> &'static str {
    use ronoctua_i18n::key;
    use ronoctua_tags::TagFormat as F;
    match format {
        F::Id3v2 => key::PLAYER_TAG_FORMAT_ID3V2,
        F::Id3v1 => key::PLAYER_TAG_FORMAT_ID3V1,
        F::Ape => key::PLAYER_TAG_FORMAT_APE,
        F::Vorbis => key::PLAYER_TAG_FORMAT_VORBIS,
        F::Mp4 => key::PLAYER_TAG_FORMAT_MP4,
        F::RiffInfo => key::PLAYER_TAG_FORMAT_RIFF_INFO,
        F::AiffText => key::PLAYER_TAG_FORMAT_AIFF_TEXT,
    }
}

/// Why the lyrics were not translated, in a sentence — one for every reason
/// the translating crate gives.
#[must_use]
pub fn translation_refusal(
    why: ronoctua_player::TranslationWhy,
    texts: &ronoctua_i18n::Texts,
) -> String {
    use ronoctua_i18n::key;
    use ronoctua_player::TranslationWhy as Why;
    texts.t(match why {
        Why::Empty => key::PLAYER_TROUBLE_TRANSLATION_EMPTY,
        Why::TooLong => key::PLAYER_TROUBLE_TRANSLATION_TOO_LONG,
        Why::UnknownTongue => key::PLAYER_TROUBLE_TRANSLATION_UNKNOWN_TONGUE,
        Why::WrongPass => key::PLAYER_TROUBLE_TRANSLATION_WRONG_PASS,
        Why::NoKey => key::PLAYER_TROUBLE_TRANSLATION_NO_KEY,
        Why::KeyRefused => key::PLAYER_TROUBLE_TRANSLATION_KEY_REFUSED,
        Why::Quota => key::PLAYER_TROUBLE_TRANSLATION_QUOTA,
        Why::Paced => key::PLAYER_TROUBLE_TRANSLATION_PACED,
        Why::NotFound => key::PLAYER_TROUBLE_TRANSLATION_NOT_FOUND,
        Why::NoDictionary => key::PLAYER_TROUBLE_TRANSLATION_NO_DICTIONARY,
        Why::Unreachable => key::PLAYER_TROUBLE_TRANSLATION_UNREACHABLE,
        Why::Failed => key::PLAYER_TROUBLE_TRANSLATION_FAILED,
        Why::Garbled => key::PLAYER_TROUBLE_TRANSLATION_GARBLED,
    })
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
        // THE SECURITY CENTRE'S, named in its own words (`Service::scope`).
        S::VirusTotal | S::XposedOrNot | S::PwnedPasswords => {
            crate::security_words::service_key(service)
        }
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
        Why::NotLet => key::PLAYER_TROUBLE_LOOKUP_NOT_LET,
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
        Why::NotLet => key::PLAYER_TROUBLE_DIRECTORY_NOT_LET,
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

/// How long lane `id`'s source runs, in seconds, once the player said it:
/// what its voice reads while it is on (`LaneState::duration_seconds`), what
/// the player knows of it while it is off (`State::lengths_off`). The ONE
/// answer the deck's bands, a lane's page and its "switched off" read.
#[must_use]
pub(crate) fn lane_length(s: &State, id: LaneId) -> Option<f64> {
    s.lane_states
        .iter()
        .find(|l| l.lane == id)
        .and_then(|l| l.duration_seconds)
        .or_else(|| {
            s.lengths_off
                .iter()
                .find(|l| l.lane == id)
                .map(|l| l.seconds)
        })
        .filter(|d| d.is_finite() && *d > 0.0)
}

/// What a lane that is OFF says it is doing: switched off — and, once the
/// player knows how long it runs, how long ([`lane_length`]).
#[must_use]
pub(crate) fn switched_off(s: &State, id: LaneId, texts: &ronoctua_i18n::Texts) -> String {
    match lane_length(s, id) {
        Some(seconds) => texts.t_with(
            ronoctua_i18n::key::PLAYER_LANE_SWITCHED_OFF_LENGTH,
            &[("length", clock(seconds).as_str())],
        ),
        None => texts.t(ronoctua_i18n::key::PLAYER_LANE_SWITCHED_OFF),
    }
}

/// A track's name among `kin`, the others of its kind: its title, its
/// language (by its name in the person's language), its codec — whichever
/// it has — and, where two of them would read the same, its codec after it,
/// and then its number.
///
/// ONE AT A TIME the old ring could say `eng` for three English sounds; side
/// by side on a page, three rows reading `eng` were three choices nobody can
/// tell apart — and the census of labels cannot see it, because these labels
/// are the file's.
#[must_use]
pub(crate) fn track_name(t: &Track, kin: &[&Track], texts: &ronoctua_i18n::Texts) -> String {
    let own = |o: &Track| own_name(o, texts);
    let twins = |name: &dyn Fn(&Track) -> String| {
        let mine = name(t);
        kin.iter().filter(|o| name(o) == mine).count() > 1
    };
    if !twins(&own) {
        return own(t);
    }
    let with_codec = |o: &Track| {
        let name = own(o);
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

/// A track's own name, alone: its title, its language — BY ITS NAME, from
/// the table of languages the searches bring (`eng` reads "English"), the
/// code as the file says it when the table has none — its codec; whichever
/// it has.
fn own_name(t: &Track, texts: &ronoctua_i18n::Texts) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(title) = t.title.as_deref().filter(|s| !s.is_empty()) {
        parts.push(title.to_owned());
    }
    if let Some(lang) = t.lang.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let name = crate::player_find::tongue_name(lang, texts);
        if !parts.iter().any(|p| p.eq_ignore_ascii_case(&name)) {
            parts.push(name);
        }
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
            translator_let: crate::player_translator::let_now(&ronoctua_player::prefs::read()),
            words_kept: crate::player_translator::kept_keys(),
            lyrics_tongue: None,
            kept: crate::player_internet::Kept::read(),
            directories: crate::player_directory::Directories::of_this_machine(),
            catalog: crate::player_catalog::Catalog::of_this_machine(),
            ranking: crate::player_ranking::Ranking::of_this_machine(),
            reads_prefs: true,
            ..Self::with_prefs(ronoctua_player::prefs::read())
        }
    }

    /// A fresh view on the preferences' DEFAULTS, whatever the person chose
    /// — a photograph's (`crate::photograph`), which is the same picture on
    /// every machine. Its picker's places are in `home`, as a home that is
    /// not this machine's has them (`file_picker::Context::in_home`), and
    /// this machine's own when `home` is `None`.
    #[must_use]
    pub fn blank(home: Option<std::path::PathBuf>) -> Self {
        Self {
            home,
            ..Self::with_prefs(ronoctua_player::prefs::Preferences::default())
        }
    }

    /// Whether a question of the view's waits for an answer on its rows —
    /// one it asked, "no" first, or a text open to be edited — which a door
    /// too narrow to draw rows must make room for.
    #[must_use]
    pub fn waits_for_an_answer(&self) -> bool {
        self.asking.is_some() || self.editing.is_some()
    }

    /// THE DOOR'S PICTURE: `None` while it shows none it could fold away,
    /// else whether it is folded — what the "Now" title's glyph and `z` stand
    /// for. The door's to say: only it has a picture. `true` when that
    /// changed, and with it the rows.
    pub fn picture_foldable(&mut self, fold: Option<bool>) -> bool {
        let changed = self.picture_fold != fold;
        self.picture_fold = fold;
        changed
    }

    /// `z` and the glyph on "Now": the picture folded away, or back — the
    /// door's to do. With no picture in this door there is nothing to fold,
    /// and that is said.
    fn fold_picture(&self, texts: &ronoctua_i18n::Texts) -> Errand {
        if self.picture_fold.is_none() {
            return Errand::Said(texts.t(ronoctua_i18n::key::PLAYER_NO_PICTURE_TO_FOLD));
        }
        Errand::FoldPicture
    }

    /// Whether a page of the file picker is on screen.
    #[must_use]
    pub fn browsing(&self) -> bool {
        self.top().picker.is_some()
    }

    /// How many lines of rows the door shows — told before a key is handed
    /// over, so the picker's `PgUp` and `PgDn` walk half a screen
    /// (`FilePicker::page_is`).
    pub fn page_is(&self, lines: usize) {
        if let Some(p) = self.top().picker.as_ref() {
            p.page_is(u16::try_from(lines).unwrap_or(u16::MAX));
        }
    }

    /// What the picker knows of this door's machine: its home — a
    /// photograph's, or this machine's — and its places. The player's doors
    /// light one cell of a grid (`chrome::hover_cells`), so the folder is a
    /// grid.
    fn pick_context(&self) -> crate::file_picker::Context {
        self.home.as_deref().map_or_else(
            || crate::file_picker::Context::of_this_machine(true),
            |home| crate::file_picker::Context::in_home(home, true),
        )
    }

    /// The picker of a page for `purpose`, at `start` when it has one.
    pub(crate) fn picker_for(
        &self,
        purpose: Purpose,
        start: Option<std::path::PathBuf>,
        name: String,
    ) -> crate::file_picker::FilePicker {
        let mut picker = crate::file_picker::FilePicker::open(
            purpose.request(start, name),
            &self.pick_context(),
        );
        // THE FOLDER IS WATCHED while the page is up, by a door with a loop.
        if let Some(wake) = self.wake.as_ref() {
            picker.watch_with(wake);
        }
        picker
    }

    /// A fresh view on `prefs`.
    fn with_prefs(prefs: ronoctua_player::prefs::Preferences) -> Self {
        Self {
            levels: vec![Level::home()],
            prefs,
            asking: None,
            clicked_control: None,
            home: None,
            swap: false,
            panel: false,
            no_room: false,
            outline: None,
            folds: std::collections::BTreeMap::new(),
            last_current: None,
            home_held: None,
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
            editing: None,
            picture_fold: None,
            translator_let: false,
            words_kept: Vec::new(),
            lyrics_tongue: None,
            websites: crate::player_website::Websites::default(),
            catalog: crate::player_catalog::Catalog::default(),
            ranking: crate::player_ranking::Ranking::default(),
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
                // AND THE CONSENT FILE beside it, where a press keeps the
                // hosts it lets (`prefs::let_merged`) and the translator's
                // switch keeps its host.
                // AND THE TRANSLATION KEYS, whose rows say whether each is
                // kept.
                let mut files = vec![
                    (dir.clone(), ronoctua_player::prefs::FILE),
                    (dir, ronoctua_player::prefs::CONSENT_FILE),
                ];
                files.extend(crate::player_translator::key_files());
                self.prefs_on_disk.look(&files, wake)
            }
            None => self.prefs_on_disk.take_stir(),
        };
        if read {
            self.prefs = ronoctua_player::prefs::read();
            self.translator_let = crate::player_translator::let_now(&self.prefs);
            self.words_kept = crate::player_translator::kept_keys();
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
    pub fn told(&mut self, answer: &Told, texts: &ronoctua_i18n::Texts) -> bool {
        match answer {
            Told::Listing { query, answer } => {
                self.listing_told(query, answer);
                true
            }
            // A REFUSAL THE PERSON MAY OVERRULE: asked on the page, "no"
            // first, and "yes" sends the ask that overrules it.
            Told::Asks { again, trouble } => {
                let Some((sentence, yes)) = self.overrule(again, trouble, texts) else {
                    return false;
                };
                self.ask(sentence, yes, Errand::Nothing);
                true
            }
            Told::KeyTrouble(_) => {
                self.kept.stir();
                true
            }
            Told::Unlock(answer) => {
                self.unlocked(*answer);
                true
            }
            // A VIDEO HIDDEN OR DELETED: its own page closes, and the
            // ranking is read again.
            Told::Ranking(outcome) => self.ranking_told(outcome),
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
        // A SUBTITLE IS LOOKED FOR BESIDE ITS FILM, and a cover beside its
        // file, where a scan of it lives: the picker opens on that folder
        // when it is here.
        let beside = match page {
            PageId::Pick(Purpose::Subtitle) => state
                .and_then(crate::player_subtitles::film)
                .map(|l| l.source.clone()),
            PageId::Pick(Purpose::Cover) => state.and_then(|s| self.meta.location(s)),
            _ => None,
        }
        .filter(|l| !l.contains("://"))
        .and_then(|l| {
            std::path::Path::new(&l)
                .parent()
                .map(std::path::Path::to_path_buf)
        })
        .filter(|f| f.is_dir());
        self.push(page, from);
        // THE WHOLE QUEUE opens on what plays, which its window was around.
        if page == PageId::Queue
            && self.page() == page
            && let Some(current) = state.and_then(|s| s.current)
        {
            self.follow(
                &crate::player_queue::spot_of(current),
                state,
                columns,
                texts,
            );
            self.top_mut().moved = true;
        }
        if let (PageId::Pick(purpose), Some(folder)) = (page, beside)
            && self.page() == page
        {
            self.top_mut().picker = Some(Box::new(self.picker_for(
                purpose,
                Some(folder),
                String::new(),
            )));
        }
        self.settle_in(state, columns, texts);
    }

    /// Which row the cursor is on.
    #[must_use]
    pub fn cursor(&self) -> usize {
        // A TEXT BEING EDITED keeps its caret in sight — or the answer its
        // question's cursor is on — where it was last placed.
        if self.asking.is_none()
            && let Some(area) = &self.editing
        {
            return area.cursor_row();
        }
        self.top().cursor
    }

    /// Opens a TEXT OF SEVERAL LINES over the page — a field's words, a
    /// description — in whichever door this view is: every key is its until
    /// it is left, `C-s` hands its words to [`View::keep_text`], and `Esc`
    /// asks before a change is thrown away (`crate::text_area`).
    pub fn edit_text(&mut self, area: crate::text_area::TextArea) {
        self.clicked_control = None;
        self.editing = Some(area);
    }

    /// Whether a text is being edited — a door that types capitals follows
    /// Caps Lock while one is.
    #[must_use]
    pub const fn editing_text(&self) -> bool {
        self.editing.is_some()
    }

    /// A key, as this view reads it: EVERY KEY IS THE TEXT'S while one is
    /// being edited (as a question takes every key), else [`press_of_key`]
    /// — the one reader both doors call.
    #[must_use]
    pub fn press_of_key(&self, key: ronoctua_keymap::Key) -> Option<Press> {
        if self.asking.is_none() && self.editing.is_some() {
            return Some(Press::Text(key));
        }
        // THE PICKER ON SCREEN takes what it reads — every letter is what is
        // typed there — and leaves the door the rest.
        if self.asking.is_none()
            && let Some(picker) = self.top().picker.as_ref()
            && picker.claims(key)
        {
            return Some(Press::Pick(key));
        }
        // A KEY TWO ENTRIES SHARE is the one whose page this is.
        press_of_key_where(key, |w| w.on_the_page(self))
    }

    /// What was copied, read by the door for [`Errand::Paste`] — typed into
    /// the text being edited, lines and all. `true` when it changed; a paste
    /// that arrives after the text was left is nobody's.
    pub fn text_pasted(&mut self, text: &str) -> bool {
        if self.editing.is_none()
            && let Some(picker) = self.top_mut().picker.as_mut()
        {
            let changed = picker.pasted(text) != crate::file_picker::Step::Nothing;
            if changed {
                self.top_mut().moved = false;
            }
            return changed;
        }
        self.editing
            .as_mut()
            .is_some_and(|area| area.paste(text) != crate::text_area::Step::Nothing)
    }

    /// The text being edited was WRITTEN where it belongs, as `text`: the
    /// area measures "changed" from it again. Called by what keeps it, once
    /// the write went through — never for a refused one.
    pub fn text_kept_as(&mut self, text: &str) {
        if let Some(area) = self.editing.as_mut() {
            area.kept_as(text);
        }
    }

    /// The text being edited was LEFT — thrown away, or left as kept: the
    /// page comes back, and nothing of it is waited for.
    fn text_left(&mut self) {
        self.editing = None;
        crate::player_metadata::edit_left(&mut self.meta);
    }

    /// A key typed into the text being edited, or `Esc` from a door.
    fn text_key(
        &mut self,
        press: Press,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use crate::text_area::Step;
        let key = match press {
            Press::Text(key) => key,
            Press::Escape => ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Esc),
            _ => return Errand::Nothing,
        };
        let Some(area) = self.editing.as_mut() else {
            return Errand::Nothing;
        };
        match area.key(key, area.drawn_width()) {
            Step::Nothing | Step::Moved | Step::Changed | Step::Asking => Errand::Nothing,
            Step::WantsPaste => Errand::Paste,
            Step::Keep(text) => {
                let id = area.id;
                self.keep_text(id, text, state, texts)
            }
            Step::Discard => {
                self.text_left();
                Errand::Nothing
            }
        }
    }

    /// `C-s` on the text opened as `id`: its words, written where they
    /// belong — by the view that opened it, which calls
    /// [`View::text_kept_as`] once the write went through.
    ///
    /// The metadata's "Edit here" (`player_metadata::EDIT_HERE`) sends the
    /// field's `Retag`, and the text is marked kept once the file read back
    /// says the words (`player_metadata::kept_back`, at the next settle). A
    /// text of anybody else — a photograph's — SAYS it keeps nothing, and
    /// stays open, changed, so `Esc` still asks.
    fn keep_text(
        &mut self,
        id: &'static str,
        text: String,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        if id == crate::player_metadata::EDIT_HERE {
            return crate::player_metadata::edit_kept(&mut self.meta, text, state, texts);
        }
        tracing::info!(
            id,
            bytes = text.len(),
            "a text asked to be kept, and nothing keeps it"
        );
        Errand::Said(texts.t(ronoctua_i18n::key::TEXT_AREA_NOT_KEPT))
    }

    /// The hint line.
    #[must_use]
    pub fn hint(
        &self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        // A TEXT BEING EDITED teaches its own keys, clickable.
        if self.asking.is_none() && self.editing.is_some() {
            return crate::text_area::hint(texts);
        }
        // THE PICKER teaches its own keys: every letter is typed there.
        if self.asking.is_none()
            && let Some(picker) = self.top().picker.as_ref()
        {
            return picker.hint(texts);
        }
        let rows = self.rows(state, columns, texts);
        let target = rows
            .get(self.cursor())
            .map_or(Target::Nothing, |r| r.target.clone());
        // A KEY THAT IS ONLY HERE BECAUSE OF THE ROW UNDER THE CURSOR reads as
        // one (`Register::Contextual`): the owner asked for the passing guests to
        // look apart from the keys that are always on the line.
        let here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(&target, state, self))
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        crate::chrome::hint_of_marked(&here, |_| None, texts)
    }

    /// A click on row `n` at pixel `x`, where the door's rows are `across`
    /// — the door's ONE answer, which its hover lights a grid's cell with
    /// too: on a grid row the cell under `x` ([`chrome::Across::cell`]), and
    /// nothing where no cell is lit; elsewhere, what [`Self::clicked_at`]
    /// takes at the column under `x`.
    ///
    /// [`chrome::Across::cell`]: crate::chrome::Across::cell
    pub fn clicked_px(
        &mut self,
        n: usize,
        x: f32,
        across: crate::chrome::Across,
        rows: &[Row],
    ) -> bool {
        match self.column_px(n, x, across, rows) {
            Some(column) => self.clicked_at(n, column, rows),
            None => false,
        }
    }

    /// [`Self::mark_clicked_at`] at pixel `x`, by the same answer as
    /// [`Self::clicked_px`].
    pub fn mark_clicked_px(
        &mut self,
        n: usize,
        x: f32,
        across: crate::chrome::Across,
        rows: &[Row],
        range: bool,
    ) -> bool {
        match self.column_px(n, x, across, rows) {
            Some(column) => self.mark_clicked_at(n, column, rows, range),
            None => false,
        }
    }

    /// The column a click at pixel `x` on row `n` takes: on a GRID row the
    /// first column of the cell under `x` — `None` where the hover lights
    /// none — and elsewhere the column under `x`, the row's left margin its
    /// first.
    fn column_px(
        &self,
        n: usize,
        x: f32,
        across: crate::chrome::Across,
        rows: &[Row],
    ) -> Option<u16> {
        let cells = crate::player_rows::grid_cells(rows, Row::owner(rows, n));
        if cells.is_empty() || self.editing.is_some() {
            return Some(across.column(x).unwrap_or(0));
        }
        across.cell(&cells, x).map(|c| cells[c].0)
    }

    /// A click on row `n`, at `column` (a field's `at`); `true` when the
    /// door should run what `RET` does — the second click on the same place,
    /// or a click on a control beside a title, which acts at once.
    pub fn clicked_at(&mut self, n: usize, column: u16, rows: &[Row]) -> bool {
        self.catalog_touched();
        // A TEXT BEING EDITED takes the click: the caret where it lands, or
        // an answer of its question chosen, then given (`text_area::hit`,
        // the cells it drew). Nothing else on its page is a place to click.
        if self.asking.is_none()
            && let Some(area) = self.editing.as_mut()
        {
            if let Some(hit) = crate::text_area::hit(rows, n, column)
                && area.clicked(hit) == crate::text_area::Step::Discard
            {
                self.text_left();
            }
            return false;
        }
        // THE REST OF A WRAPPED LABEL is its row: a click there is a click
        // on the row it continues.
        let line = n;
        let n = Row::owner(rows, n);
        let Some(row) = rows.get(n) else {
            return false;
        };
        // THE PICKER'S ROWS: what is under the click is chosen there — a
        // cell of its grid by the same arithmetic its hover lights
        // (`chrome::cell_under`) — and the second click is its `RET`.
        if self.asking.is_none()
            && let Some(target) = picked_under(row, column)
            && let Some(picker) = self.top_mut().picker.as_mut()
        {
            let again = picker.click_target(target);
            self.top_mut().cursor = n;
            self.top_mut().moved = true;
            return again;
        }
        // A CONTROL IS TAKEN ON THE LINE IT IS DRAWN ON, its row's first:
        // under it, on the rest of the row, the click is the row's
        // (`player_rows::controls_under`).
        let cell = row.cells.iter().position(|c| {
            (line == n || !c.control) && crate::chrome::in_cell(column, c.from, c.width)
        });
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

    /// A click with `Ctrl` held on row `n` at `column` — the file there
    /// marked, or no longer — or with `Shift` — every file from the cursor to
    /// it: the picker's own pointer marking (`FilePicker::mark_target`).
    /// `true` when that changed the rows.
    pub fn mark_clicked_at(&mut self, n: usize, column: u16, rows: &[Row], range: bool) -> bool {
        let n = Row::owner(rows, n);
        let Some(target) = rows.get(n).and_then(|r| picked_under(r, column)) else {
            return false;
        };
        let changed = self
            .top_mut()
            .picker
            .as_mut()
            .is_some_and(|p| p.mark_target(target, range));
        if changed {
            let top = self.top_mut();
            top.cursor = n;
            top.moved = true;
        }
        changed
    }

    /// The picker, open on `at` for `purpose`, in a home that is `at` — what a
    /// test of a door stands on, with no folder of the machine's read.
    #[cfg(test)]
    pub fn pick_at(&mut self, at: &std::path::Path, purpose: Purpose) {
        if self.home.is_none() {
            self.home = Some(at.to_path_buf());
        }
        self.push(PageId::Pick(purpose), None);
        self.top_mut().picker = Some(Box::new(self.picker_for(
            purpose,
            Some(at.to_path_buf()),
            String::new(),
        )));
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
        // THE LYRICS' LANGUAGE, remembered past a translation turned off.
        if let Some(t) = state.and_then(|s| s.lyrics_translation.as_ref()) {
            self.lyrics_tongue = Some(t.to.clone());
        }
        if self.page() == PageId::Tracks(TrackKind::Subtitles) {
            self.beside.look(state, self.wake.as_ref());
        }
        // THE ADDRESSES PLAYED LATELY, while their page is on screen.
        // ANOTHER DOOR'S play reorders it under the cursor, which stays on
        // its address.
        if self.page() == PageId::Website {
            let here = self.spot_here(state, columns, texts);
            if self.websites.look(self.wake.as_ref())
                && let Some(here) = here
            {
                self.follow(&here, state, columns, texts);
            }
        }
        // A FIELD EDITED HERE whose words the file now says: kept, and
        // leaving it asks nothing.
        if let Some(kept) = crate::player_metadata::kept_back(&mut self.meta, state) {
            self.text_kept_as(&kept);
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
                | PageId::Pick(Purpose::Cover)
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
        ) {
            self.directories.picks.look(self.wake.as_ref());
        }
        // THE RANKING'S PAGES read the video catalogue, off the loop in a
        // door that has one (`crate::player_ranking`).
        self.ranking_look(state, columns, texts);
        // THE CATALOGUE'S PAGES read the catalogue — off the loop in a door
        // that has one — and what a stale source shown here owes the player
        // is told it, off the loop too; a door with no loop keeps it.
        let owed = self.catalog_look(state);
        if self.wake.is_some() {
            for ask in owed {
                tell_then(ask, |answer| {
                    if answer != Told::Taken {
                        tracing::debug!(?answer, "the catalogue's ask was not taken");
                    }
                });
            }
        } else {
            self.catalog.owed.extend(owed);
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
        // THE PICKER'S FOLDER, read again when it moved on disk — the watch
        // woke the door (`FilePicker::look`).
        if let Some(picker) = self.top_mut().picker.as_mut() {
            picker.look();
        }
        let rows = self.rows(state, columns, texts);
        self.clamp(&rows);
        // THE PICKER KEEPS ITS OWN CURSOR, by identity: the view's is the row
        // it is on, for the doors to follow.
        if self.asking.is_none()
            && self.editing.is_none()
            && self.browsing()
            && let Some(i) = rows.iter().position(|r| r.focused)
        {
            self.top_mut().cursor = i;
            return;
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
        // A KEY is the person here: the adult catalogue's time starts again.
        self.catalog_touched();
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
        // A TEXT BEING EDITED takes every key typed, and `Esc` — which asks
        // before a change is lost. A press that acts wherever the cursor is
        // (play, the volume, the stages) still acts; one that would open a
        // page or move among rows the text hides does nothing.
        if self.editing.is_some() {
            if matches!(press, Press::Text(_) | Press::Escape) {
                return self.text_key(press, state, texts);
            }
            if press.unseen() != Unseen::Acts {
                return Errand::Nothing;
            }
        }
        // THE PICKER'S PAGE takes its keys, and the view's own that walk and
        // answer — a door's `RET` after a second click, a command's arrows,
        // the pointer's way back — as the picker's.
        if self.browsing()
            && let Some(key) = picker_key(press)
        {
            return self.pick_key(key, state, columns, texts);
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
            // A KEY FOR A TEXT with none open — one left while the key was
            // on its way — types nothing anywhere.
            Press::Text(_) | Press::Pick(_) => return Errand::Nothing,
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
                    self.open(PageId::Pick(Purpose::Queue), true, state, columns, texts);
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
            Press::SaveSet => return self.save(Saved::Set, state, columns, texts),
            Press::SaveQueue => return self.save(Saved::Queue, state, columns, texts),
            Press::Quieter | Press::Louder => {
                return volume_step(state, press == Press::Louder, texts);
            }
            Press::Swap => return button_errand("player-swap", state, texts),
            Press::EveryScreen => return Errand::EveryScreen,
            Press::Metadata => return self.show_the_metadata(state, columns, texts),
            // THE CATALOGUE'S PAGES search the catalogue.
            Press::Search if matches!(self.page(), PageId::Channels | PageId::Catalog) => {
                return self.search_the_catalogue();
            }
            Press::Search => return self.search_here(),
            Press::Listing(key) => return self.listing_pressed(key, state, columns, texts),
            Press::Ranking(key) => return self.ranking_pressed(key, state, columns, texts),
            Press::Globe(turn) => {
                self.globe_turned(turn);
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
            // TEN SECONDS BACK OR AHEAD, the seeks beside the clock.
            Press::Rewind | Press::Forward => {
                if !state.is_some_and(loaded) {
                    return Errand::Said(texts.t(ronoctua_i18n::key::PLAYER_NOTHING));
                }
                return crate::player_transport::seek_errand(press == Press::Forward, texts);
            }
            // THE PICTURE FOLDED AWAY, or back: the door's, which has one.
            Press::FoldPicture => return self.fold_picture(texts),
            Press::SaveQueueAgain => return self.save_again(state, columns, texts),
            Press::Translate => return self.translate(state, texts),
            Press::CountriesOrder => return self.countries_order(state, columns, texts),
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
        if press == Press::Enter {
            match target {
                // A PAGE, over this one.
                Target::Go(page) => {
                    self.open(page, false, state, columns, texts);
                    return Errand::Nothing;
                }
                Target::ClearQueue => return self.ask_to_clear(state, texts),
                Target::Swap => return button_errand("player-swap", state, texts),
                // THE TITLES' GLYPHS: the picture folded, the queue saved
                // again, the countries' order, the lyrics translated.
                Target::FoldPicture => return self.fold_picture(texts),
                Target::SaveQueueAgain => return self.save_again(state, columns, texts),
                Target::CountriesOrder => return self.countries_order(state, columns, texts),
                Target::Translate => return self.translate(state, texts),
                Target::Quit => return self.ask_to_quit(texts),
                Target::Restart => return self.ask_to_restart(texts),
                Target::Rename => return self.ask_a_name(&Target::Nothing, state, texts),
                Target::SaveSet => return self.save(Saved::Set, state, columns, texts),
                Target::SaveQueue => return self.save(Saved::Queue, state, columns, texts),
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
        // THE RANKING'S rows: a video, its `⋯`, the page and the layout, and
        // what a video's own page does (`crate::player_ranking`).
        if let Some(errand) = self.ranking_turned(&target, press, state, columns, texts) {
            return errand;
        }
        // THE LYRICS' LANGUAGE, and the host a translation was refused for.
        if let Some(errand) = self.translation_turned(&target, press, state, texts) {
            return errand;
        }
        // THE WEBSITE PAGE'S rows: its two questions and the addresses
        // played lately (`crate::player_website`).
        if let Some(errand) = self.website_turned(&target, press, state, columns, texts) {
            return errand;
        }
        // THE DIRECTORIES' rows: a list asked or opened, the search, a row
        // that plays and its star (`crate::player_directory`).
        if let Some(errand) = self.directory_turned(&target, press, state, columns, texts) {
            return errand;
        }
        // THE CATALOGUE'S rows: its source, a listing's rings, an item, the
        // adult lock (`crate::player_catalog`) — and its sources' switches
        // (`crate::player_sources`).
        if let Some(errand) = self.catalog_turned(&target, press, state, columns, texts) {
            return errand;
        }
        if let Some(errand) = self.sources_turned(&target, press, state, texts) {
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
        // AND ITS TRANSLATOR: who, the server's address, the keys.
        if let Some(errand) = self.translator_turned(&target, press, texts) {
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
        // AN ENTRY MOVED ON THE HOME stays in its window's sight — the
        // window may grow by it, and the cursor stays on the row it was on.
        if let (Target::Entry(id) | Target::Group(id), Press::Earlier | Press::Later)
        | (Target::EntryMove { id, .. }, Press::Enter) = (&target, press)
            && self.page() == PageId::Home
            && self.home_held != Some(*id)
        {
            let here = self.spot_here(state, columns, texts);
            self.home_held = Some(*id);
            if let Some(here) = here {
                self.follow(&here, state, columns, texts);
            }
        }
        turn(&target, press, state, self.outline.as_deref())
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
            // A FIELD'S WORDS, opened over the page to be changed here.
            Turned::Edit { title, text } => {
                self.edit_text(
                    crate::text_area::TextArea::new(crate::player_metadata::EDIT_HERE, &text)
                        .titled(title),
                );
                Errand::Nothing
            }
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
    pub fn picture(&self, s: Option<&State>, key: &str) -> Option<&crate::player_find::Picture> {
        // A POSTER OF THE CATALOGUE's listing (`crate::player_posters`) first:
        // channels by theme are there with no player running, and so are
        // their posters.
        self.catalog
            .posters
            .picture(key)
            // AND A THUMBNAIL OF THE RANKING (`crate::player_ranking`), there
            // with no player running too.
            .or_else(|| self.ranking.posters.picture(key))
            .or_else(|| s.and_then(|s| self.meta.cover(s)).filter(|(k, _)| k == key))
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
            && matches!(self.page(), PageId::Pick(Purpose::Lane | Purpose::Subtitle))
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

    /// SAVING: the picker, where a folder is chosen and the name is typed —
    /// holding, for what plays, the name it has now; for the queue, none.
    /// With nothing to save it says so.
    pub(crate) fn save(
        &mut self,
        saved: Saved,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use ronoctua_i18n::key;
        let name = match saved {
            Saved::Set => {
                let Some(s) = state.filter(|s| loaded(s)) else {
                    return Errand::Said(texts.t(key::PLAYER_NOTHING));
                };
                s.current
                    .zip(self.outline.as_deref())
                    .and_then(|(id, o)| node_of(o, id))
                    .map(|n| name_of(&n.name, texts, NAME_IN_A_SENTENCE))
                    .or_else(|| s.title.clone())
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_default()
            }
            Saved::Queue => {
                if self.outline.as_ref().is_none_or(|o| o.entries.is_empty()) {
                    return Errand::Said(texts.t(key::PLAYER_QUEUE_EMPTY));
                }
                String::new()
            }
        };
        self.open_saving(saved, name, state, columns, texts);
        Errand::Nothing
    }

    /// The save's picker, holding `name`.
    fn open_saving(
        &mut self,
        saved: Saved,
        name: String,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let purpose = match saved {
            Saved::Set => Purpose::SaveSet,
            Saved::Queue => Purpose::SaveQueue,
        };
        let page = PageId::Pick(purpose);
        self.open(page, true, state, columns, texts);
        if self.page() == page {
            self.top_mut().picker = Some(Box::new(self.picker_for(purpose, None, name)));
            self.settle_in(state, columns, texts);
        }
    }

    /// A key for the picker on screen.
    fn pick_key(
        &mut self,
        key: ronoctua_keymap::Key,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(picker) = self.top_mut().picker.as_mut() else {
            return Errand::Nothing;
        };
        let step = picker.key(key);
        let errand = self.stepped(step, state, columns, texts);
        self.settle_in(state, columns, texts);
        errand
    }

    /// What the picker's step is to this view.
    fn stepped(
        &mut self,
        step: crate::file_picker::Step,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use crate::file_picker::Step;
        match step {
            Step::Nothing | Step::Changed => Errand::Nothing,
            Step::Back => {
                self.back(state, columns, texts);
                Errand::Nothing
            }
            Step::Say(e) => Errand::Said(texts.t_with(e.key, &e.borrowed())),
            Step::Paste => Errand::Paste,
            Step::Picked(picked) => match self.page() {
                PageId::Pick(purpose) => self.picked(purpose, picked, state, columns, texts),
                _ => Errand::Nothing,
            },
        }
    }

    /// What the picker answered, for `purpose`: into the queue — and the page
    /// stays, for the next — or played together, laid over the picture, the
    /// cover, a set or a queue opened, where a file is saved: the page goes,
    /// back on the row that asked, once the choice was taken.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn picked(
        &mut self,
        purpose: Purpose,
        picked: crate::file_picker::Picked,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use crate::file_picker::{How, PLAY_FOLDER, Picked};
        use ronoctua_i18n::key;
        let location = |p: &std::path::Path| p.to_string_lossy().into_owned();
        match (purpose, picked) {
            (Purpose::Queue, Picked::Paths { paths, how }) => {
                let play = how == How::Alternate;
                let said = match paths.as_slice() {
                    [one] => texts.t_with(
                        if play {
                            key::PLAYER_PLAYING_NOW
                        } else {
                            key::PLAYER_ADDED
                        },
                        &[("name", &file_name(one))],
                    ),
                    several => texts.t_args(
                        if play {
                            key::PLAYER_PLAYING_SEVERAL
                        } else {
                            key::PLAYER_ADDED_SEVERAL
                        },
                        &[("count", number(several.len()))],
                    ),
                };
                // THROUGH `put_all`, where the rule of what goes in ONCE is
                // — the one a single location follows too.
                Errand::TellSaying(
                    put_all(paths.iter().map(|p| location(p)).collect(), play),
                    said,
                )
            }
            (Purpose::Queue, Picked::Address(url)) => Errand::TellSaying(
                put(url.clone(), false),
                texts.t_with(key::PLAYER_ADDED, &[("name", &url)]),
            ),
            // A FOLDER WHOLE, played now or added: ONE location, which the
            // player makes a playlist named after the folder. What it plays
            // of it is the player's walk; a folder with nothing to play is
            // said here, before it is sent.
            (Purpose::Queue, Picked::Extra { id, folder }) => {
                if crate::player_kinds::gather(&folder).is_empty() {
                    return Errand::Said(texts.t(key::PLAYER_BROWSE_NOTHING_TO_PLAY));
                }
                let play = id == PLAY_FOLDER.id;
                Errand::TellSaying(
                    put(location(&folder), play),
                    texts.t_with(
                        if play {
                            key::PLAYER_PLAYING_NOW
                        } else {
                            key::PLAYER_ADDED
                        },
                        &[("name", &file_name(&folder))],
                    ),
                )
            }
            // TOGETHER, a file is a lane of what plays: chosen, it plays with
            // it on one clock, and the page that asked for it is back.
            (Purpose::Lane, Picked::Paths { paths, .. }) if !paths.is_empty() => {
                self.back(state, columns, texts);
                let source = location(&paths[0]);
                Errand::TellSaying(
                    crate::player_question::together(source),
                    texts.t_with(
                        key::PLAYER_TOGETHER_ADDED,
                        &[("name", &file_name(&paths[0]))],
                    ),
                )
            }
            (Purpose::Lane, Picked::Address(url)) => {
                self.back(state, columns, texts);
                Errand::TellSaying(
                    crate::player_question::together(url.clone()),
                    texts.t_with(key::PLAYER_TOGETHER_ADDED, &[("name", &url)]),
                )
            }
            // A SUBTITLE FILE, over the picture: a lane of its own.
            (Purpose::Subtitle, Picked::Paths { paths, .. }) if !paths.is_empty() => {
                let said = state.map_or(Errand::Nothing, |s| {
                    crate::player_subtitles::lay_over(&location(&paths[0]), s, texts)
                });
                if matches!(said, Errand::TellSaying(..)) {
                    self.back(state, columns, texts);
                }
                said
            }
            // A PICTURE FOR THE COVER, put in the file shown.
            (Purpose::Cover, Picked::Paths { paths, .. }) if !paths.is_empty() => {
                let said = state.and_then(|s| {
                    crate::player_metadata::cover_picked(&self.meta, s, &paths[0], texts)
                });
                if matches!(said, Some(Errand::TellSaying(..))) {
                    self.back(state, columns, texts);
                }
                said.unwrap_or(Errand::Nothing)
            }
            // ONE OF THE PLAYER'S OWN FILES, opened: a set plays, a queue
            // takes the queue's place after a question.
            (Purpose::Open, Picked::Paths { paths, .. }) if !paths.is_empty() => {
                self.open_saved(&paths[0], state, columns, texts)
            }
            // WHERE A SET OR THE QUEUE IS SAVED — the picker asked already,
            // "no" first, when a file of that name was there.
            (Purpose::SaveSet | Purpose::SaveQueue, Picked::Paths { paths, .. })
                if !paths.is_empty() =>
            {
                let at = &paths[0];
                let stem = at
                    .file_stem()
                    .map(|n| n.to_string_lossy().into_owned())
                    .filter(|n| !n.is_empty());
                let to = location(at);
                let ask = if purpose == Purpose::SaveSet {
                    Ask::SaveSet { to, name: stem }
                } else {
                    Ask::SaveQueue { to, name: stem }
                };
                self.back(state, columns, texts);
                Errand::TellSaying(
                    ask,
                    texts.t_with(
                        key::PLAYER_SAVING,
                        &[
                            ("name", &file_name(at)),
                            ("folder", &shown_path(at.parent().unwrap_or(at))),
                        ],
                    ),
                )
            }
            _ => Errand::Nothing,
        }
    }

    /// What was typed for one of the player's questions, IN THIS VIEW — the
    /// ONE function both doors hand every answer to while the view is open.
    ///
    /// The view's own questions — a new name, when a lane begins — are
    /// answered here, where it is known what they were about ([`Typing`]);
    /// every other one is `player_question::answered_saying`'s. An empty
    /// answer is never mind, as it is for every question. A save's name
    /// typed from no question (`LEADER :` with an argument) opens the save's
    /// picker holding it.
    pub fn answered(
        &mut self,
        id: &str,
        said: &str,
        pairing: &mut crate::player_machines::Pairing,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use crate::player_question::{RENAME, SAVE_QUEUE, SAVE_SET};
        let asked_before = self.asking.is_some();
        let errand = match id {
            RENAME => self.renamed(said, texts),
            SAVE_SET | SAVE_QUEUE => {
                let saved = if id == SAVE_SET {
                    Saved::Set
                } else {
                    Saved::Queue
                };
                let name = file_stem_of(said, saved).unwrap_or_default();
                self.open_saving(saved, name, state, columns, texts);
                Errand::Nothing
            }
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
            // AN ADDRESS OF A WEBSITE, played now or queued.
            id if crate::player_website::asks(id) => self.website_typed(id, said, texts),
            // A DIRECTORY'S SEARCH: the list of what it names, opened.
            id if crate::player_directory::asks(id) => {
                self.searched(id, said, state, columns, texts)
            }
            // THE CATALOGUE'S: a letter, a page, words, a pasted source, the
            // password.
            id if crate::player_catalog::asks(id) => {
                self.catalog_answered(id, said, state, columns, texts)
            }
            crate::player_question::KEY_OPENSUBTITLES
            | crate::player_question::KEY_TMDB
            | crate::player_question::KEY_SUBDL => match self.typing.take() {
                Some(Typing::Key(k)) => self.key_typed(k, said),
                _ => Errand::Nothing,
            },
            // THE LYRICS' TRANSLATOR: a server's address, a key.
            // THE ROW UNDER THE CURSOR, BY ITS SPOT: an address takes room
            // in the value column, and a label above may wrap into two rows.
            id if crate::player_translator::asks(id) => {
                let here = self.spot_here(state, columns, texts);
                let errand = self.translator_answered(said, texts);
                if let Some(here) = here {
                    self.follow(&here, state, columns, texts);
                }
                errand
            }
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
        // AN ADULT LISTING HANDED OVER whose password was let go is let go
        // with it (`crate::player_catalog`).
        if self.typing == Some(Typing::AdultPassword) {
            self.catalog.waiting = None;
        }
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
            // TYPED ON A LANE'S ROW, it moves that lane alone: the sound's
            // start typed is how it is put back in step with its picture.
            Ok(seconds) => Errand::Tell(Ask::Lane {
                lane: id,
                change: LaneChange::Start {
                    seconds,
                    voice: false,
                },
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
                    voice: false,
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
        if self.page() == PageId::Pick(Purpose::Open) {
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
                        once: false,
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
        // THE PICKER'S PAGE: its title, and the picker's own rows — its
        // cells the cells of a grid, each lit alone under the pointer
        // (`chrome::hover_cells`). A question of the view's, or a text being
        // edited, is drawn over it as over any page.
        if self.asking.is_none()
            && self.editing.is_none()
            && let Some(picker) = top.picker.as_ref()
        {
            let mut rows = Builder::build(left, room, usize::MAX, 0, texts, |b| {
                b.heading(top.page.key());
                // WHAT OPENING DOES, before a file is chosen: a set plays, a
                // queue takes the queue's place.
                if top.page == PageId::Pick(Purpose::Open) {
                    b.note(ronoctua_i18n::key::PLAYER_ABOUT_OPEN);
                }
            });
            rows.extend(
                picker
                    .rows(columns, texts)
                    .into_iter()
                    .map(crate::player_rows::of_the_picker),
            );
            return rows;
        }
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
        // A TEXT BEING EDITED is the page while it is open.
        if let Some(area) = &self.editing {
            b.text_area(area);
            return;
        }
        let page = top.page;
        // A LIST'S PAGE is titled by what it lists — a country, a genre, what
        // was searched for — and by its own key while it lists nothing. A
        // LISTING OF THE CATALOGUE carries its layout and its whole screen
        // beside its title.
        if let Some(title) = self.catalog_title(b.texts) {
            let layout = self
                .catalog
                .listing
                .as_ref()
                .map_or(crate::player_catalog::Layout::List, |l| l.layout);
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
        } else if let Some(title) = self.listing_title(b.texts) {
            b.heading_said(title);
        } else if page == PageId::Ranking {
            // THE RANKING carries its layout beside its title.
            self.ranking_title(b);
        } else if page != PageId::Home && !(page == PageId::Queue && state.is_some()) {
            // THE WHOLE QUEUE titles itself, with the queue's controls on its
            // title (`player_queue::queue`).
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
            // THE PICKER'S PAGE is drawn by [`View::rows`], its own rows.
            (PageId::Pick(_), _) => {}
            (PageId::Internet, _) => self.internet(b),
            (PageId::Website, _) => self.website(b),
            (PageId::Queue, Some(s)) => {
                crate::player_queue::queue(self, s, self.outline.as_deref(), true, b);
            }
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
                | PageId::Favourites
                | PageId::Recent,
                Some(s),
            ) => self.directory_rows(page, s, b),
            // CHANNELS BY THEME, ITS LISTING AND ITS SOURCES: the local
            // catalogue's, read by this door itself (`crate::player_catalog`).
            (PageId::Channels | PageId::Catalog | PageId::Sources, _) => {
                self.catalog_rows(page, state, b);
            }
            // THE RANKING AND A VIDEO'S OWN PAGE: the video catalogue's,
            // read by this door itself (`crate::player_ranking`).
            (PageId::Ranking, _) => self.ranking_rows(b),
            (PageId::RankedVideo, _) => self.ranked_video_rows(b),
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
                | PageId::Favourites
                | PageId::Recent
                | PageId::Queue,
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
    put_all(vec![location], play)
}

/// Puts several locations in the queue, as [`put`] puts one — the ONE place
/// an `Ask::Put` of what a person chose is written, so what a single file
/// and a marked set go in under is one rule.
#[must_use]
pub(crate) fn put_all(locations: Vec<String>, play: bool) -> Ask {
    Ask::Put {
        what: locations.into_iter().map(Thing::Location).collect(),
        after: None,
        into: None,
        play,
        replace: false,
        // PUT AT THE QUEUE'S END, it is asked ONCE: one already there is
        // refused, and the person asked whether to add it again (`Told::Asks`,
        // decision A5). Played now it plays, wherever else it is.
        once: !play,
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
                        voice: false,
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
                    voice: false,
                },
            })
        }),
        // WHICH PICTURE A SUBTITLE SITS OVER: a ring, any key turns it.
        (Target::LaneOver(id), Press::Enter | Press::Less | Press::More) => {
            crate::player_lane::over_turned(s, *id, forward)
        }
        // WHICH OF ITS FILE'S LANGUAGES A SUBTITLE SHOWS: a ring, any key.
        (Target::LaneLanguage(id), Press::Enter | Press::Less | Press::More) => {
            crate::player_lane::language_turned(s, *id, forward)
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
        // AN ASK A PERSON MAY OVERRULE comes back with the ask that does:
        // its refusal is a question of the view's, "no" first (`Told::Asks`).
        let asked = matches!(
            ask,
            Ask::Put { once: true, .. } | Ask::SaveQueueAgain { force: false }
        )
        .then(|| ask.clone());
        tell_then(ask, move |answer| {
            let answer = match (listing, &answer, asked) {
                (Some(query), _, _) => Told::Listing {
                    query,
                    answer: Box::new(answer),
                },
                (None, Told::Refused(trouble), Some(asked)) => match overruled(&asked, trouble) {
                    Some(again) => Told::Asks {
                        again: Box::new(again),
                        trouble: trouble.clone(),
                    },
                    None => answer,
                },
                (None, ..) => answer,
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
        | Errand::Paste
        | Errand::FoldPicture
        | Errand::Ask(_)
        | Errand::Kiosk(_)
        | Errand::Pair { .. } => Err(errand),
        // A VIDEO OF THE RANKING hidden, deleted or downloaded, OFF THE LOOP —
        // a write waits for a scan that holds the catalogue — and said when
        // it is done (`Told::Ranking`).
        Errand::Ranking(act) => {
            crate::daemon::about_to_act("a video of the ranking, hidden, deleted or downloaded");
            let told = told.cloned();
            let born = std::thread::Builder::new()
                .name("player-ranking-act".to_owned())
                .spawn(move || {
                    let outcome = crate::player_ranking::carry(act);
                    match told {
                        Some(told) => {
                            let _ = told.send(Told::Ranking(outcome));
                        }
                        None => {
                            tracing::info!(?outcome, "the ranking's act, with no door to say it")
                        }
                    }
                });
            if let Err(e) = born {
                tracing::error!(error = %e, "could not start the thread that acts on a video");
            }
            Ok(None)
        }
        // THE PERSON'S PASSWORD, checked OFF THE LOOP — PAM waits seconds on
        // a wrong one — and how it ended told back (`Told::Unlock`): the
        // view opens the adult catalogue, or says why not.
        Errand::Unlock(secret) => {
            crate::daemon::about_to_act("the person's system password, checked");
            let told = told.cloned();
            let born = std::thread::Builder::new()
                .name("player-unlock".to_owned())
                .spawn(move || {
                    let answer = crate::player_catalog::unlock(&secret);
                    drop(secret);
                    if let Some(told) = told {
                        let _ = told.send(Told::Unlock(answer));
                    }
                });
            if let Err(e) = born {
                tracing::error!(error = %e, "could not start the thread that checks the password");
                return Ok(Some(texts.t(ronoctua_i18n::key::PLAYER_ADULT_UNAVAILABLE)));
            }
            Ok(None)
        }
        Errand::Said(sentence) => Ok(Some(sentence)),
        // AN ADDRESS OF THE WEBSITE PAGE, kept or forgotten — merged with the
        // file as it is on disk; a list that could not be written says so.
        Errand::Websites(change) => {
            crate::daemon::about_to_act("the addresses played lately, written");
            Ok(crate::player_website::keep(&change)
                .err()
                .map(|why| why.sentence(texts)))
        }
        // HOSTS LET — the lyrics' translator, a default source of the
        // catalogue — in the person's consent file, merged by host; and what
        // the press asked, after it: the daemon reads the file when it asks.
        Errand::LetHosts { hosts, on, then } => {
            crate::daemon::about_to_act("the person's consents, written");
            let hosts: Vec<&str> = hosts.iter().map(String::as_str).collect();
            match ronoctua_consent::let_hosts(&hosts, on) {
                Ok(()) => carry_out(*then, texts, told),
                Err(why) => {
                    use ronoctua_consent::Unkept;
                    use ronoctua_i18n::key;
                    let why = texts.t(match why {
                        Unkept::NoHome => key::PLAYER_FILE_NO_HOME,
                        Unkept::Unparseable => key::PLAYER_FILE_UNPARSEABLE,
                        Unkept::Unwritable => key::PLAYER_FILE_UNWRITABLE,
                    });
                    Ok(Some(texts.t_with(
                        key::ECHO_SETTING_NOT_WRITTEN,
                        &[("file", ronoctua_consent::FILE), ("why", &why)],
                    )))
                }
            }
        }
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
        // A TRANSLATION KEY, kept or forgotten OFF THE LOOP — synced to the
        // disk before it is renamed into place; a trouble is said when the
        // thread says it.
        Errand::KeepWordsKey { key, secret } => {
            crate::daemon::about_to_act("a translation key of the person's, kept");
            words_key_off_the_loop(
                move || ronoctua_words::keys::keep(key, &secret),
                told.cloned(),
                true,
            );
            Ok(Some(crate::player_translator::kept_said(key, texts)))
        }
        Errand::ForgetWordsKey(key) => {
            crate::daemon::about_to_act("a translation key of the person's, forgotten");
            words_key_off_the_loop(
                move || ronoctua_words::keys::forget(key),
                told.cloned(),
                false,
            );
            Ok(Some(texts.t(ronoctua_i18n::key::PLAYER_KEY_FORGOTTEN)))
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

/// `write` — a translation key kept or forgotten — on a thread of its own,
/// and a trouble it met said down `told`.
fn words_key_off_the_loop(
    write: impl FnOnce() -> Result<(), ronoctua_words::keys::KeyTrouble> + Send + 'static,
    told: Option<ToldSender>,
    reread: bool,
) {
    let born = std::thread::Builder::new()
        .name("player-words-key".to_owned())
        .spawn(move || match write() {
            // A KEY KEPT: a player that runs reads its preferences again, and
            // asks again the translation it was refused for want of one.
            Ok(()) if reread => tell_if_running(&Ask::Reload),
            Ok(()) => {}
            Err(t) => {
                if let Some(told) = told {
                    let _ = told.send(Told::KeyTrouble(crate::player_translator::trouble_of(t)));
                }
            }
        });
    if let Err(e) = born {
        tracing::error!(error = %e, "could not start the thread that keeps a translation key");
    }
}

/// `switch` written into `player.json` MERGED with what is on disk: read as
/// it is there, changed only in what was pressed, and written back whole. A
/// file that will not parse is refused, never written over.
fn kept_merged(switch: &Switch) -> Result<(), NotLet> {
    // A SWITCH THAT ASKS ON ITS OWN lets the host it asks: turned on, it is
    // the person's consent to that host, and without it in the consent file
    // the first lookup it starts would come back refused, as a row asking
    // for what the switch already said.
    let asks = match switch {
        Switch::LyricsOnline(true) => Some(ronoctua_lookup::Service::Lrclib),
        Switch::Sponsors(true) => Some(ronoctua_lookup::Service::SponsorBlock),
        _ => None,
    };
    if let Some(service) = asks {
        ronoctua_player::prefs::let_hosts(&ronoctua_player::prefs::hosts_of(&[service]), true)
            .map_err(|why| match why {
                ronoctua_player::prefs::Unlet::Consent(unkept) => NotLet::Consent(unkept),
                ronoctua_player::prefs::Unlet::WillNotParse => NotLet::WillNotParse,
                ronoctua_player::prefs::Unlet::Unwritable(e) => NotLet::Unwritable(e),
            })?;
    }
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
    /// The consent file would not take it.
    Consent(ronoctua_player::prefs::ConsentUnkept),
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
            Self::Consent(unkept) => texts.t({
                use ronoctua_i18n::key;
                use ronoctua_player::prefs::ConsentUnkept as Unkept;
                match unkept {
                    Unkept::NoHome => key::PLAYER_CONSENT_UNKEPT_NO_HOME,
                    Unkept::Unparseable => key::PLAYER_CONSENT_UNKEPT_UNPARSEABLE,
                    Unkept::Unwritable => key::PLAYER_CONSENT_UNKEPT_UNWRITABLE,
                }
            }),
        }
    }
}

/// The services and directories let — or no longer — KEPT where their
/// consent lives: their hosts in the consent file, merged by host, and the
/// player's older lists losing their word for one taken off
/// (`ronoctua_player::prefs::let_merged`). A file that will not parse is
/// refused, never written over.
fn let_merged(
    services: &[ronoctua_lookup::Service],
    directories: &[ronoctua_player::find::Directory],
    on: bool,
) -> Result<(), NotLet> {
    use ronoctua_player::prefs::Unlet;
    ronoctua_player::prefs::let_merged(services, directories, on).map_err(|why| match why {
        Unlet::WillNotParse => NotLet::WillNotParse,
        Unlet::Unwritable(e) => NotLet::Unwritable(e),
        Unlet::Consent(unkept) => NotLet::Consent(unkept),
    })
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

/// What the file picker on screen takes of a press of the view: its own
/// keys, and the view's that walk, answer and go back — a door's `RET` after a
/// second click, a command's arrows, the pointer's way back. `None` for the
/// rest, which stay the view's: play, the volume, the stages.
fn picker_key(press: Press) -> Option<ronoctua_keymap::Key> {
    use ronoctua_keymap::{Key, Sym};
    Some(match press {
        Press::Pick(key) => key,
        Press::Up => Key::simple(Sym::Up),
        Press::Down => Key::simple(Sym::Down),
        Press::Less => Key::simple(Sym::Left),
        Press::More => Key::simple(Sym::Right),
        Press::Enter => Key::simple(Sym::Enter),
        Press::Escape => Key::simple(Sym::Esc),
        _ => return None,
    })
}

/// The picker's target under `column` of `row` — a cell of its grid, found
/// by the arithmetic its hover lights with (`chrome::cell_under`), else the
/// row's own. `None` off the picker's rows.
fn picked_under(row: &Row, column: u16) -> Option<crate::file_picker::Target> {
    let of = |t: &Target| match t {
        Target::Picker(t) if *t != crate::file_picker::Target::Nothing => Some(t.clone()),
        _ => None,
    };
    let grid: Vec<&crate::player_rows::Cell> = row.cells.iter().filter(|c| !c.control).collect();
    if grid.is_empty() {
        return of(&row.target);
    }
    let n = crate::chrome::cell_under(grid.iter().map(|c| (c.from, c.width)), column)?;
    of(&grid[n].target)
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
        // THE SEEKS BESIDE THE CLOCK, the picture folded, the queue saved
        // again, the countries' order, the lyrics translated — and THE
        // WEBSITE PAGE, `LEADER p h`.
        "player-rewind" => Press::Rewind,
        "player-forward" => Press::Forward,
        "player-picture-fold" => Press::FoldPicture,
        "player-save-queue-again" => Press::SaveQueueAgain,
        "player-countries-order" => Press::CountriesOrder,
        "player-lyrics-translate" => Press::Translate,
        "player-website" => Press::Go(PageId::Website),
        // A LISTING OF THE CATALOGUE's keys (`crate::player_catalog`).
        "player-listing-earlier" => Press::Listing(ListingKey::Earlier),
        "player-listing-later" => Press::Listing(ListingKey::Later),
        "player-listing-first" => Press::Listing(ListingKey::First),
        "player-listing-last" => Press::Listing(ListingKey::Last),
        "player-listing-full" => Press::Listing(ListingKey::Full),
        "player-listing-layout" => Press::Listing(ListingKey::Layout),
        "player-listing-source" => Press::Listing(ListingKey::Source),
        "player-listing-letter" => Press::Listing(ListingKey::Letter),
        "player-listing-order" => Press::Listing(ListingKey::Order),
        // THE RANKING — `LEADER p k` — and its keys (`crate::player_ranking`).
        "player-ranking" => Press::Go(PageId::Ranking),
        id if crate::player_ranking::RankingKey::of_command(id).is_some() => {
            Press::Ranking(crate::player_ranking::RankingKey::of_command(id)?)
        }
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
    press_of_key_where(key, |_| None)
}

/// [`press_of_key`], with a key two entries share read by `page`
/// ([`pressed_where`]).
fn press_of_key_where(
    key: ronoctua_keymap::Key,
    page: impl Fn(Where) -> Option<bool>,
) -> Option<Press> {
    use ronoctua_keymap::{Key, Sym};
    match key.sym {
        Sym::Esc => Some(Press::Escape),
        Sym::Char(_) => pressed_where(&key.notation(), &page),
        other => pressed_where(&key.notation(), &page)
            .or_else(|| pressed_where(&Key::simple(other).notation(), &page)),
    }
}

/// Which press a notation is, through [`OF_THE_VIEW`] — the first entry of
/// the key. The tests' reader: a door reads a key through the view
/// ([`View::press_of_key`]), which knows the page.
#[cfg(test)]
#[must_use]
pub fn pressed(notation: &str) -> Option<Press> {
    pressed_where(notation, |_| None)
}

/// Which press a notation is ON THE PAGE `page` answers for
/// ([`Where::on_the_page`]): among the entries that share the key, the
/// first whose page this is, else the first the page does not rule out —
/// the row decides that one in [`View::key`] — else the first. With one
/// entry, that entry.
fn pressed_where(notation: &str, page: impl Fn(Where) -> Option<bool>) -> Option<Press> {
    pressed_in(OF_THE_VIEW, notation, page)
}

/// [`pressed_where`], over `table`.
fn pressed_in(
    table: &[(&str, &str, Where)],
    notation: &str,
    page: impl Fn(Where) -> Option<bool>,
) -> Option<Press> {
    let sharing: Vec<&(&str, &str, Where)> =
        table.iter().filter(|(k, _, _)| *k == notation).collect();
    sharing
        .iter()
        .find(|(_, _, w)| page(*w) == Some(true))
        .or_else(|| sharing.iter().find(|(_, _, w)| page(*w).is_none()))
        .or_else(|| sharing.first())
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
                    languages: Vec::new(),
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
                languages: Vec::new(),
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

    /// FAILS IF: a reason a file's tags are refused, or the lyrics'
    /// translation is, has no sentence of its own in either language — the
    /// screen would show a key in brackets, or two reasons one sentence.
    #[test]
    fn every_tag_and_translation_refusal_has_its_sentence() {
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a declared language");
            let tags: Vec<String> = ronoctua_tags::Why::every()
                .iter()
                .map(|why| tags_refusal(why, &t))
                .collect();
            let words: Vec<String> = ronoctua_player::TranslationWhy::ALL
                .iter()
                .map(|why| {
                    trouble_sentence(
                        &Trouble::Translation(ronoctua_player::TranslationRefusal::new(*why)),
                        &t,
                    )
                })
                .collect();
            for said in tags.iter().chain(&words) {
                assert!(
                    !said.trim().is_empty() && !said.contains('['),
                    "{language}: {said:?}"
                );
            }
            let distinct =
                |all: &[String]| all.iter().collect::<std::collections::HashSet<_>>().len();
            assert_eq!(
                distinct(&tags),
                tags.len(),
                "{language}: two reasons, one sentence"
            );
            assert_eq!(
                distinct(&words),
                words.len(),
                "{language}: two reasons, one sentence"
            );
        }
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

    /// A view with the queue's picker open on `at`, the cursor where it lands.
    fn among(at: &std::path::Path) -> View {
        let mut v = View::blank(None);
        v.pick_at(at, Purpose::Queue);
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

    /// FAILS IF: the pointer lights a cell of a grid that is not the one a
    /// click at that pixel takes — or lights more than one, or the whole row
    /// — or a click where nothing is lit takes something.
    ///
    /// `a-grid-cell-is-lit-alone`: for every pixel across a grid row of the
    /// picker's page, the band `chrome::hover_cells` paints with a door's
    /// `chrome::Across` is the band of the cell the VIEW'S OWN click path
    /// (`View::clicked_px`, what the window and the footer call with the
    /// same `Across`) takes at that pixel.
    #[test]
    fn a_grid_cell_is_lit_alone() {
        let root = a_library("lit-alone");
        for n in 0..9 {
            std::fs::write(root.join(format!("track {n}.flac")), b"").unwrap();
        }
        let t = texts();
        let columns = 90;
        let fresh = || {
            let mut v = View::blank(None);
            v.pick_at(&root, Purpose::Queue);
            v.settle_in(None, columns, &t);
            v
        };
        let taken = |v: &View| v.rows(None, columns, &t)[v.cursor()].target.clone();
        let v = fresh();
        let rows = v.rows(None, columns, &t);
        let cells_of = |r: usize| crate::player_rows::grid_cells(&rows, r);
        let n = (0..rows.len())
            .find(|&r| {
                cells_of(r).len() == 3
                    && rows[r].cells.iter().all(|c| {
                        matches!(
                            c.target,
                            Target::Picker(crate::file_picker::Target::File(_))
                        )
                    })
            })
            .expect("a grid row of three files");
        let across = crate::chrome::Across {
            lead: 2,
            cell_px: 9.0,
        };
        let colour = tokens::color::SURFACE_HOVER;
        let (mut lit, mut dark) = (0, 0);
        for third in 0..(columns + 4) * 3 {
            let x = f32::from(third) * across.cell_px / 3.0;
            let bands = crate::chrome::hover_cells(
                std::iter::once((colour, Some(n), Some(x))),
                cells_of,
                0,
                rows.len(),
                rows.len(),
                0,
                10_000.0,
                across,
            );
            assert!(bands.len() <= 1, "one band at most, at {x}");
            let mut clicked = fresh();
            let before = taken(&clicked);
            let _ = clicked.clicked_px(n, x, across, &rows);
            let after = taken(&clicked);
            match bands.first() {
                Some(b) => {
                    lit += 1;
                    assert!(b.width < 10_000.0, "a cell, never the whole row");
                    assert!(x >= b.x && x < b.x + b.width, "the band holds the pointer");
                    let cell = rows[n]
                        .cells
                        .iter()
                        .find(|c| {
                            let (bx, bw) = across.band_of(c.from, c.width);
                            (bx - b.x).abs() < f32::EPSILON && (bw - b.width).abs() < f32::EPSILON
                        })
                        .expect("the band is a cell's");
                    assert_eq!(after, cell.target, "the click takes the lit cell, at {x}");
                }
                None => {
                    dark += 1;
                    assert_eq!(
                        after, before,
                        "a click where nothing is lit takes nothing, at {x}"
                    );
                }
            }
        }
        assert!(lit > 0 && dark > 0, "{lit} lit, {dark} dark");
        // AND THE SECOND CLICK on the lit cell is its `RET`.
        let (bx, bw) = across.band_of(rows[n].cells[1].from, rows[n].cells[1].width);
        let mut v = fresh();
        assert!(!v.clicked_px(n, bx + bw / 2.0, across, &rows));
        assert!(v.clicked_px(n, bx + bw / 2.0, across, &rows));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A key of the picker, by its notation.
    fn picking(v: &mut View, notation: &str) -> Errand {
        let key = ronoctua_keymap::Key::parse(notation).expect("a key");
        let press = v.press_of_key(key).expect("the picker reads it");
        v.key(press, None, 80, &texts())
    }

    /// Text typed on the picker, a key a letter.
    fn typing(v: &mut View, text: &str) {
        for c in text.chars() {
            let key = ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Char(c));
            assert_eq!(v.press_of_key(key), Some(Press::Pick(key)), "{c} is typed");
            v.key(Press::Pick(key), None, 80, &texts());
        }
    }

    /// The picker's target, as a row of the view carries it.
    fn picked(t: crate::file_picker::Target) -> Target {
        Target::Picker(t)
    }

    fn on(v: &View) -> Target {
        v.rows(None, 80, &texts())[v.cursor()].target.clone()
    }

    /// FAILS IF: the files stop being walked the way a person walks them in
    /// the picker — into a folder, back up onto it, a file added or played
    /// now, a folder queued whole — or `Esc` stops going back a page before it
    /// closes the door.
    #[test]
    fn the_files_are_walked_played_and_queued() {
        use crate::file_picker::Target as P;
        let t = texts();
        let root = a_library("walked");
        let album = root.join("Album");
        let song = album.join("01.ogg").to_string_lossy().into_owned();
        let mut v = among(&root);
        // THE CURSOR LANDS ON THE FIRST ENTRY, the folder — not on `..`.
        assert_eq!(on(&v), picked(P::Folder(album.clone())));
        // `RET` ENTERS IT, and the cursor is on its first file.
        assert_eq!(v.key(Press::Enter, None, 80, &t), Errand::Nothing);
        assert_eq!(on(&v), picked(P::File(album.join("01.ogg"))));
        // THE FOLDER WHOLE, into the queue: ONE location, which the player
        // makes a playlist named after it — and says so, by its name.
        let Errand::TellSaying(ask, said) = picking(&mut v, "C-e") else {
            panic!("the folder was not queued");
        };
        assert_eq!(ask, put(album.to_string_lossy().into_owned(), false));
        assert!(said.contains("Album"), "{said}");
        // `RET` ON A FILE ADDS IT at the end, ONCE (one already there is a
        // question, decision A5) — and the page stays, for the next; `C-RET`
        // PLAYS IT NOW, right after what plays, the queue kept.
        let Errand::TellSaying(ask, _) = v.key(Press::Enter, None, 80, &t) else {
            panic!("RET did not add");
        };
        assert_eq!(
            ask,
            Ask::Put {
                what: vec![Thing::Location(song.clone())],
                after: None,
                into: None,
                play: false,
                replace: false,
                once: true,
            }
        );
        assert!(v.browsing(), "the queue's picker stays for the next");
        let Errand::TellSaying(ask, _) = picking(&mut v, "C-RET") else {
            panic!("C-RET did not play");
        };
        assert_eq!(
            ask,
            Ask::Put {
                what: vec![Thing::Location(song)],
                after: None,
                into: None,
                play: true,
                replace: false,
                once: false,
            },
            "playing now must never start the queue over"
        );
        // SEVERAL MARKED go in together.
        picking(&mut v, "C-SPC");
        picking(&mut v, "C-SPC");
        let Errand::TellSaying(Ask::Put { what, .. }, said) = v.key(Press::Enter, None, 80, &t)
        else {
            panic!("the marked were not taken");
        };
        assert_eq!(what.len(), 2, "{what:?}");
        assert!(said.contains('2'), "{said}");
        // `DEL` GOES UP, and lands on the folder it came from.
        picking(&mut v, "DEL");
        assert_eq!(on(&v), picked(P::Folder(album)));
        // `Esc` GOES BACK ONE LEVEL, and only the second one closes.
        assert_eq!(v.key(Press::Escape, None, 80, &t), Errand::Nothing);
        assert!(!v.browsing());
        assert_eq!(v.key(Press::Escape, None, 80, &t), Errand::Close);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: every letter is not the picker's while its page is up — `s`
    /// stopped the player while a name was typed — or typing does not narrow
    /// the folder, or a path typed does not walk to where it names.
    #[test]
    fn a_letter_is_typed_into_the_picker() {
        use crate::file_picker::Target as P;
        let root = a_library("letters");
        let mut v = among(&root);
        typing(&mut v, "one");
        assert_eq!(on(&v), picked(P::File(root.join("one.flac"))));
        picking(&mut v, "ESC");
        assert!(
            v.browsing(),
            "Esc clears what was typed before it goes back"
        );
        typing(&mut v, &format!("{}/Album/02", root.display()));
        assert_eq!(on(&v), picked(P::File(root.join("Album/02.ogg"))));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: the entries stop being laid out in columns where there is
    /// room, or `..` stops being the first of them, or `→` stops crossing.
    #[test]
    fn the_entries_are_a_grid_with_the_way_back_first() {
        use crate::file_picker::Target as P;
        let t = texts();
        let root = a_library("grid");
        let mut v = among(&root);
        let rows = v.rows(None, 200, &t);
        let first = rows
            .iter()
            .find(|r| r.cells.iter().any(|c| c.target == picked(P::Up)))
            .unwrap();
        assert_eq!(first.cells[0].target, picked(P::Up));
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
        use crate::file_picker::Target as P;
        let t = texts();
        let root = a_library("open");
        let mut v = View::blank(Some(root.clone()));
        v.key(Press::Browse, None, 80, &t);
        assert_eq!(on(&v), picked(P::Folder(root.join("Album"))));
        // AND `Esc` LANDS BACK ON THE ROW THAT OPENED THEM.
        v.key(Press::Escape, None, 80, &t);
        assert_eq!(on(&v), Target::Go(PageId::Pick(Purpose::Queue)));
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
        assert_eq!(
            picking(&mut v, "C-p"),
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
            Target::Go(PageId::Pick(Purpose::Queue))
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

    /// FAILS IF: two pages share a key, or a page's key does not name it back
    /// — `photograph --open` would open the wrong page, or none.
    #[test]
    fn every_page_is_named_by_its_own_key() {
        let mut keys: Vec<&str> = PageId::PAGES.iter().map(|p| p.key()).collect();
        for page in PageId::PAGES {
            assert_eq!(PageId::by_key(page.key()), Some(page), "{page:?}");
        }
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), PageId::PAGES.len(), "two pages share a key");
        assert_eq!(PageId::by_key("player-page-nothing"), None);
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
        // AND A CATALOGUE OF ITS OWN: a listing is reached from a row of
        // channels by theme (`crate::player_catalog`).
        let catalogue =
            std::env::temp_dir().join(format!("ronoctua-pages-catalog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&catalogue);
        crate::player_catalog::write_fixture(&catalogue).expect("a catalogue");
        let mut reached = vec![PageId::Home];
        let mut queue = vec![PageId::Home];
        while let Some(page) = queue.pop() {
            // A QUEUE, which the row that saves it needs — LONG: the home
            // shows a window of it, and the row that opens the whole.
            let mut v = View::blank(Some(nowhere.clone()));
            v.outline_arrived(Some(outline(
                (1..=12)
                    .map(|i| Name::Titled(format!("Track {i}")))
                    .collect(),
            )));
            v.catalogue_at(catalogue.clone());
            v.catalog.source = Some("archive".into());
            // AND A RANKING HANDED ITS VIDEOS: a video's own page is reached
            // from the `⋯` of its row (`crate::player_ranking`).
            v.ranking
                .given(vec![crate::player_ranking::tests::video(1)], 1);
            v.ranking.chosen = Some(crate::player_ranking::tests::video(1).id);
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
            // AND THE WORLD'S RADIO AND TV, their countries listed — and
            // allowed: a list kept from before a permission was taken back is
            // not shown — a country's page is reached from its row.
            for d in ronoctua_player::find::Directory::ALL {
                v.prefs.let_directory(d);
            }
            v.found.given(crate::player_globe::tests::countries_desk());
            v.found
                .given(crate::player_globe::tests::tv_countries_desk());
            if page != PageId::Home {
                v.push(page, None);
            }
            v.settle_in(Some(&s), 80, &t);
            for r in v.rows(Some(&s), 80, &t) {
                // A VIDEO'S OWN PAGE, by the `⋯` CELL at the end of a video's
                // row — a control of the row, not its `RET`, which queues it.
                if r.cells
                    .iter()
                    .any(|c| matches!(c.target, Target::RankedMore(_)))
                    && !reached.contains(&PageId::RankedVideo)
                {
                    reached.push(PageId::RankedVideo);
                    queue.push(PageId::RankedVideo);
                }
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
                    // A SAVE'S PICKER, by `RET` on the row that saves —
                    // whose label says what is saved.
                    Target::SaveSet => PageId::Pick(Purpose::SaveSet),
                    Target::SaveQueue => PageId::Pick(Purpose::SaveQueue),
                    // A LISTING OF THE CATALOGUE, by `RET` on a theme's or a
                    // category's row — whose label is its name.
                    Target::CatalogScope(..) => PageId::Catalog,
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
            // SAVING AGAIN ACTS ON NO ROW but may ask on them
            // (`Unseen::OpensThere`).
            let everywhere = at.of_no_row()
                && !matches!(*id, "player-up" | "player-down" | "player-save-queue-again");
            assert_eq!(press.unseen() == Unseen::Acts, everywhere, "{notation}");
        }
        for press in [
            Press::Tracks,
            Press::Browse,
            Press::Clear,
            Press::SaveQueueAgain,
            Press::Go(PageId::Machines),
        ] {
            assert_eq!(press.unseen(), Unseen::OpensThere, "{press:?}");
        }
        for press in [Press::Enter, Press::Remove, Press::Less, Press::Down] {
            assert_eq!(press.unseen(), Unseen::ShowsTheRows, "{press:?}");
        }
        assert_eq!(Press::Escape.unseen(), Unseen::Acts);
    }

    /// FAILS IF: a question the view took stops taking the order's sentence
    /// off the door's line ("added…" while it asks whether to add it again)
    /// — in any door, for all three read this one answer — or a refusal no
    /// view took stops being said, or a taken ask starts saying something.
    #[test]
    fn a_question_asked_takes_back_what_the_order_said() {
        use ronoctua_player::queue::{EntryId, Name};
        let t = texts();
        let asks = Told::Asks {
            again: Box::new(Ask::SaveQueueAgain { force: true }),
            trouble: Trouble::AlreadyQueued {
                entry: EntryId(2),
                name: Name::Titled("Two".into()),
            },
        };
        assert_eq!(heard(true, &asks, false, &t), Heard::Asked);
        assert!(
            matches!(heard(false, &asks, false, &t), Heard::Said(Some(said)) if said.contains("Two"))
        );
        assert_eq!(heard(true, &Told::Taken, false, &t), Heard::Said(None));
        assert_eq!(
            heard(false, &Told::NotRunning, false, &t),
            Heard::Said(Some(t.t(ronoctua_i18n::key::PLAYER_NOT_RUNNING)))
        );
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

    /// FAILS IF: a click in the empty room under the last row chooses the
    /// last row (the line past the list was held to it), or a second click
    /// there does what `RET` does on it.
    #[test]
    fn a_click_under_the_last_row_takes_nothing() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let s = playing();
        let mut v = View::blank(None);
        let rows = v.rows(Some(&s), 80, &t);
        let first = v.cursor();
        assert_eq!(Row::owner(&rows, rows.len() + 3), rows.len() + 3);
        for past in [rows.len(), rows.len() + 4] {
            assert!(!v.clicked_at(past, 2, &rows), "nothing to act on");
            assert!(!v.clicked_at(past, 2, &rows), "not even the second time");
            assert_eq!(v.cursor(), first, "the cursor did not come down");
        }
        // THE LAST ROW ITSELF is still a row.
        let last = (0..rows.len())
            .rev()
            .find(|&n| rows[n].stops())
            .expect("a row");
        v.clicked_at(last, 2, &rows);
        assert_eq!(v.cursor(), last);
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
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let names: Vec<String> = three.iter().map(|x| track_name(x, &kin, &t)).collect();
        let unique: std::collections::BTreeSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), 3, "{names:?}");
        assert_eq!(names[0], "English · dts");
        let one = english(1, "dts");
        assert_eq!(track_name(&one, &[&one], &t), "English");
    }

    /// FAILS IF: something typed on the leader's question to play together
    /// plays and leaves the picker open, where a pick closes it and goes back
    /// to the page that asked.
    #[test]
    fn what_is_typed_to_play_together_closes_the_picker_as_a_pick_does() {
        let t = texts();
        let s = playing();
        let root = a_library("together");
        let mut v = View::blank(None);
        v.key(Press::Go(PageId::Together), Some(&s), 80, &t);
        v.pick_at(&root, Purpose::Lane);
        assert_eq!(v.page(), PageId::Pick(Purpose::Lane));
        let mut pairing = crate::player_machines::Pairing::default();
        let errand = crate::player_question::answered_saying(
            crate::player_question::TOGETHER,
            "/v/b.mkv",
            &mut pairing,
            &t,
        );
        v.answer_came(&errand, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Together);
        // NEVER MIND keeps the picker open.
        v.pick_at(&root, Purpose::Lane);
        v.answer_came(&Errand::Nothing, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Pick(Purpose::Lane));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: an address typed in the picker stops being an answer where
    /// the player plays one — into the queue, and together, which closes the
    /// page — or the leader's questions stop giving the same orders.
    #[test]
    fn an_address_typed_is_an_answer() {
        use crate::player_machines::Pairing;
        use crate::player_question::{ENQUEUE, answered};
        let t = texts();
        let s = playing();
        let root = a_library("typed");
        let mut v = among(&root);
        typing(&mut v, "https://example.org/a");
        let Errand::TellSaying(ask, _) = v.key(Press::Enter, None, 80, &t) else {
            panic!("the address was not taken");
        };
        assert_eq!(ask, put("https://example.org/a".into(), false));
        assert!(v.browsing(), "the queue's picker stays");
        let mut pairing = Pairing::default();
        assert_eq!(
            answered(ENQUEUE, "https://example.org/a", &mut pairing),
            Errand::Tell(put("https://example.org/a".into(), false))
        );
        assert_eq!(answered(ENQUEUE, "   ", &mut pairing), Errand::Nothing);
        // TOGETHER: the address is a lane, and the page closes.
        v.back(None, 80, &t);
        v.key(Press::Go(PageId::Together), Some(&s), 80, &t);
        v.pick_at(&root, Purpose::Lane);
        typing(&mut v, "https://example.org/b.mkv");
        let Errand::TellSaying(ask, _) = v.key(Press::Enter, Some(&s), 80, &t) else {
            panic!("the address was not played together");
        };
        assert_eq!(
            ask,
            crate::player_question::together("https://example.org/b.mkv".into())
        );
        assert_eq!(v.page(), PageId::Together);
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
        // `C-s` IS A KEY OF ITS OWN — the queue saved again — and never the
        // `s` it is written with.
        assert_eq!(
            press_of_key(chord),
            Some(Press::SaveQueueAgain),
            "C-s was read as s"
        );
        assert_eq!(press_of_key(Key::simple(Sym::Esc)), Some(Press::Escape));
    }

    /// FAILS IF: of two entries that share a key, the one whose page this
    /// is stops being the one read — `O` on a world page sorting something
    /// else once another page takes `O` too — or a key the page does not
    /// decide stops being read by its first entry.
    #[test]
    fn a_shared_key_is_the_one_of_the_page() {
        let table = [
            ("O", "player-rewind", Where::WithAPosition),
            ("O", "player-countries-order", Where::OnTheCountries),
            ("O", "player-forward", Where::OnTheGlobe),
        ];
        let on_the_countries = |w: Where| match w {
            Where::OnTheCountries => Some(true),
            Where::OnTheGlobe => Some(false),
            _ => None,
        };
        assert_eq!(
            pressed_in(&table, "O", on_the_countries),
            Some(Press::CountriesOrder)
        );
        let elsewhere = |w: Where| match w {
            Where::OnTheCountries | Where::OnTheGlobe => Some(false),
            _ => None,
        };
        assert_eq!(pressed_in(&table, "O", elsewhere), Some(Press::Rewind));
        assert_eq!(pressed_in(&table, "O", |_| None), Some(Press::Rewind));
        assert_eq!(pressed_in(&table, "x", |_| None), None);
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
    /// picker — a folder and a name typed there, holding what plays' name —
    /// or writes over a file that is there without a question
    /// born on its "no"; or the file stops opening again from its page: a
    /// set plays after what plays, the queue kept, and a queue takes the
    /// queue's place only after a question of its own.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn saving_and_opening_go_through_the_picker() {
        use crate::file_picker::Target as P;
        use ronoctua_i18n::key;
        let t = texts();
        let s = playing();
        let root = a_library("saved");
        let music = root.join("Music");
        std::fs::create_dir_all(&music).unwrap();
        let mut v = queued(View::blank(Some(root.clone())));
        // THE ROW: the save's picker, on the music folder, holding what
        // plays' own name.
        chosen(&mut v, Some(&s), &Target::SaveSet);
        assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), Errand::Nothing);
        assert_eq!(v.page(), PageId::Pick(Purpose::SaveSet));
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(rows[v.cursor()].target, picked(P::Name), "born on the name");
        assert!(
            rows[v.cursor()]
                .fields
                .iter()
                .any(|f| f.text.contains("one.flac.rset")),
            "{:?}",
            rows[v.cursor()].fields
        );
        // ANOTHER NAME, typed: every letter is the name's.
        for _ in 0.."one.flac".len() {
            picking(&mut v, "DEL");
        }
        typing(&mut v, "Film night");
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
        assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), saving);
        assert_eq!(v.page(), PageId::Home, "the picker closes on the save");
        // A FILE ALREADY THERE: a question, born on its "no" — and "yes"
        // writes over it.
        std::fs::write(&to, b"{}").unwrap();
        v.key(Press::SaveSet, Some(&s), 80, &t);
        for _ in 0.."one.flac".len() {
            picking(&mut v, "DEL");
        }
        typing(&mut v, "Film night");
        assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), Errand::Nothing);
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(
            rows[v.cursor()].target,
            picked(P::Answer(false)),
            "born on its no"
        );
        v.key(Press::Down, Some(&s), 80, &t);
        assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), saving);
        // A NAME NO FILE CAN HAVE: said.
        v.key(Press::SaveSet, Some(&s), 80, &t);
        for _ in 0.."one.flac".len() {
            picking(&mut v, "DEL");
        }
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Said(_)
        ));
        picking(&mut v, "ESC");
        // THE QUEUE: no name to start from, and its own extension.
        chosen(&mut v, Some(&s), &Target::SaveQueue);
        v.key(Press::Enter, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Pick(Purpose::SaveQueue));
        typing(&mut v, "Week");
        let week = music.join("Week.rqueue");
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::TellSaying(Ask::SaveQueue { ref to, .. }, _)
                if *to == week.to_string_lossy()
        ));
        std::fs::write(&week, b"{}").unwrap();
        // OPENED AGAIN, from the page that lists them: the set plays now,
        // after what plays, and the page closes.
        chosen(&mut v, Some(&s), &Target::Go(PageId::Pick(Purpose::Open)));
        v.key(Press::Enter, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Pick(Purpose::Open));
        let listed: Vec<Target> = v
            .rows(Some(&s), 80, &t)
            .iter()
            .flat_map(|r| r.cells.iter().map(|c| c.target.clone()))
            .filter(|t| matches!(t, Target::Picker(P::File(_))))
            .collect();
        assert_eq!(
            listed,
            [picked(P::File(to.clone())), picked(P::File(week.clone()))],
            "the page lists the player's own files, and nothing else"
        );
        chosen(&mut v, Some(&s), &picked(P::File(to.clone())));
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::TellSaying(
                put(to.to_string_lossy().into_owned(), true),
                t.t_with(key::PLAYER_PLAYING_NOW, &[("name", "Film night.rset")])
            )
        );
        assert_eq!(v.page(), PageId::Home, "the page closes on the choice");
        // THE QUEUE: its own question first, born on its "no".
        v.key(Press::Go(PageId::Pick(Purpose::Open)), Some(&s), 80, &t);
        chosen(&mut v, Some(&s), &picked(P::File(week.clone())));
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
                    once: false,
                },
                t.t_with(key::PLAYER_QUEUE_OPENED, &[("name", "Week.rqueue")])
            )
        );
        // A NAME TYPED FROM NO QUESTION (`LEADER :` with an argument) opens
        // the save's picker holding it.
        let mut pairing = crate::player_machines::Pairing::default();
        v.answered(
            crate::player_question::SAVE_SET,
            "Sunday.rset",
            &mut pairing,
            Some(&s),
            80,
            &t,
        );
        assert_eq!(v.page(), PageId::Pick(Purpose::SaveSet));
        let rows = v.rows(Some(&s), 80, &t);
        assert!(
            rows.iter()
                .any(|r| r.fields.iter().any(|f| f.text == "Sunday.rset"))
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
