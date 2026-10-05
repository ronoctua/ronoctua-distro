//! `LEADER v` — video-metrics: the person's YouTube channels, their videos
//! scored, and the rules that score them. One view, two doors: the footer's
//! region (`v v`) and a window (`v w`, `crate::video_metrics_window`).
//!
//! # What it holds, and what it does not
//!
//! The page on screen and the way back to the ones before it, the cursor by
//! IDENTITY (a spot, never an index — a list the scanner reorders under it
//! keeps the cursor on the same video), the list's filters and layout, a
//! question waiting for its "no" or its "yes", and the text of the person's
//! second description while it is edited. It holds NO READING: what the
//! catalogue says lives in [`Readings`], which each door keeps and fills from
//! its own feed (`crate::video_metrics_feed`), and hands in whenever rows are
//! asked for — so the two doors draw the same catalogue without a second
//! copy going stale.
//!
//! # Nothing here blocks, and nothing here writes
//!
//! Every read and every write of the catalogue is an [`Errand`] the door
//! hands its feed — a thread (`crate::video_metrics_feed`), never the loop.
//! What came of it arrives as a [`Done`], and [`View::took`] turns it into the
//! sentence the door says.
//!
//! # The pages
//!
//! Home (the scan, the catalogue, what was left out), the channels, one
//! channel (its facts, its score, every rule in its channel version, removing
//! it), the global rules (recency, the Short penalty, blocking Shorts, the
//! length bands, the preferred band, words), one rule, the rating weights,
//! the videos (by score, as a list or a grid of thumbnails, with filters and
//! pages) and one video (every field, the person's rating, tags and second
//! description, and what can be done with it).
//!
//! # A question is a row
//!
//! A deletion, removing a channel or a rule, hiding a video and adding one
//! the queue already has are asked ON THE PAGE, under the row the person
//! asked from — where they are looking — with the "no" first and the cursor
//! born on it; nothing floats (invariant 14). A question the view puts there
//! on its own (the queue answered "already there") is followed by the door
//! like a key: it is the consequence of the person's key. A value that is typed
//! (a channel's address, a score, a band, words) is the door's line
//! (`crate::line::Line`), where `C-v` pastes: the minibuffer in the footer,
//! the header in the window.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use ronoctua_i18n::{Arg, key};
use ronoctua_tokens as tokens;
use ronoctua_video_metrics::read::{
    Channel, Filter, Hidden, RankedVideo, ScanRecord, Shorts, Video,
};
use ronoctua_video_metrics::{
    ChannelId, Fields, Rating, Rule, RuleId, RuleKind, Settings, Skip, TermMatch, VideoId, Weights,
    Why,
};

use crate::chrome::Kind;
use crate::files::{Field, Hit as FilesHit, Ink};
use crate::video_metrics_words as words;

/// The keys this view answers to, and where each one works. The hint line is
/// GENERATED from it (`chrome::hint_of_marked`), so a key that moves here
/// moves on screen, and a key is taught only where it does something.
///
/// `+` and `-` are the design's keys for a score; they are the arrows' twins
/// on every value, and the hint teaches the arrows (a line shows an id once).
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "video-metrics-down", Where::Anywhere),
    ("<up>", "video-metrics-up", Where::Anywhere),
    ("RET", "video-metrics-enter", Where::Anywhere),
    ("<left>", "video-metrics-less", Where::OnAValue),
    ("<right>", "video-metrics-more", Where::OnAValue),
    ("-", "video-metrics-less", Where::OnAValue),
    ("+", "video-metrics-more", Where::OnAValue),
    ("<prior>", "video-metrics-page-back", Where::OnTheVideos),
    ("<next>", "video-metrics-page-on", Where::OnTheVideos),
    ("L", "video-metrics-layout", Where::OnTheVideos),
    ("/", "video-metrics-search", Where::OnTheVideos),
    ("a", "video-metrics-player", Where::OnAVideo),
    ("K", "video-metrics-kiosk", Where::OnAVideo),
    ("D", "video-metrics-download", Where::OnAVideo),
    ("h", "video-metrics-hide", Where::OnAVideo),
    ("d", "video-metrics-delete", Where::OnAVideo),
];

/// Where a key of [`OF_THE_VIEW`] means something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// On a row whose value turns — a number, a ring, a cell of the grid.
    OnAValue,
    /// On the videos' page.
    OnTheVideos,
    /// On a video: its page, or its row or cell on the videos' page.
    OnAVideo,
}

/// The command that opens the view in the footer's region.
pub const OPEN_COMMAND: &str = "video-metrics-open";
/// The command that opens it in a window.
pub const WINDOW_COMMAND: &str = "video-metrics-window";
/// The command that asks for a scan now.
pub const SCAN_COMMAND: &str = "video-metrics-scan";

/// A key, already reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row, or one shelf of the grid.
    Up,
    /// Down one row, or one shelf.
    Down,
    /// The value under the cursor down a step, a ring back — or the cell
    /// before, on the grid.
    Less,
    /// The value up a step, a ring on — or the cell after.
    More,
    /// Act on the row.
    Enter,
    /// One level back: a question let go, then the page before.
    Escape,
    /// The page of videos before.
    PageBack,
    /// The page of videos after.
    PageOn,
    /// A list, or a grid of thumbnails.
    Layout,
    /// Search the videos' words.
    Search,
    /// The video in the player.
    Player,
    /// The video in Chrome, as a kiosk.
    Kiosk,
    /// The video downloaded, by the transfers' queue.
    Download,
    /// The video hidden, or shown again.
    Hide,
    /// The video deleted for good.
    Delete,
}

/// A command of [`OF_THE_VIEW`] as the view's press — a click on the hint, or
/// the id run by name.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "video-metrics-up" => Press::Up,
        "video-metrics-down" => Press::Down,
        "video-metrics-less" => Press::Less,
        "video-metrics-more" => Press::More,
        "video-metrics-enter" => Press::Enter,
        "video-metrics-page-back" => Press::PageBack,
        "video-metrics-page-on" => Press::PageOn,
        "video-metrics-layout" => Press::Layout,
        "video-metrics-search" => Press::Search,
        "video-metrics-player" => Press::Player,
        "video-metrics-kiosk" => Press::Kiosk,
        "video-metrics-download" => Press::Download,
        "video-metrics-hide" => Press::Hide,
        "video-metrics-delete" => Press::Delete,
        _ => return None,
    })
}

/// A key, by its WHOLE notation, as the view's press — the one reader both
/// doors call. `Esc` and `C-g` are the way back whatever is held.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    use ronoctua_keymap::Sym;
    if key.sym == Sym::Esc || (key.mods.ctrl && key.sym == Sym::Char('g')) {
        return Some(Press::Escape);
    }
    let notation = key.notation();
    let id = OF_THE_VIEW
        .iter()
        .find(|(taught, _, _)| *taught == notation)
        .map(|(_, id, _)| *id)?;
    press_of(id)
}

/// Every command this view's keys and rows reach — for `leader::reachable`,
/// which is what says a command has a key.
pub fn reached() -> impl Iterator<Item = &'static str> {
    OF_THE_VIEW
        .iter()
        .map(|(_, id, _)| *id)
        .chain(Ask::ALL.iter().map(|a| a.command()))
}

/// A value the person TYPES, on the door's line — each one a command of the
/// registry (`<id>`), with its question (`ask-<id>`): the line answers under
/// the command's id, and the footer's minibuffer runs it with the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// A channel's address, `@handle` or id.
    AddChannel,
    /// A channel's score, 0–10000.
    ChannelScore,
    /// A rule's points.
    Points,
    /// Over how many days recency fades.
    Days,
    /// A length band.
    Band,
    /// A rule's words.
    Words,
    /// The words the videos are searched for.
    Search,
    /// The person's own tags of a video.
    SecondTags,
    /// A rating's weight.
    Weight,
}

impl Ask {
    /// Every one.
    pub const ALL: [Self; 9] = [
        Self::AddChannel,
        Self::ChannelScore,
        Self::Points,
        Self::Days,
        Self::Band,
        Self::Words,
        Self::Search,
        Self::SecondTags,
        Self::Weight,
    ];

    /// Its command — the id the line answers under.
    #[must_use]
    pub const fn command(self) -> &'static str {
        match self {
            Self::AddChannel => "video-metrics-add-channel",
            Self::ChannelScore => "video-metrics-channel-score",
            Self::Points => "video-metrics-points",
            Self::Days => "video-metrics-days",
            Self::Band => "video-metrics-band",
            Self::Words => "video-metrics-words",
            Self::Search => "video-metrics-search",
            Self::SecondTags => "video-metrics-second-tags",
            Self::Weight => "video-metrics-weight",
        }
    }

    /// The question a command's id asks; `None` for any other id.
    #[must_use]
    pub fn of_command(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.command() == id)
    }
}

/// A scope of rules: everywhere, or one channel's.
pub type Scope = Option<ChannelId>;

/// A rule of which there is one per scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Single {
    /// Recency: points today, fading over days.
    Recency,
    /// The Short penalty.
    Short,
    /// The preferred length band.
    Preferred,
    /// Being live or a premiere.
    Live,
}

impl Single {
    /// The four, in the order a page lists them.
    pub const ALL: [Self; 4] = [Self::Recency, Self::Short, Self::Preferred, Self::Live];

    /// Its kebab name, for a spot.
    const fn id(self) -> &'static str {
        match self {
            Self::Recency => "recency",
            Self::Short => "short",
            Self::Preferred => "preferred",
            Self::Live => "live",
        }
    }

    /// Whether a rule is of this kind.
    const fn is(self, kind: &RuleKind) -> bool {
        matches!(
            (self, kind),
            (Self::Recency, RuleKind::Recency { .. })
                | (Self::Short, RuleKind::Short)
                | (Self::Preferred, RuleKind::Preferred { .. })
                | (Self::Live, RuleKind::Live)
        )
    }

    /// The rule a scope starts from when it has none of this kind: no
    /// points, so it changes nothing until the person gives it some.
    fn fresh(self, scope: &Scope) -> Rule {
        Rule {
            id: None,
            channel: scope.clone(),
            kind: match self {
                Self::Recency => RuleKind::Recency { days: RECENCY_DAYS },
                Self::Short => RuleKind::Short,
                Self::Preferred => RuleKind::Preferred {
                    min_s: PREFERRED.0,
                    max_s: PREFERRED.1,
                },
                Self::Live => RuleKind::Live,
            },
            points: 0,
            enabled: true,
        }
    }
}

/// Over how many days a fresh recency rule fades.
const RECENCY_DAYS: u32 = 30;
/// A fresh preferred band: four to twenty minutes.
const PREFERRED: (u32, u32) = (240, 1200);
/// A new length band, before the person says which.
const BAND: (u32, u32) = (0, 240);

/// Which rule a rule's page is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The one of this kind in force for the scope.
    Single(Single),
    /// A band or words, by id.
    Id(RuleId),
}

/// A page of the view.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Page {
    /// The scan, the catalogue, what was left out.
    Home,
    /// Every channel.
    Channels,
    /// One channel.
    Channel(ChannelId),
    /// The rules for everywhere.
    Rules,
    /// One rule, in a scope.
    Rule(Scope, Slot),
    /// The rating weights.
    Weights,
    /// The videos, by score.
    Videos,
    /// One video.
    Video(VideoId),
}

/// How the videos are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// One row a video.
    #[default]
    List,
    /// A grid of thumbnails.
    Grid,
}

/// How far back the videos' list reaches, by when each was published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Since {
    /// Any time.
    #[default]
    Any,
    /// The last day.
    Day,
    /// The last week.
    Week,
    /// The last thirty days.
    Month,
    /// The last year.
    Year,
}

impl Since {
    /// The five, in the ring's order.
    pub const ALL: [Self; 5] = [Self::Any, Self::Day, Self::Week, Self::Month, Self::Year];

    /// How many seconds back it reaches; `None` for any time.
    const fn seconds(self) -> Option<i64> {
        match self {
            Self::Any => None,
            Self::Day => Some(86_400),
            Self::Week => Some(7 * 86_400),
            Self::Month => Some(30 * 86_400),
            Self::Year => Some(365 * 86_400),
        }
    }

    /// Its words.
    const fn key(self) -> &'static str {
        match self {
            Self::Any => key::VIDEO_METRICS_SINCE_ANY,
            Self::Day => key::VIDEO_METRICS_SINCE_DAY,
            Self::Week => key::VIDEO_METRICS_SINCE_WEEK,
            Self::Month => key::VIDEO_METRICS_SINCE_MONTH,
            Self::Year => key::VIDEO_METRICS_SINCE_YEAR,
        }
    }
}

/// Now, in seconds since the epoch — the catalogue's unit (a signed one:
/// SQLite's `unixepoch`), which every age and date of the view counts from.
#[must_use]
pub fn now_s() -> i64 {
    i64::try_from(crate::software_feed::seconds_now()).unwrap_or(i64::MAX)
}

/// How many videos a page of the list shows.
pub const PER_PAGE: u32 = 24;

/// How much a step of `←`/`→` moves a score, a weight or a rule's points.
pub const POINTS_STEP: i32 = 100;

/// Which page of which list the door is asked to read: its filter, its page
/// and whether the thumbnails are decoded with it (the grid's).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Query {
    /// What the list shows.
    pub filter: Filter,
    /// Which page, from 0.
    pub page: u32,
    /// Whether the thumbnails come too.
    pub pictures: bool,
}

/// What the door's feed is asked to read: everything the page on screen
/// draws. The feed reads it again, as it is, whenever the catalogue moves.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Look {
    /// The page.
    pub page: Page,
    /// The list's query, on the videos' page.
    pub query: Option<Query>,
}

/// A thumbnail, decoded.
#[derive(Debug, Clone)]
pub struct Picture {
    /// The pixels.
    pub rgba: Arc<ronoctua_render::image::Rgba>,
    /// Its size, in pixels.
    pub size: (u32, u32),
}

/// A page of the videos' list, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// What was asked.
    pub query: Query,
    /// Its rows, best first.
    pub rows: Vec<RankedVideo>,
    /// How many the filter keeps, on every page.
    pub total: u64,
}

/// One reading of the catalogue, for one look: each field filled only when
/// the look asked for it.
#[derive(Debug, Clone, Default)]
pub struct Reading {
    /// Whether the catalogue answered, and why not.
    pub catalogue: Option<Result<(), Why>>,
    /// The last scan, when there was one.
    pub last_scan: Option<Option<ScanRecord>>,
    /// Whether a scan holds the catalogue now — what tells a scan running
    /// from one killed before it wrote its end (`Catalog::scanning`).
    pub scanning: Option<bool>,
    /// How many videos are catalogued.
    pub videos_count: Option<u64>,
    /// The tombstones, by reason.
    pub skipped: Option<Vec<(Skip, u64)>>,
    /// Whether the hosts a scan asks are let.
    pub consent: Option<bool>,
    /// Every channel.
    pub channels: Option<Vec<Channel>>,
    /// The rules in force in a scope.
    pub rules: Option<(Scope, Vec<Rule>)>,
    /// The switches.
    pub settings: Option<Settings>,
    /// The rating weights.
    pub weights: Option<Weights>,
    /// A page of the list.
    pub listed: Option<Listed>,
    /// One video whole, or none by that id.
    pub video: Option<(VideoId, Option<Video>)>,
    /// The thumbnails the look drew, by their file.
    pub pictures: Option<HashMap<PathBuf, Picture>>,
}

/// Everything the catalogue said, latest of each — what both doors draw.
#[derive(Debug, Clone, Default)]
pub struct Readings {
    /// Whether the catalogue answered, and why not; `None` before it did.
    pub catalogue: Option<Result<(), Why>>,
    /// The last scan (`Some(None)`: there never was one).
    pub last_scan: Option<Option<ScanRecord>>,
    /// Whether a scan holds the catalogue now.
    pub scanning: Option<bool>,
    /// How many videos are catalogued.
    pub videos_count: Option<u64>,
    /// The tombstones, by reason.
    pub skipped: Vec<(Skip, u64)>,
    /// Whether YouTube's hosts are let.
    pub consent: Option<bool>,
    /// Every channel, by score.
    pub channels: Vec<Channel>,
    /// The rules in force in the scope last read.
    pub rules: Option<(Scope, Vec<Rule>)>,
    /// The switches.
    pub settings: Option<Settings>,
    /// The rating weights.
    pub weights: Option<Weights>,
    /// The page of the list last read.
    pub listed: Option<Listed>,
    /// The video last read.
    pub video: Option<(VideoId, Option<Video>)>,
    /// The thumbnails of what is on screen.
    pub pictures: HashMap<PathBuf, Picture>,
    /// Now, seconds since the epoch — ages are counted from it.
    pub now_s: i64,
    /// The machine's zone, for dates.
    pub zone: ronoctua_files::when::Zone,
    /// How the transfers' queue is opened from the leader, taught by the
    /// sentence a download says (`leader::taught_keys`, filled by the door).
    pub transfers_keys: String,
}

impl Readings {
    /// A reading merged in: what it read replaces what was there.
    pub fn took(&mut self, r: Reading) {
        if let Some(c) = r.catalogue {
            self.catalogue = Some(c);
        }
        if let Some(s) = r.last_scan {
            self.last_scan = Some(s);
        }
        if let Some(s) = r.scanning {
            self.scanning = Some(s);
        }
        if let Some(n) = r.videos_count {
            self.videos_count = Some(n);
        }
        if let Some(s) = r.skipped {
            self.skipped = s;
        }
        if let Some(c) = r.consent {
            self.consent = Some(c);
        }
        if let Some(c) = r.channels {
            self.channels = c;
        }
        if let Some(rules) = r.rules {
            self.rules = Some(rules);
        }
        if let Some(s) = r.settings {
            self.settings = Some(s);
        }
        if let Some(w) = r.weights {
            self.weights = Some(w);
        }
        if let Some(l) = r.listed {
            self.listed = Some(l);
        }
        if let Some(v) = r.video {
            self.video = Some(v);
        }
        if let Some(p) = r.pictures {
            self.pictures = p;
        }
    }

    /// A channel, by id.
    fn channel(&self, id: &ChannelId) -> Option<&Channel> {
        self.channels.iter().find(|c| &c.id == id)
    }

    /// The rules in force in `scope`, when they were read for it.
    fn rules_of(&self, scope: &Scope) -> Option<&[Rule]> {
        self.rules
            .as_ref()
            .filter(|(s, _)| s == scope)
            .map(|(_, r)| r.as_slice())
    }

    /// The video by `id`, when it was read.
    fn video_of(&self, id: &VideoId) -> Option<&Video> {
        self.video
            .as_ref()
            .filter(|(v, _)| v == id)
            .and_then(|(_, v)| v.as_ref())
    }

    /// A video of the list, by id.
    fn ranked(&self, id: &VideoId) -> Option<&RankedVideo> {
        self.listed.as_ref()?.rows.iter().find(|r| &r.id == id)
    }
}

/// A write the view asks the door's feed to make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Add the channel an address, `@handle` or id names.
    AddChannel(String),
    /// A channel's score.
    ChannelScore(ChannelId, i32),
    /// A channel's own "block Shorts" (`None`: as everywhere).
    ChannelShorts(ChannelId, Option<bool>),
    /// Remove a channel and its videos.
    RemoveChannel(ChannelId),
    /// Keep a rule (a new one when its id is `None`).
    PutRule(Rule),
    /// Remove a rule.
    RemoveRule(RuleId),
    /// The rating weights.
    Weights(Weights),
    /// The switches.
    Settings(Settings),
    /// A video's rating.
    Rating(VideoId, Option<Rating>),
    /// A video's second description.
    SecondDescription(VideoId, String),
    /// A video's second tags.
    SecondTags(VideoId, Vec<String>),
    /// Hide a video, or show it again.
    Hidden(VideoId, bool),
    /// Delete a video for good.
    Delete(VideoId),
    /// Let YouTube's hosts be asked, or not.
    Consent(bool),
}

/// Which way the player is asked to take a video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToPlay {
    /// Its address.
    pub url: String,
    /// Its title — the leaf's name from the start.
    pub title: String,
    /// Only when the queue does not have it yet.
    pub once: bool,
}

/// What the view wants done, after a key, a click, a paste or an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing more; redraw.
    Nothing,
    /// Close the view.
    Close,
    /// Read what the page draws.
    Look(Look),
    /// Write this, then read the page again.
    Edit(Edit),
    /// Ask a value on the door's line, starting from `initial`.
    Ask {
        /// What.
        ask: Ask,
        /// What the line starts with.
        initial: Option<String>,
    },
    /// Read the clipboard's text for the description being edited.
    Paste,
    /// Put a value on the clipboard, and say so.
    Copy(String),
    /// A scan, now — through the brain, which marks the modeline while it
    /// runs.
    Scan,
    /// The video in the player.
    Play(ToPlay),
    /// The video in Chrome, as a kiosk (`kiosk-open`, the footer's).
    Kiosk(String),
    /// The video downloaded, by the transfers' queue.
    Download {
        /// Its address.
        url: String,
        /// Its title, for the sentence.
        title: String,
    },
    /// The transfers' queue opened — `LEADER t t`, the footer's — where a
    /// download that was just handed over is seen going.
    Transfers,
    /// Say this.
    Say(String),
}

/// What came of an errand the feed carried out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Done {
    /// A write, and what the catalogue answered.
    Edited(Edit, Result<(), Why>),
    /// A channel added — its name — or why not.
    Added(Result<String, Unadded>),
    /// Where a scan was asked.
    Scan(Scanned),
    /// What the player said to a video, or that nobody answered.
    Queued(ToPlay, Result<(), Queued>),
    /// A download handed to the queue, or the queue not there.
    Downloading(String, bool),
}

/// Why a channel was not added, as the door can say it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unadded {
    /// yt-dlp did not say which channel it is (the key of its words).
    Unresolved(&'static str),
    /// The catalogue refused.
    Refused(Why),
    /// It is in, and the consent could not be kept.
    NotLet(&'static str),
}

/// Where a scan was asked — or why it was not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scanned {
    /// The brain took it: queued, gated, journaled, marked.
    Brain,
    /// No brain (or one without this module); systemd started the unit.
    Systemd,
    /// Neither answered.
    Nowhere,
    /// The power saver is on, and no scan starts while it is — the module's
    /// own rule: nothing was asked.
    Saver,
    /// This is a second instance (the development loop): its brain only
    /// rehearses, and the unit is the installed session's, which scans the
    /// installed catalogue. Nothing was asked.
    OtherInstance,
}

/// Why the player did not take a video — the player's own facts, worded by
/// the view in the door's language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Queued {
    /// The queue has it already — under this name.
    Already(ronoctua_player::queue::Name),
    /// The player refused.
    Refused(ronoctua_player::wire::Trouble),
    /// Nobody answered.
    Unreachable,
}

/// A question on the page, waiting for its "no" or its "yes".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    /// Delete a video for good.
    Delete(VideoId, String),
    /// Hide a video.
    Hide(VideoId, String),
    /// Remove a channel and its videos.
    RemoveChannel(ChannelId, String),
    /// Remove a rule.
    RemoveRule(RuleId),
    /// Add a video the queue already has.
    Again(ToPlay, String),
}

/// A question, and where its cursor is.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Confirm {
    what: Pending,
    /// The cursor is on "yes".
    yes: bool,
    /// The spot of the row it was asked from: the question is drawn under
    /// it, and the cursor goes back to it once answered.
    under: String,
}

/// What `RET` — or a turn — does on a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Nothing: a title, a note.
    Nothing,
    /// A fact: `RET` copies it.
    Copy(String),
    /// Open a page.
    Go(Page),
    /// A scan, now.
    Scan,
    /// Let YouTube be asked, or not — what it is now.
    Consent(bool),
    /// Add a channel.
    AddChannel,
    /// A channel's score, as it is now.
    ChannelScore(ChannelId, i32),
    /// A channel's Shorts, as they are now.
    ChannelShorts(ChannelId, Option<bool>),
    /// A channel's videos.
    ChannelVideos(ChannelId),
    /// Remove a channel.
    RemoveChannel(ChannelId, String),
    /// Add a length band in a scope.
    AddBand(Scope),
    /// Add words in a scope.
    AddTerm(Scope),
    /// Block Shorts everywhere, or not — as it is now.
    BlockShorts(bool),
    /// A rule's points.
    Points(Rule),
    /// A recency rule's days.
    Days(Rule),
    /// A band's length.
    Band(Rule),
    /// A term's words.
    Words(Rule),
    /// A term's way of matching.
    Match(Rule),
    /// One field a term reads.
    Field(Rule, Fields),
    /// A rule in force, or not.
    On(Rule),
    /// Remove a rule.
    RemoveRule(RuleId),
    /// A rating's weight, as it is now.
    Weight(Rating),
    /// The list's channel.
    FilterChannel,
    /// The list's Shorts.
    FilterShorts,
    /// The list's rating.
    FilterRating,
    /// The list's hidden videos.
    FilterHidden,
    /// The list's words.
    FilterText,
    /// How far back the list reaches.
    FilterSince,
    /// A list or a grid.
    Layout,
    /// The page of the list.
    PageOf,
    /// A video: its page.
    Video(VideoId),
    /// A video's rating.
    Rating(VideoId, Option<Rating>),
    /// A video's second tags.
    SecondTags(VideoId, String),
    /// A video's second description: edit it here.
    SecondDescription(VideoId, String),
    /// The video in the player.
    Player(VideoId),
    /// The video in Chrome.
    Kiosk(VideoId),
    /// The video downloaded.
    Download(VideoId),
    /// The video hidden, or shown — as it is now.
    Hide(VideoId, bool),
    /// The video deleted.
    Delete(VideoId),
    /// An answer of the question on the page: `true` is "yes".
    Answer(bool),
    /// A line of the description being edited, and where it is in the text.
    Text(crate::text_area::Drawn),
}

/// A box on a row where a thumbnail is drawn: which line of its picture the
/// row is, where the box starts among the row's cells and how big it is, and
/// the picture's file — placed ONCE, when the rows are built, so each door
/// draws it in the same box (`player_metadata::cover_place`, the drawing
/// both doors already share for a picture over rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumb {
    /// Its line of the picture: `0` is the top.
    pub line: u16,
    /// How many lines the box takes.
    pub lines: u16,
    /// The box's first cell, as a field's `at`.
    pub left: u16,
    /// How many cells wide the box is.
    pub cells: u16,
    /// The picture's file.
    pub file: PathBuf,
}

/// A cell of a grid's shelf a click takes — and the hover lights, the same
/// box — and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Its first cell, as a field's `at`.
    pub from: u16,
    /// How many cells it takes: the tile and the air on both sides of it
    /// ([`TILE`]); the air between two cells is nobody's.
    pub width: u16,
    /// What it is.
    pub target: Target,
    /// Its identity.
    pub spot: String,
}

/// How the hover lights a line of the view in the footer's region
/// (`Footer::video_metrics_band`): the window lights the same cells from the
/// same rows (`video_metrics_window::body`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Band {
    /// The usual band.
    Plain,
    /// A cell of a shelf: where it starts after the region's base and how
    /// wide it is, in pixels — the cell a click takes.
    Cell(f32, f32),
    /// Nothing: a picture's or a title's line, or the air between two cells.
    Nothing,
}

/// One drawn line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Placed and cut.
    pub fields: Vec<Field>,
    /// A title, a note, the rest of a label: the cursor skips it.
    pub heading: bool,
    /// What `RET` does here.
    pub target: Target,
    /// Its identity across a rebuild.
    pub spot: String,
    /// The line CONTINUES the row above it — the rest of a label, a shelf's
    /// lower lines: a click there is a click on it ([`Row::owner`]).
    pub continues: bool,
    /// The cells of a grid's shelf, on its first line.
    pub cells: Vec<Cell>,
    /// The thumbnails drawn over this line.
    pub thumbs: Vec<Thumb>,
    /// The cell a text caret stands before, on a line being edited.
    pub caret: Option<u16>,
    /// What the pointer resting here says, when it is more than its label.
    pub about: Option<&'static str>,
}

impl Row {
    /// Whether the cursor stops on it.
    #[must_use]
    pub const fn stops(&self) -> bool {
        !self.heading && !self.continues
    }

    /// The row line `n` of `rows` belongs to.
    #[must_use]
    pub fn owner(rows: &[Self], n: usize) -> usize {
        let mut n = n.min(rows.len().saturating_sub(1));
        while n > 0 && rows.get(n).is_some_and(|r| r.continues) {
            n -= 1;
        }
        n
    }

    /// Every line of the row that begins at `owner`.
    #[must_use]
    pub fn lines_of(rows: &[Self], owner: usize) -> usize {
        1 + rows
            .iter()
            .skip(owner + 1)
            .take_while(|r| r.continues)
            .count()
    }
}

/// Where a row's cells begin and how wide they are — the power view's
/// arithmetic, which every view of this shape shares.
#[must_use]
pub fn extent(columns: u16) -> (u16, u16) {
    crate::power::extent(columns)
}

/// The air a tile keeps inside its cell on EACH side — the grid's own gap
/// (`ronoctua_design::GAP`): the hover lights the cell, and a picture, a
/// title and a score that touched its left edge with all the air on the
/// right read as cut.
pub const TILE_AIR: u16 = ronoctua_design::GAP;

/// A cell of the grid: the thumbnail's box, `space.videoMetrics.thumbCells`
/// wide, and its air on both sides.
pub const TILE: u16 = tokens::space::VIDEOMETRICS_THUMBCELLS + 2 * TILE_AIR;

/// The grid's shelves, at a width: how many cells side by side, each
/// [`TILE`] wide — through `ronoctua_design::Grid`, the one grid arithmetic
/// of the tree.
#[must_use]
pub fn grid_of(room: u16) -> ronoctua_design::grid::Grid {
    ronoctua_design::grid::Grid::fitted(room, TILE, 0, u16::MAX, SHELF_LINES)
}

/// How many lines a grid's shelf takes: the picture's, two for the title,
/// one for what it scores, one for when it was published, one of air. The
/// score and the age are a line EACH: "10 000 pontos · há 12 dias" is more
/// than a tile is wide in either language, and a tile never cuts them.
pub const SHELF_LINES: u16 =
    tokens::space::VIDEOMETRICS_THUMBLINES + tokens::space::VIDEOMETRICS_TITLELINES + 3;

/// How many pictures a page of the grid may put on screen — never more than
/// the renderer's frame keeps for the file manager's grid, which is the room
/// this view's pictures take while it is up.
pub const PICTURES_MOST: u32 = tokens::space::VIDEOMETRICS_THUMBSMOST as u32;

const _: () = assert!(
    tokens::space::VIDEOMETRICS_THUMBSMOST <= tokens::space::FILES_THUMBMOST,
    "a page of thumbnails beyond what the renderer's frame keeps would lose pictures"
);
const _: () = assert!(
    PER_PAGE <= PICTURES_MOST,
    "a page of the grid is a page of the list: all of it must be drawable"
);

/// The texture a thumbnail goes up under: its file, the size it was decoded
/// at and the version of the decoding — what changes the pixels. No palette:
/// nothing here is painted in a token's value.
#[must_use]
pub fn picture_key(file: &std::path::Path) -> String {
    format!(
        "video-metrics\u{1}{}\u{1}{}\u{1}{}",
        file.display(),
        tokens::space::VIDEOMETRICS_THUMBPX,
        PICTURE_FORMAT
    )
}

/// The version of how a thumbnail is decoded. A change that changes the
/// pixels bumps it, and every key with it.
pub const PICTURE_FORMAT: u32 = 1;

/// The view's state.
#[derive(Debug, Clone, Default)]
pub struct View {
    /// The page on screen, and the ones before it.
    trail: Vec<Page>,
    /// The cursor's identity.
    at: String,
    /// Its index at the last build — where it goes when its spot is gone.
    last: usize,
    /// The spot the cursor had on each page left — where it returns.
    ///
    /// With its row's index then: a row that went while the page was away
    /// (the video deleted from its own page) leaves the cursor on the row
    /// that took its place, not on the page's first.
    left_at: HashMap<Page, (String, usize)>,
    /// What the list shows.
    filter: Filter,
    /// How far back the list reaches.
    since: Since,
    /// Which page of the list.
    page: u32,
    /// A list or a grid.
    layout: Layout,
    /// A question on the page.
    confirm: Option<Confirm>,
    /// What the line's question is about — the row it was asked from.
    asked_for: Option<Target>,
    /// The second description being edited: whose, and the text.
    editing: Option<(VideoId, crate::text_area::TextArea)>,
    /// The width the rows were last built at, in cells — STORED WHERE IT IS
    /// MEASURED, for a key that arrives without one (a click on the hint).
    width: std::cell::Cell<u16>,
    /// The cursor moved without a key — a question the view put on the page
    /// from what an errand came to — and the door owes it a follow
    /// ([`View::follow_owed`]).
    follow_owed: bool,
}

/// The id a text area of this view answers under.
const DESCRIPTION: &str = "video-metrics-second-description";

impl View {
    /// Opens it on `page` — the whole trail back to the home page, so `Esc`
    /// walks back the way a person would have come — and asks for what the
    /// page draws.
    #[must_use]
    pub fn fresh(page: Page) -> (Self, Errand) {
        let trail = match page {
            Page::Home => vec![Page::Home],
            other => vec![Page::Home, other],
        };
        let me = Self {
            trail,
            ..Self::default()
        };
        let look = me.look();
        (me, Errand::Look(look))
    }

    /// The page on screen.
    #[must_use]
    pub fn page(&self) -> &Page {
        self.trail.last().unwrap_or(&Page::Home)
    }

    /// What the page on screen draws — what the feed reads.
    #[must_use]
    pub fn look(&self) -> Look {
        let page = self.page().clone();
        let query = (page == Page::Videos).then(|| Query {
            filter: self.filter.clone(),
            page: self.page,
            pictures: self.layout == Layout::Grid,
        });
        Look { page, query }
    }

    /// "Since" made a MOMENT, once, when it is chosen: `now − back`. The
    /// list then stays the list that was asked for — a filter worked out
    /// again at every draw would stop matching the reading the feed holds
    /// the moment its clock moved past a boundary, and the page would say
    /// "reading" for ever.
    fn since_chosen(&mut self, since: Since) {
        self.since = since;
        self.filter.published_after_s = since.seconds().map(|back| now_s() - back);
    }

    /// Whether the cursor moved without a key since the door last asked —
    /// a question the view put on the page from what an errand came to. The
    /// door follows it as it follows a key: it is the consequence of one.
    pub fn follow_owed(&mut self) -> bool {
        std::mem::take(&mut self.follow_owed)
    }

    /// Whether a description is being edited — every key is its.
    #[must_use]
    pub const fn editing(&self) -> bool {
        self.editing.is_some()
    }

    /// The row the cursor is on, among `rows`.
    #[must_use]
    pub fn cursor(&self, rows: &[Row]) -> usize {
        rows.iter()
            .position(|r| r.spot == self.at || r.cells.iter().any(|c| c.spot == self.at))
            .unwrap_or_else(|| self.nearest(rows))
    }

    /// The row nearest to where the cursor last was that it may stop on.
    fn nearest(&self, rows: &[Row]) -> usize {
        let from = self.last.min(rows.len().saturating_sub(1));
        (from..rows.len())
            .chain((0..from).rev())
            .find(|&i| rows.get(i).is_some_and(Row::stops))
            .unwrap_or(0)
    }

    /// Puts the cursor's identity back in step with the rows drawn — after a
    /// reading arrived. It never moves the page.
    pub fn settle(&mut self, rows: &[Row]) {
        let i = self.cursor(rows);
        if let Some(row) = rows.get(i) {
            if row.spot != self.at && !row.cells.iter().any(|c| c.spot == self.at) {
                self.at = row
                    .cells
                    .first()
                    .map_or_else(|| row.spot.clone(), |c| c.spot.clone());
            }
            self.last = i;
        }
    }

    /// The hint line: the keys that work where the cursor is — or the text's
    /// own keys while one is edited. The way back goes in front of them in
    /// each door (`chrome::with_the_way_back`).
    #[must_use]
    pub fn hint(&self, rows: &[Row], texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        if self.editing.is_some() {
            return crate::text_area::hint(texts);
        }
        let target = self.focused_target(rows);
        let here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| self.works(*w, target.as_ref()))
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        crate::chrome::hint_of_marked(&here, |_| None, texts)
    }

    /// The row the region follows: the caret's while a text is edited, else
    /// the cursor's.
    #[must_use]
    pub fn drawn_cursor(&self, rows: &[Row]) -> usize {
        if self.editing.is_some() {
            return text_cursor(rows).unwrap_or(0);
        }
        self.cursor(rows)
    }

    /// Whether a key of `w` does something with the cursor on `target`.
    fn works(&self, w: Where, target: Option<&Target>) -> bool {
        match w {
            Where::Anywhere => true,
            Where::OnAValue => target.is_some_and(|t| turns(t) || self.on_the_grid(t)),
            Where::OnTheVideos => *self.page() == Page::Videos && self.confirm.is_none(),
            Where::OnAVideo => self.confirm.is_none() && self.video_here(target).is_some(),
        }
    }

    /// Whether the cursor is on a cell of the grid.
    fn on_the_grid(&self, target: &Target) -> bool {
        self.layout == Layout::Grid
            && *self.page() == Page::Videos
            && matches!(target, Target::Video(_))
    }

    /// The video the cursor is about: the page's, or the list's row or cell.
    fn video_here(&self, target: Option<&Target>) -> Option<VideoId> {
        match self.page() {
            Page::Video(id) => Some(id.clone()),
            Page::Videos => match target {
                Some(Target::Video(id)) => Some(id.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    /// What the focused row — or cell — does.
    fn focused_target(&self, rows: &[Row]) -> Option<Target> {
        let row = rows.get(self.cursor(rows))?;
        Some(
            row.cells
                .iter()
                .find(|c| c.spot == self.at)
                .map_or_else(|| row.target.clone(), |c| c.target.clone()),
        )
    }

    /// The sentence the pointer resting on row `n` reads: the row's own, or
    /// what `RET` does there.
    #[must_use]
    pub fn about(rows: &[Row], n: usize, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let row = rows.get(Row::owner(rows, n))?;
        if let Some(k) = row.about {
            return Some(texts.t(k));
        }
        let k = match &row.target {
            Target::Scan => key::CMD_VIDEO_METRICS_SCAN,
            Target::AddChannel => key::CMD_VIDEO_METRICS_ADD_CHANNEL,
            Target::FilterText => key::CMD_VIDEO_METRICS_SEARCH,
            Target::Player(_) => key::CMD_VIDEO_METRICS_PLAYER,
            Target::Kiosk(_) => key::CMD_VIDEO_METRICS_KIOSK,
            Target::Download(_) => key::CMD_VIDEO_METRICS_DOWNLOAD,
            Target::Hide(..) => key::CMD_VIDEO_METRICS_HIDE,
            Target::Delete(_) => key::CMD_VIDEO_METRICS_DELETE,
            Target::Copy(_) => key::ABOUT_VIDEO_METRICS_COPY,
            _ => return None,
        };
        Some(texts.t(k))
    }

    /// Puts the cursor where a click landed — on a row, or on the cell of a
    /// shelf at `column` (a field's `at`). Answers whether the cursor was
    /// ALREADY there: that is the second click, which the door turns into
    /// `RET` (invariant 19). A click on a line being edited places the caret.
    pub fn clicked(&mut self, n: usize, column: u16, rows: &[Row]) -> bool {
        let owner = Row::owner(rows, n);
        let Some(row) = rows.get(owner) else {
            return false;
        };
        // A SHELF: the cell under the column, on any of its lines — and the
        // air between two cells, or past the last, takes nothing: it lights
        // nothing either (`Band::Nothing`).
        if !row.cells.is_empty() {
            let Some(cell) = row
                .cells
                .iter()
                .find(|c| crate::chrome::in_cell(column, c.from, c.width))
            else {
                return false;
            };
            let again = self.at == cell.spot;
            self.at = cell.spot.clone();
            self.last = owner;
            return again;
        }
        if !row.stops() {
            return false;
        }
        let again = self.at == row.spot;
        self.at = row.spot.clone();
        self.last = owner;
        again
    }

    /// A key, with the rows as the door drew them at its width.
    pub fn press(&mut self, press: Press, rows: &[Row], r: &Readings) -> Errand {
        if self.editing.is_some() {
            // THE TEXT IS EDITED BY `text_key`; a press here is a click on
            // its hint (`Esc`), read as the key.
            if press == Press::Escape {
                return self.text_key(
                    ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Esc),
                    self.width.get(),
                );
            }
            return Errand::Nothing;
        }
        let i = self.cursor(rows);
        if let Some(row) = rows.get(i) {
            self.last = i;
            if row.spot != self.at && !row.cells.iter().any(|c| c.spot == self.at) {
                self.at = row
                    .cells
                    .first()
                    .map_or_else(|| row.spot.clone(), |c| c.spot.clone());
            }
        }
        let target = self.focused_target(rows).unwrap_or(Target::Nothing);
        if self.confirm.is_some() {
            return self.confirm_press(press);
        }
        match press {
            Press::Up => {
                self.walk(rows, false);
                Errand::Nothing
            }
            Press::Down => {
                self.walk(rows, true);
                Errand::Nothing
            }
            Press::Less | Press::More if self.on_the_grid(&target) => {
                self.sideways(rows, press == Press::More);
                Errand::Nothing
            }
            Press::Less => self.turn(&target, false, r),
            Press::More => self.turn(&target, true, r),
            Press::Enter => self.enter(&target, r),
            Press::Escape => self.back(),
            Press::PageBack => self.turn_page(false, r),
            Press::PageOn => self.turn_page(true, r),
            Press::Layout => self.relayout(),
            Press::Search => self.ask(
                Ask::Search,
                self.filter.text.clone(),
                Some(Target::FilterText),
            ),
            Press::Player | Press::Kiosk | Press::Download | Press::Hide | Press::Delete => {
                match self.video_here(Some(&target)) {
                    Some(id) => self.on_a_video(press, &id, r),
                    None => Errand::Nothing,
                }
            }
        }
    }

    /// A key while the description is edited — every key is the text's.
    /// `width` is the width its lines are drawn at, which `↑`/`↓` walk.
    pub fn text_key(&mut self, key: ronoctua_keymap::Key, width: u16) -> Errand {
        use crate::text_area::Step;
        let Some((id, area)) = self.editing.as_mut() else {
            return Errand::Nothing;
        };
        match area.key(key, width) {
            Step::Nothing | Step::Moved | Step::Changed | Step::Asking => Errand::Nothing,
            Step::WantsPaste => Errand::Paste,
            Step::Keep(text) => Errand::Edit(Edit::SecondDescription(id.clone(), text)),
            Step::Discard => {
                self.editing = None;
                Errand::Nothing
            }
        }
    }

    /// The clipboard's text, into the description being edited. Answers
    /// whether a frame is owed.
    pub fn text_pasted(&mut self, text: &str) -> bool {
        self.editing
            .as_mut()
            .is_some_and(|(_, a)| a.paste(text) != crate::text_area::Step::Nothing)
    }

    /// Moves the cursor to the next row it stops on — on the grid, to the
    /// shelf above or below, at the same place along it.
    fn walk(&mut self, rows: &[Row], down: bool) {
        let i = self.cursor(rows);
        let along = rows
            .get(i)
            .and_then(|r| r.cells.iter().position(|c| c.spot == self.at));
        let order: Vec<usize> = if down {
            (i + 1..rows.len()).collect()
        } else {
            (0..i).rev().collect()
        };
        for j in order {
            let Some(row) = rows.get(j) else { continue };
            if !row.stops() {
                continue;
            }
            self.last = j;
            self.at = match (along, row.cells.is_empty()) {
                (Some(k), false) => row.cells[k.min(row.cells.len() - 1)].spot.clone(),
                (None, false) => row.cells[0].spot.clone(),
                _ => row.spot.clone(),
            };
            return;
        }
    }

    /// The cell before or after on the grid — on to the next shelf past the
    /// end of one.
    fn sideways(&mut self, rows: &[Row], on: bool) {
        let cells: Vec<(usize, &Cell)> = rows
            .iter()
            .enumerate()
            .flat_map(|(i, r)| r.cells.iter().map(move |c| (i, c)))
            .collect();
        let Some(k) = cells.iter().position(|(_, c)| c.spot == self.at) else {
            return;
        };
        let to = if on {
            k + 1
        } else if let Some(k) = k.checked_sub(1) {
            k
        } else {
            return;
        };
        if let Some((i, c)) = cells.get(to) {
            self.at = c.spot.clone();
            self.last = *i;
        }
    }

    /// One page back — the question first, and the home page closes.
    fn back(&mut self) -> Errand {
        if self.trail.len() <= 1 {
            return Errand::Close;
        }
        self.leave();
        Errand::Look(self.look())
    }

    /// The page on screen left for the one before it, the cursor where it
    /// was there. The caller asks for the page now on screen: here it is
    /// [`View::back`]; after a page whose subject was deleted it is
    /// [`View::took`], once the deletion is done.
    fn leave(&mut self) {
        if self.trail.len() <= 1 {
            return;
        }
        if let Some(page) = self.trail.pop() {
            self.left_at.remove(&page);
        }
        let here = self.page().clone();
        (self.at, self.last) = self.left_at.remove(&here).unwrap_or_default();
    }

    /// Opens a page, remembering where the cursor was on this one.
    fn go(&mut self, page: Page) -> Errand {
        if page == *self.page() {
            return Errand::Look(self.look());
        }
        self.left_at
            .insert(self.page().clone(), (self.at.clone(), self.last));
        self.trail.push(page);
        self.at.clear();
        self.last = 0;
        Errand::Look(self.look())
    }

    /// Opens a page from outside — a command, a mark — keeping the trail to
    /// it: the home page under it.
    pub fn open(&mut self, page: Page) -> Errand {
        if *self.page() == page {
            return Errand::Look(self.look());
        }
        self.trail.truncate(1);
        self.confirm = None;
        self.editing = None;
        self.go(page)
    }

    /// Asks a value on the door's line, about the row `target`.
    fn ask(&mut self, ask: Ask, initial: Option<String>, target: Option<Target>) -> Errand {
        self.asked_for = target;
        Errand::Ask { ask, initial }
    }

    /// `RET` on a row.
    fn enter(&mut self, target: &Target, r: &Readings) -> Errand {
        match target {
            Target::Nothing | Target::Text(_) | Target::Answer(_) => Errand::Nothing,
            Target::Copy(text) => Errand::Copy(text.clone()),
            Target::Go(page) => self.go(page.clone()),
            Target::Video(id) => self.go(Page::Video(id.clone())),
            Target::ChannelVideos(id) => {
                self.filter = Filter {
                    channel: Some(id.clone()),
                    ..Filter::default()
                };
                self.since = Since::Any;
                self.page = 0;
                self.go(Page::Videos)
            }
            Target::Scan => Errand::Scan,
            Target::Consent(on) => Errand::Edit(Edit::Consent(!*on)),
            Target::AddChannel => self.ask(Ask::AddChannel, None, Some(target.clone())),
            Target::ChannelScore(_, score) => self.ask(
                Ask::ChannelScore,
                Some(score.to_string()),
                Some(target.clone()),
            ),
            // A WEIGHT IS TYPED FROM THE ONE READ: the row exists only then.
            Target::Weight(rating) => match r.weights {
                Some(w) => self.ask(
                    Ask::Weight,
                    Some(w.of(*rating).to_string()),
                    Some(target.clone()),
                ),
                None => Errand::Nothing,
            },
            Target::Points(rule) => self.ask(
                Ask::Points,
                Some(rule.points.to_string()),
                Some(target.clone()),
            ),
            Target::Days(rule) => {
                let days = match rule.kind {
                    RuleKind::Recency { days } => days,
                    _ => RECENCY_DAYS,
                };
                self.ask(Ask::Days, Some(days.to_string()), Some(target.clone()))
            }
            Target::Band(rule) => {
                let (min_s, max_s) = band_of_rule(rule).unwrap_or(BAND);
                let typed = format!("{}-{}", words::length(min_s), words::length(max_s));
                self.ask(Ask::Band, Some(typed), Some(target.clone()))
            }
            Target::Words(rule) => {
                let now = match &rule.kind {
                    RuleKind::Term { pattern, .. } => Some(pattern.clone()),
                    _ => None,
                };
                self.ask(Ask::Words, now, Some(target.clone()))
            }
            Target::AddBand(_) => self.ask(Ask::Band, None, Some(target.clone())),
            Target::AddTerm(_) => self.ask(Ask::Words, None, Some(target.clone())),
            Target::FilterText => {
                self.ask(Ask::Search, self.filter.text.clone(), Some(target.clone()))
            }
            Target::SecondTags(_, now) => self.ask(
                Ask::SecondTags,
                (!now.is_empty()).then(|| now.clone()),
                Some(target.clone()),
            ),
            Target::SecondDescription(id, text) => {
                self.editing = Some((
                    id.clone(),
                    crate::text_area::TextArea::new(DESCRIPTION, text),
                ));
                Errand::Nothing
            }
            Target::RemoveChannel(id, name) => {
                self.asks(Pending::RemoveChannel(id.clone(), name.clone()))
            }
            Target::RemoveRule(id) => self.asks(Pending::RemoveRule(*id)),
            Target::Player(id) => self.on_a_video(Press::Player, id, r),
            Target::Kiosk(id) => self.on_a_video(Press::Kiosk, id, r),
            Target::Download(id) => self.on_a_video(Press::Download, id, r),
            Target::Hide(id, _) => self.on_a_video(Press::Hide, id, r),
            Target::Delete(id) => self.on_a_video(Press::Delete, id, r),
            // A RING OR A SWITCH: `RET` turns it on, as `→` does.
            other => self.turn(other, true, r),
        }
    }

    /// `←`/`→` on a row: a number a step, a ring round, a switch over.
    fn turn(&mut self, target: &Target, on: bool, r: &Readings) -> Errand {
        let step = if on { POINTS_STEP } else { -POINTS_STEP };
        match target {
            Target::ChannelScore(id, score) => Errand::Edit(Edit::ChannelScore(
                id.clone(),
                (score + step).clamp(0, ronoctua_video_metrics::CHANNEL_SCORE_MOST),
            )),
            Target::ChannelShorts(id, now) => {
                let ring = [None, Some(true), Some(false)];
                Errand::Edit(Edit::ChannelShorts(id.clone(), ring_step(&ring, now, on)))
            }
            // FROM THE VALUES READ, never from a default: the three weights
            // not turned would be written back as the defaults.
            Target::Weight(rating) => {
                let Some(mut w) = r.weights else {
                    return Errand::Nothing;
                };
                let slot = match rating {
                    Rating::VeryNegative => &mut w.very_negative,
                    Rating::Negative => &mut w.negative,
                    Rating::Positive => &mut w.positive,
                    Rating::VeryPositive => &mut w.very_positive,
                };
                *slot = slot.saturating_add(step);
                Errand::Edit(Edit::Weights(w))
            }
            // FROM THE SWITCHES READ: a default would write the first scan's
            // limits back as the defaults, with this one switch flipped.
            Target::BlockShorts(now) => {
                let Some(mut s) = r.settings else {
                    return Errand::Nothing;
                };
                s.block_shorts = !now;
                Errand::Edit(Edit::Settings(s))
            }
            Target::Consent(now) => Errand::Edit(Edit::Consent(!now)),
            Target::Points(rule) => {
                let mut rule = rule.clone();
                rule.points = rule.points.saturating_add(step);
                // RECENCY IS NEVER NEGATIVE: the catalogue refuses it, and a
                // step that would take it there stops at nothing.
                if matches!(rule.kind, RuleKind::Recency { .. }) {
                    rule.points = rule.points.max(0);
                }
                Errand::Edit(Edit::PutRule(rule))
            }
            Target::Days(rule) => {
                let mut rule = rule.clone();
                if let RuleKind::Recency { days } = &mut rule.kind {
                    *days = if on {
                        days.saturating_add(1)
                    } else {
                        days.saturating_sub(1).max(1)
                    };
                }
                Errand::Edit(Edit::PutRule(rule))
            }
            Target::Match(rule) => {
                let mut rule = rule.clone();
                if let RuleKind::Term { how, .. } = &mut rule.kind {
                    let ring = [TermMatch::Word, TermMatch::Phrase, TermMatch::Prefix];
                    *how = ring_step(&ring, how, on);
                }
                Errand::Edit(Edit::PutRule(rule))
            }
            Target::Field(rule, field) => {
                let mut rule = rule.clone();
                if let RuleKind::Term { fields, .. } = &mut rule.kind {
                    let flipped = Fields(fields.0 ^ field.0);
                    // A TERM READS SOMETHING: the last field is not taken
                    // away (the catalogue would refuse the rule).
                    if flipped.0 != 0 {
                        *fields = flipped;
                    }
                }
                Errand::Edit(Edit::PutRule(rule))
            }
            Target::On(rule) => {
                let mut rule = rule.clone();
                rule.enabled = !rule.enabled;
                Errand::Edit(Edit::PutRule(rule))
            }
            Target::Rating(id, now) => {
                let mut ring: Vec<Option<Rating>> = vec![None];
                ring.extend(Rating::ALL.map(Some));
                Errand::Edit(Edit::Rating(id.clone(), ring_step(&ring, now, on)))
            }
            Target::FilterChannel => {
                let mut ring: Vec<Option<ChannelId>> = vec![None];
                ring.extend(r.channels.iter().map(|c| Some(c.id.clone())));
                self.filter.channel = ring_step(&ring, &self.filter.channel, on);
                self.refiltered()
            }
            Target::FilterShorts => {
                let ring = [Shorts::Any, Shorts::Without, Shorts::Only];
                self.filter.shorts = ring_step(&ring, &self.filter.shorts, on);
                self.refiltered()
            }
            Target::FilterRating => {
                let mut ring: Vec<Option<Rating>> = vec![None];
                ring.extend(Rating::ALL.map(Some));
                self.filter.rating = ring_step(&ring, &self.filter.rating, on);
                self.refiltered()
            }
            Target::FilterHidden => {
                let ring = [Hidden::Without, Hidden::Any, Hidden::Only];
                self.filter.hidden = ring_step(&ring, &self.filter.hidden, on);
                self.refiltered()
            }
            Target::FilterSince => {
                self.since_chosen(ring_step(&Since::ALL, &self.since, on));
                self.refiltered()
            }
            Target::Layout => self.relayout(),
            Target::PageOf => self.turn_page(on, r),
            Target::Hide(id, _) => self.on_a_video(Press::Hide, id, r),
            _ => Errand::Nothing,
        }
    }

    /// The list read again from its first page, the filter changed.
    fn refiltered(&mut self) -> Errand {
        self.page = 0;
        Errand::Look(self.look())
    }

    /// A list, or a grid — the cursor stays on its video.
    fn relayout(&mut self) -> Errand {
        if *self.page() != Page::Videos {
            return Errand::Nothing;
        }
        self.layout = match self.layout {
            Layout::List => Layout::Grid,
            Layout::Grid => Layout::List,
        };
        Errand::Look(self.look())
    }

    /// The page of the list before or after — never past either end.
    fn turn_page(&mut self, on: bool, r: &Readings) -> Errand {
        if *self.page() != Page::Videos {
            return Errand::Nothing;
        }
        let pages = pages_of(r.listed.as_ref().map_or(0, |l| l.total));
        let to = if on {
            (self.page + 1).min(pages.saturating_sub(1))
        } else {
            self.page.saturating_sub(1)
        };
        if to == self.page {
            return Errand::Nothing;
        }
        self.page = to;
        self.at.clear();
        self.last = 0;
        Errand::Look(self.look())
    }

    /// What a key for a video does to it.
    fn on_a_video(&mut self, press: Press, id: &VideoId, r: &Readings) -> Errand {
        let (url, title, hidden) = match r.video_of(id).map(|v| &v.row).or_else(|| r.ranked(id)) {
            Some(row) => (row.url.clone(), row.title.clone(), row.hidden),
            None => return Errand::Nothing,
        };
        match press {
            Press::Player => Errand::Play(ToPlay {
                url,
                title,
                once: true,
            }),
            Press::Kiosk => Errand::Kiosk(url),
            Press::Download => Errand::Download { url, title },
            // SHOWN AGAIN AT ONCE: nothing is lost, so nothing is asked.
            Press::Hide if hidden => Errand::Edit(Edit::Hidden(id.clone(), false)),
            Press::Hide => self.asks(Pending::Hide(id.clone(), title)),
            Press::Delete => self.asks(Pending::Delete(id.clone(), title)),
            _ => Errand::Nothing,
        }
    }

    /// Puts a question on the page, under the row the cursor is on — the one
    /// it was asked from — its cursor born on the "no".
    fn asks(&mut self, what: Pending) -> Errand {
        let under = match &self.confirm {
            // A QUESTION IN PLACE OF ANOTHER stays where the first was asked.
            Some(c) => c.under.clone(),
            None => self.at.clone(),
        };
        self.confirm = Some(Confirm {
            what,
            yes: false,
            under,
        });
        self.at = answer_spot(false).to_owned();
        Errand::Nothing
    }

    /// A key while a question is on the page: the arrows move between its
    /// answers, `RET` gives the one the cursor is on, `Esc` is "no".
    fn confirm_press(&mut self, press: Press) -> Errand {
        let Some(c) = self.confirm.as_mut() else {
            return Errand::Nothing;
        };
        match press {
            Press::Up | Press::Down | Press::Less | Press::More => {
                c.yes = !c.yes;
                self.at = answer_spot(c.yes).to_owned();
                Errand::Nothing
            }
            Press::Enter => {
                let yes = c.yes;
                self.answer(yes)
            }
            Press::Escape => self.answer(false),
            _ => Errand::Nothing,
        }
    }

    /// The question answered — the cursor back on the row it was asked
    /// from, or, when the page it was about is left, where it was on the
    /// page before.
    fn answer(&mut self, yes: bool) -> Errand {
        let Some(c) = self.confirm.take() else {
            return Errand::Nothing;
        };
        self.at = c.under;
        if !yes {
            return Errand::Nothing;
        }
        match c.what {
            Pending::Delete(id, _) => {
                // THE VIDEO'S PAGE IS LEFT: what it was about is gone.
                if matches!(self.page(), Page::Video(v) if *v == id) {
                    self.leave();
                }
                Errand::Edit(Edit::Delete(id))
            }
            Pending::Hide(id, _) => Errand::Edit(Edit::Hidden(id, true)),
            Pending::RemoveChannel(id, _) => {
                if matches!(self.page(), Page::Channel(c) if *c == id) {
                    self.leave();
                }
                Errand::Edit(Edit::RemoveChannel(id))
            }
            Pending::RemoveRule(id) => {
                if matches!(self.page(), Page::Rule(..)) {
                    self.leave();
                }
                Errand::Edit(Edit::RemoveRule(id))
            }
            Pending::Again(play, _) => Errand::Play(ToPlay {
                once: false,
                ..play
            }),
        }
    }

    /// A click on an answer — or its row chosen by the keys — given.
    fn answered_by_click(&mut self, yes: bool) -> bool {
        let Some(c) = self.confirm.as_mut() else {
            return false;
        };
        if c.yes == yes {
            return true;
        }
        c.yes = yes;
        self.at = answer_spot(yes).to_owned();
        false
    }

    /// A second click on an answer: the door turns it into this, as it turns
    /// the second click on a row into `RET`.
    pub fn click_answers(&mut self, n: usize, rows: &[Row]) -> Option<Errand> {
        let Target::Answer(yes) = rows.get(n)?.target else {
            return None;
        };
        if self.answered_by_click(yes) {
            Some(self.answer(yes))
        } else {
            Some(Errand::Nothing)
        }
    }

    /// What was typed on the line, for the question `id` asked — the same
    /// call from either door.
    pub fn typed(
        &mut self,
        id: &str,
        text: &str,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(ask) = Ask::of_command(id) else {
            return Errand::Nothing;
        };
        let target = self.asked_for.take();
        let say = |k: &'static str| Errand::Say(texts.t(k));
        let number = || words::whole_of(text).and_then(|n| i32::try_from(n).ok());
        match (ask, target) {
            (Ask::AddChannel, _) => {
                let input = text.trim();
                if input.is_empty() {
                    return Errand::Nothing;
                }
                Errand::Edit(Edit::AddChannel(input.to_owned()))
            }
            // THE LIST IS WHERE A SEARCH IS SEEN: asked by name from another
            // page, the answer goes there.
            (Ask::Search, _) => {
                let words = text.trim();
                self.filter.text = (!words.is_empty()).then(|| words.to_owned());
                self.page = 0;
                if *self.page() == Page::Videos {
                    self.refiltered()
                } else {
                    self.go(Page::Videos)
                }
            }
            (Ask::ChannelScore, Some(Target::ChannelScore(id, _))) => match number() {
                Some(n) => Errand::Edit(Edit::ChannelScore(
                    id,
                    n.clamp(0, ronoctua_video_metrics::CHANNEL_SCORE_MOST),
                )),
                None => say(key::VIDEO_METRICS_TYPED_NOT_A_NUMBER),
            },
            (Ask::Weight, Some(Target::Weight(rating))) => match (number(), r.weights) {
                (Some(n), Some(mut w)) => {
                    match rating {
                        Rating::VeryNegative => w.very_negative = n,
                        Rating::Negative => w.negative = n,
                        Rating::Positive => w.positive = n,
                        Rating::VeryPositive => w.very_positive = n,
                    }
                    Errand::Edit(Edit::Weights(w))
                }
                (None, _) => say(key::VIDEO_METRICS_TYPED_NOT_A_NUMBER),
                // THE WEIGHTS WENT UNREAD since the question: the other three
                // would be written back as defaults.
                (Some(_), None) => say(key::VIDEO_METRICS_TYPED_NO_ROW),
            },
            (Ask::Points, Some(Target::Points(mut rule))) => match number() {
                Some(n) => {
                    rule.points = n;
                    Errand::Edit(Edit::PutRule(rule))
                }
                None => say(key::VIDEO_METRICS_TYPED_NOT_A_NUMBER),
            },
            (Ask::Days, Some(Target::Days(mut rule))) => {
                match number()
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|n| *n > 0)
                {
                    Some(n) => {
                        if let RuleKind::Recency { days } = &mut rule.kind {
                            *days = n;
                        }
                        Errand::Edit(Edit::PutRule(rule))
                    }
                    None => say(key::VIDEO_METRICS_TYPED_NOT_DAYS),
                }
            }
            (Ask::Band, Some(Target::Band(mut rule))) => match words::band_of(text, texts) {
                Some((min_s, max_s)) => {
                    match &mut rule.kind {
                        RuleKind::Band { min_s: a, max_s: b }
                        | RuleKind::Preferred { min_s: a, max_s: b } => {
                            *a = min_s;
                            *b = max_s;
                        }
                        _ => {}
                    }
                    Errand::Edit(Edit::PutRule(rule))
                }
                None => say(key::VIDEO_METRICS_TYPED_NOT_A_BAND),
            },
            (Ask::Band, Some(Target::AddBand(scope))) => match words::band_of(text, texts) {
                Some((min_s, max_s)) => Errand::Edit(Edit::PutRule(Rule {
                    id: None,
                    channel: scope,
                    kind: RuleKind::Band { min_s, max_s },
                    points: 0,
                    enabled: true,
                })),
                None => say(key::VIDEO_METRICS_TYPED_NOT_A_BAND),
            },
            (Ask::Words, Some(Target::Words(mut rule))) => {
                if ronoctua_video_metrics::score::words_of(text).is_empty() {
                    return say(key::VIDEO_METRICS_TYPED_NO_WORDS);
                }
                if let RuleKind::Term { pattern, .. } = &mut rule.kind {
                    text.trim().clone_into(pattern);
                }
                Errand::Edit(Edit::PutRule(rule))
            }
            (Ask::Words, Some(Target::AddTerm(scope))) => {
                if ronoctua_video_metrics::score::words_of(text).is_empty() {
                    return say(key::VIDEO_METRICS_TYPED_NO_WORDS);
                }
                Errand::Edit(Edit::PutRule(Rule {
                    id: None,
                    channel: scope,
                    kind: RuleKind::Term {
                        fields: Fields::TITLE.with(Fields::DESCRIPTION).with(Fields::TAGS),
                        pattern: text.trim().to_owned(),
                        how: TermMatch::Word,
                    },
                    points: 0,
                    enabled: true,
                }))
            }
            (Ask::SecondTags, Some(Target::SecondTags(id, _))) => {
                Errand::Edit(Edit::SecondTags(id, words::tags_of(text)))
            }
            // NO ROW TO TAKE IT: the question was asked by name with the
            // cursor on nothing it fits, or its row went away. Said, never
            // dropped in silence.
            _ => say(key::VIDEO_METRICS_TYPED_NO_ROW),
        }
    }

    /// The line's question was let go.
    pub fn unanswered(&mut self) {
        self.asked_for = None;
    }

    /// A question of [`Ask`] run BY NAME (`LEADER :`, the registry) with no
    /// value: asked about the row the cursor is on when that row is one it
    /// fits — exactly as `RET` there would ask it — and refused with a
    /// sentence otherwise. A channel's address and a search need no row.
    pub fn asked_by_name(
        &mut self,
        ask: Ask,
        rows: &[Row],
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        match ask {
            Ask::AddChannel => return self.ask(Ask::AddChannel, None, Some(Target::AddChannel)),
            Ask::Search => {
                return self.ask(
                    Ask::Search,
                    self.filter.text.clone(),
                    Some(Target::FilterText),
                );
            }
            _ => {}
        }
        if self.editing.is_none() && self.confirm.is_none() {
            let target = self.focused_target(rows).unwrap_or(Target::Nothing);
            if asks_of(&target) == Some(ask) {
                return self.enter(&target, r);
            }
        }
        Errand::Say(texts.t(key::VIDEO_METRICS_TYPED_NO_ROW))
    }

    /// What came of an errand the feed carried out, as the sentence the door
    /// says — and what follows it.
    pub fn took(
        &mut self,
        done: Done,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> (Option<Said>, Errand) {
        let long = |text: String| Some(Said::Long(text));
        let normal = |text: String| Some(Said::Normal(text));
        match done {
            Done::Edited(edit, Ok(())) => {
                if let Edit::SecondDescription(id, text) = &edit
                    && let Some((editing, area)) = self.editing.as_mut()
                    && editing == id
                {
                    area.kept_as(text);
                    return (
                        normal(texts.t(key::VIDEO_METRICS_DESCRIPTION_KEPT)),
                        Errand::Nothing,
                    );
                }
                // A PAGE WHOSE SUBJECT WENT was left when the question was
                // answered (`View::leave`); the page now on screen is asked
                // for, or it would draw what it held before — the deleted
                // video among the list's — and the feed would go on reading
                // the page that is gone.
                match edit {
                    Edit::Delete(_) => (
                        normal(texts.t(key::VIDEO_METRICS_DELETED)),
                        Errand::Look(self.look()),
                    ),
                    Edit::RemoveChannel(_) => (
                        normal(texts.t(key::VIDEO_METRICS_CHANNEL_REMOVED)),
                        Errand::Look(self.look()),
                    ),
                    Edit::RemoveRule(_) => (None, Errand::Look(self.look())),
                    _ => (None, Errand::Nothing),
                }
            }
            Done::Edited(_, Err(why)) => (long(texts.t(words::why_key(why))), Errand::Nothing),
            Done::Added(Ok(name)) => (
                normal(texts.t_with(key::VIDEO_METRICS_CHANNEL_ADDED, &[("name", &name)])),
                Errand::Nothing,
            ),
            Done::Added(Err(Unadded::Unresolved(k))) => (long(texts.t(k)), Errand::Nothing),
            Done::Added(Err(Unadded::Refused(why))) => {
                (long(texts.t(words::why_key(why))), Errand::Nothing)
            }
            Done::Added(Err(Unadded::NotLet(k))) => (long(texts.t(k)), Errand::Nothing),
            Done::Scan(Scanned::Brain) => (
                normal(texts.t(key::VIDEO_METRICS_SCAN_ASKED)),
                Errand::Nothing,
            ),
            Done::Scan(Scanned::Systemd) => (
                normal(texts.t(key::VIDEO_METRICS_SCAN_STARTED)),
                Errand::Nothing,
            ),
            Done::Scan(Scanned::Nowhere) => (
                long(texts.t(key::VIDEO_METRICS_SCAN_NOWHERE)),
                Errand::Nothing,
            ),
            Done::Scan(Scanned::Saver) => (
                long(texts.t(key::VIDEO_METRICS_SCAN_SAVER)),
                Errand::Nothing,
            ),
            Done::Scan(Scanned::OtherInstance) => (
                long(texts.t(key::VIDEO_METRICS_SCAN_OTHER_INSTANCE)),
                Errand::Nothing,
            ),
            Done::Queued(play, Ok(())) => (
                normal(texts.t_with(key::PLAYER_ADDED, &[("name", &play.title)])),
                Errand::Nothing,
            ),
            // ALREADY THERE: asked on the page, "no" first, under the row the
            // person asked from — and the door follows it there, as after a
            // key: it is the answer to theirs.
            Done::Queued(play, Err(Queued::Already(name))) => {
                let name = crate::player::name_of(&name, texts, u16::MAX);
                self.follow_owed = true;
                (None, self.asks(Pending::Again(play, name)))
            }
            Done::Queued(_, Err(Queued::Refused(t))) => (
                long(crate::player::trouble_sentence(&t, texts)),
                Errand::Nothing,
            ),
            Done::Queued(_, Err(Queued::Unreachable)) => (
                long(texts.t(key::VIDEO_METRICS_PLAYER_UNREACHABLE)),
                Errand::Nothing,
            ),
            // HANDED OVER: the queue is opened where it is seen going (the
            // design's `t t`), and the sentence says how to come back to it.
            Done::Downloading(title, true) => (
                normal(texts.t_with(
                    key::VIDEO_METRICS_DOWNLOADING,
                    &[("title", &title), ("keys", &r.transfers_keys)],
                )),
                Errand::Transfers,
            ),
            Done::Downloading(_, false) => (long(texts.t(key::TRANSFERS_OFFLINE)), Errand::Nothing),
        }
    }

    /// The rows to draw, at this width, from the readings as they are.
    #[must_use]
    pub fn rows(&self, r: &Readings, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let (left, room) = extent(columns);
        self.width.set(room);
        if let Some((_, area)) = &self.editing {
            return text_rows(area, left, room, texts);
        }
        let mut items: Vec<Item> = Vec::new();
        match self.page() {
            Page::Home => home(&mut items, r, texts),
            Page::Channels => channels(&mut items, r, texts),
            Page::Channel(id) => channel(&mut items, id, r, texts),
            Page::Rules => rules(&mut items, &None, r, texts),
            Page::Rule(scope, slot) => rule_page(&mut items, scope, slot, r, texts),
            Page::Weights => weights(&mut items, r, texts),
            Page::Videos => self.videos(&mut items, r, room, texts),
            Page::Video(id) => video(&mut items, id, r, texts),
        }
        // THE QUESTION UNDER THE ROW IT WAS ASKED FROM — a shelf's, for a
        // cell of the grid — or first, when that row went away.
        if let Some(c) = &self.confirm {
            let mut asked = Vec::new();
            question(&mut asked, c, texts);
            let at = items
                .iter()
                .position(|i| i.has_spot(&c.under))
                .map_or(0, |i| i + 1);
            items.splice(at..at, asked);
        }
        // THE CURSOR IS DRAWN WHERE A KEY WOULD ACT: on its spot, or — on a
        // page just opened, or when its row went away — on the row nearest to
        // where it was, which is the one `press` settles on.
        let rows = lay_out(items.clone(), left, room, &self.at);
        if rows
            .iter()
            .any(|x| x.spot == self.at || x.cells.iter().any(|c| c.spot == self.at))
        {
            return rows;
        }
        let near = rows.get(self.nearest(&rows)).map(|x| {
            x.cells
                .first()
                .map_or_else(|| x.spot.clone(), |c| c.spot.clone())
        });
        match near {
            Some(spot) => lay_out(items, left, room, &spot),
            None => rows,
        }
    }

    /// The videos' page: its filters, its page, and the list or the grid.
    fn videos(&self, items: &mut Vec<Item>, r: &Readings, room: u16, texts: &ronoctua_i18n::Texts) {
        items.push(Item::Heading(texts.t(key::VIDEO_METRICS_VIDEOS)));
        let channel = self.filter.channel.as_ref().map_or_else(
            || texts.t(key::VIDEO_METRICS_EVERY_CHANNEL),
            |id| {
                r.channel(id)
                    .map_or_else(|| id.0.clone(), |c| c.title.clone())
            },
        );
        let rating = self
            .filter
            .rating
            .map_or(key::VIDEO_METRICS_RATING_ANY, words::rating_key);
        items.extend([
            setting(
                "filter-channel",
                texts.t(key::VIDEO_METRICS_CHANNEL),
                channel,
                Target::FilterChannel,
            ),
            setting(
                "filter-shorts",
                texts.t(key::VIDEO_METRICS_FILTER_SHORTS),
                texts.t(words::shorts_key(self.filter.shorts)),
                Target::FilterShorts,
            ),
            setting(
                "filter-rating",
                texts.t(key::VIDEO_METRICS_RATING),
                texts.t(rating),
                Target::FilterRating,
            ),
            setting(
                "filter-hidden",
                texts.t(key::VIDEO_METRICS_FILTER_HIDDEN),
                texts.t(words::hidden_key(self.filter.hidden)),
                Target::FilterHidden,
            ),
            setting(
                "filter-text",
                texts.t(key::VIDEO_METRICS_FILTER_TEXT),
                self.filter
                    .text
                    .clone()
                    .unwrap_or_else(|| texts.t(key::VIDEO_METRICS_SEARCH_NONE)),
                Target::FilterText,
            ),
            setting(
                "filter-since",
                texts.t(key::VIDEO_METRICS_PUBLISHED),
                texts.t(self.since.key()),
                Target::FilterSince,
            ),
            setting(
                "layout",
                texts.t(key::VIDEO_METRICS_FILTER_LAYOUT),
                texts.t(match self.layout {
                    Layout::List => key::VIDEO_METRICS_LAYOUT_LIST,
                    Layout::Grid => key::VIDEO_METRICS_LAYOUT_GRID,
                }),
                Target::Layout,
            ),
        ]);
        // THE PAGE ASKED, and no other: a reading of the filter before (or of
        // the page before) is not this list, and says "reading" until this
        // one arrives. The layout is not asked about — the rows are the same.
        let listed = r
            .listed
            .as_ref()
            .filter(|l| l.query.page == self.page && l.query.filter == self.filter);
        let Some(listed) = listed else {
            refused_or_reading(items, r, texts);
            return;
        };
        let pages = pages_of(listed.total);
        items.push(setting(
            "page",
            texts.t(key::VIDEO_METRICS_PAGE),
            texts.t_args(
                key::VIDEO_METRICS_PAGE_OF,
                &[
                    ("page", Arg::Number(i64::from(self.page) + 1)),
                    ("pages", Arg::Number(i64::from(pages.max(1)))),
                    (
                        "videos",
                        Arg::Number(i64::try_from(listed.total).unwrap_or(i64::MAX)),
                    ),
                ],
            ),
            Target::PageOf,
        ));
        items.push(Item::Heading(texts.t(key::VIDEO_METRICS_BY_SCORE)));
        if listed.rows.is_empty() {
            items.push(Item::Note(
                texts.t(key::VIDEO_METRICS_NO_VIDEOS),
                tokens::color::TEXT_DIM,
            ));
            return;
        }
        match self.layout {
            // A TITLE IS A LINE OF ITS OWN, the whole width, wrapped — and
            // what it scores, its channel and its age under it: a title is
            // the long half of a video, and a label's column would fold it
            // into three short lines beside a value that says little.
            Layout::List => {
                for v in &listed.rows {
                    items.push(Item::Entry(Tile {
                        spot: video_spot(&v.id),
                        title: v.title.clone(),
                        meta: vec![texts.t_with(
                            key::VIDEO_METRICS_VIDEO_ROW,
                            &[
                                ("score", &words::points(v.score, texts)),
                                ("channel", &v.channel),
                                ("age", &age_of(v, r, texts)),
                            ],
                        )],
                        file: None,
                        target: Target::Video(v.id.clone()),
                        hidden: v.hidden,
                    }));
                }
            }
            Layout::Grid => {
                let grid = grid_of(room);
                for shelf in listed.rows.chunks(usize::from(grid.across.max(1))) {
                    items.push(Item::Shelf(
                        shelf
                            .iter()
                            .map(|v| Tile {
                                spot: video_spot(&v.id),
                                title: v.title.clone(),
                                meta: vec![words::points(v.score, texts), age_of(v, r, texts)],
                                file: v.thumb_file.clone(),
                                target: Target::Video(v.id.clone()),
                                hidden: v.hidden,
                            })
                            .collect(),
                        grid,
                    ));
                }
            }
        }
    }
}

/// What the door says after a [`Done`], and for how long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// A sentence.
    Normal(String),
    /// A refusal, read to the end.
    Long(String),
}

/// How many pages `total` videos make.
fn pages_of(total: u64) -> u32 {
    u32::try_from(total.div_ceil(u64::from(PER_PAGE))).unwrap_or(u32::MAX)
}

/// The band a rule covers, when it is a band.
const fn band_of_rule(rule: &Rule) -> Option<(u32, u32)> {
    match rule.kind {
        RuleKind::Band { min_s, max_s } | RuleKind::Preferred { min_s, max_s } => {
            Some((min_s, max_s))
        }
        _ => None,
    }
}

/// The next member of `ring` after `now` — or the one before — round the
/// ends; the first when `now` is not in it.
fn ring_step<T: Clone + PartialEq>(ring: &[T], now: &T, on: bool) -> T {
    let n = ring.len().max(1);
    let at = ring.iter().position(|x| x == now);
    let to = match (at, on) {
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
        (None, _) => 0,
    };
    ring[to].clone()
}

/// Whether `←`/`→` turn the row's value.
const fn turns(t: &Target) -> bool {
    matches!(
        t,
        Target::ChannelScore(..)
            | Target::ChannelShorts(..)
            | Target::Weight(_)
            | Target::BlockShorts(_)
            | Target::Consent(_)
            | Target::Points(_)
            | Target::Days(_)
            | Target::Match(_)
            | Target::Field(..)
            | Target::On(_)
            | Target::Rating(..)
            | Target::FilterChannel
            | Target::FilterShorts
            | Target::FilterRating
            | Target::FilterHidden
            | Target::FilterSince
            | Target::Layout
            | Target::PageOf
    )
}

/// The question `RET` asks on a row — `None` for a row it asks nothing on.
const fn asks_of(t: &Target) -> Option<Ask> {
    Some(match t {
        Target::AddChannel => Ask::AddChannel,
        Target::ChannelScore(..) => Ask::ChannelScore,
        Target::Points(_) => Ask::Points,
        Target::Days(_) => Ask::Days,
        Target::Band(_) | Target::AddBand(_) => Ask::Band,
        Target::Words(_) | Target::AddTerm(_) => Ask::Words,
        Target::FilterText => Ask::Search,
        Target::SecondTags(..) => Ask::SecondTags,
        Target::Weight(_) => Ask::Weight,
        _ => return None,
    })
}

/// The spot of a video's row or cell.
fn video_spot(id: &VideoId) -> String {
    format!("video:{}", id.0)
}

/// The spot of an answer of a question.
const fn answer_spot(yes: bool) -> &'static str {
    if yes { "answer-yes" } else { "answer-no" }
}

/// How long ago a video was published, or nothing when that is not known.
fn age_of(v: &RankedVideo, r: &Readings, texts: &ronoctua_i18n::Texts) -> String {
    v.published_at_s
        .map(|p| words::age(p, r.now_s, texts))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// The pages, as items: what a page says, before it is placed.
// ---------------------------------------------------------------------------

/// A `label: value` row of a page, before it is placed.
#[derive(Debug, Clone)]
struct Pair {
    spot: String,
    label: String,
    value: String,
    kind: Kind,
    target: Target,
    /// A colour its value says a state in — a failure, a warning.
    tone: Option<tokens::Color>,
    about: Option<&'static str>,
}

/// A video of the list, or of the grid.
#[derive(Debug, Clone)]
struct Tile {
    spot: String,
    title: String,
    /// What it scores, and when: one sentence on the list (wrapped), two
    /// lines on a tile — the score's and the age's.
    meta: Vec<String>,
    file: Option<PathBuf>,
    target: Target,
    hidden: bool,
}

/// What a page says, before it is placed.
#[derive(Debug, Clone)]
enum Item {
    /// A section's title (air before it, but for the first).
    Heading(String),
    /// A row.
    Pair(Pair),
    /// A sentence, wrapped and never cut.
    Note(String, tokens::Color),
    /// A video of the list: its title on the whole width, and under it what
    /// it scores, its channel and its age.
    Entry(Tile),
    /// A shelf of the grid.
    Shelf(Vec<Tile>, ronoctua_design::grid::Grid),
    /// A picture on its own, `thumbLines` tall.
    Figure(PathBuf),
    /// An answer of the question on the page.
    Answer { yes: bool, label: String },
}

impl Item {
    /// Whether the row — or a cell of the shelf — it becomes has `spot`.
    fn has_spot(&self, spot: &str) -> bool {
        match self {
            Self::Pair(p) => p.spot == spot,
            Self::Entry(t) => t.spot == spot,
            Self::Shelf(tiles, _) => tiles.iter().any(|t| t.spot == spot),
            _ => false,
        }
    }
}

/// A fact: `RET` copies its value.
fn fact(spot: &str, label: String, value: String) -> Item {
    Item::Pair(Pair {
        spot: spot.to_owned(),
        target: if value.is_empty() {
            Target::Nothing
        } else {
            Target::Copy(value.clone())
        },
        label,
        value,
        kind: Kind::Fact,
        tone: None,
        about: None,
    })
}

/// A row that acts.
fn setting(spot: &str, label: String, value: String, target: Target) -> Item {
    Item::Pair(Pair {
        spot: spot.to_owned(),
        label,
        value,
        kind: Kind::Setting,
        target,
        tone: None,
        about: None,
    })
}

/// A switch's words.
fn on_off(on: bool, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(if on {
        key::VIDEO_METRICS_ON
    } else {
        key::VIDEO_METRICS_OFF
    })
}

/// A date, in the person's shape; "never" for none.
fn date_or_never(at: Option<i64>, r: &Readings, texts: &ronoctua_i18n::Texts) -> String {
    at.map_or_else(
        || texts.t(key::VIDEO_METRICS_NEVER),
        |s| crate::chrome::when_text(s, &r.zone, false, texts),
    )
}

/// The catalogue's refusal, when it refused, or the sentence that it is
/// being read — a page with nothing to show says why.
fn refused_or_reading(items: &mut Vec<Item>, r: &Readings, texts: &ronoctua_i18n::Texts) {
    match r.catalogue {
        Some(Err(why)) => items.push(Item::Note(
            texts.t(words::why_key(why)),
            if why == Why::NotThere {
                tokens::color::TEXT_DIM
            } else {
                tokens::color::STATE_ERROR
            },
        )),
        _ => items.push(Item::Note(
            texts.t(key::VIDEO_METRICS_READING),
            tokens::color::TEXT_DIM,
        )),
    }
}

/// The question on the page, "no" first.
fn question(items: &mut Vec<Item>, c: &Confirm, texts: &ronoctua_i18n::Texts) {
    let (asked, no, yes) = match &c.what {
        Pending::Delete(_, title) => (
            texts.t_with(key::ASK_VIDEO_METRICS_DELETE, &[("title", title)]),
            key::VIDEO_METRICS_KEEP_VIDEO,
            key::VIDEO_METRICS_DELETE_IT,
        ),
        Pending::Hide(_, title) => (
            texts.t_with(key::ASK_VIDEO_METRICS_HIDE, &[("title", title)]),
            key::VIDEO_METRICS_LEAVE_SHOWN,
            key::VIDEO_METRICS_HIDE_IT,
        ),
        Pending::RemoveChannel(_, name) => (
            texts.t_with(key::ASK_VIDEO_METRICS_REMOVE_CHANNEL, &[("name", name)]),
            key::VIDEO_METRICS_KEEP_CHANNEL,
            key::VIDEO_METRICS_REMOVE_CHANNEL_IT,
        ),
        Pending::RemoveRule(_) => (
            texts.t(key::ASK_VIDEO_METRICS_REMOVE_RULE),
            key::VIDEO_METRICS_KEEP_RULE,
            key::VIDEO_METRICS_REMOVE_RULE_IT,
        ),
        Pending::Again(_, name) => (
            texts.t_with(key::ASK_VIDEO_METRICS_AGAIN, &[("name", name)]),
            key::VIDEO_METRICS_NOT_AGAIN,
            key::VIDEO_METRICS_AGAIN,
        ),
    };
    items.push(Item::Note(asked, tokens::color::TEXT_SECONDARY));
    items.push(Item::Answer {
        yes: false,
        label: texts.t(no),
    });
    items.push(Item::Answer {
        yes: true,
        label: texts.t(yes),
    });
}

/// The home page: the scan, the catalogue, what was left out.
fn home(items: &mut Vec<Item>, r: &Readings, texts: &ronoctua_i18n::Texts) {
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_SCAN)));
    if let Some(Err(why)) = r.catalogue
        && why != Why::NotThere
    {
        items.push(Item::Note(
            texts.t(words::why_key(why)),
            tokens::color::STATE_ERROR,
        ));
    }
    let scan = r.last_scan.as_ref().and_then(Option::as_ref);
    // THE LAST SCAN IN THREE FACTS — when, what it added, how many channels
    // failed — and not one sentence a narrow door would have to cut.
    // A RECORD WITH NO END is a scan running only while a scan HOLDS the
    // catalogue; one whose process was killed before it wrote its end is
    // said as that, and not as "scanning since" for hours.
    let running = r.scanning == Some(true);
    let (when, tone) = match scan {
        None => (texts.t(key::VIDEO_METRICS_NEVER), None),
        Some(s) if s.ended_at_s.is_none() => {
            let since = date_or_never(Some(s.started_at_s), r, texts);
            if running {
                (
                    texts.t_with(key::VIDEO_METRICS_SCAN_RUNNING, &[("when", &since)]),
                    Some(tokens::color::STATE_ACCENT),
                )
            } else {
                (
                    texts.t_with(key::VIDEO_METRICS_SCAN_CUT, &[("when", &since)]),
                    Some(tokens::color::STATE_WARNING),
                )
            }
        }
        Some(s) => (date_or_never(s.ended_at_s, r, texts), None),
    };
    items.push(Item::Pair(Pair {
        spot: "last-scan".to_owned(),
        label: texts.t(key::VIDEO_METRICS_LAST_SCAN),
        value: when,
        kind: Kind::Fact,
        target: Target::Nothing,
        tone,
        about: None,
    }));
    if let Some(s) = scan.filter(|s| s.ended_at_s.is_some()) {
        let all_failed = s.channels > 0 && s.channels_failed == s.channels;
        items.push(fact(
            "scan-added",
            texts.t(key::VIDEO_METRICS_SCAN_ADDED),
            s.added.to_string(),
        ));
        items.push(Item::Pair(Pair {
            spot: "scan-failed".to_owned(),
            label: texts.t(key::VIDEO_METRICS_SCAN_FAILED),
            value: texts.t_args(
                key::VIDEO_METRICS_FAILED_OF,
                &[
                    ("failed", Arg::Number(i64::from(s.channels_failed))),
                    ("channels", Arg::Number(i64::from(s.channels))),
                ],
            ),
            kind: Kind::Fact,
            target: Target::Nothing,
            tone: if all_failed {
                Some(tokens::color::STATE_ERROR)
            } else if s.channels_failed > 0 {
                Some(tokens::color::STATE_WARNING)
            } else {
                None
            },
            about: None,
        }));
    }
    if let Some(stop) = scan.and_then(|s| s.stopped) {
        items.push(Item::Note(
            texts.t(words::stop_key(stop)),
            tokens::color::STATE_WARNING,
        ));
    }
    if let Some(s) = scan.filter(|s| s.pending > 0) {
        items.push(Item::Note(
            texts.t_args(
                key::VIDEO_METRICS_PENDING,
                &[("pending", Arg::Number(i64::from(s.pending)))],
            ),
            tokens::color::TEXT_DIM,
        ));
    }
    items.push(setting(
        "scan",
        texts.t(key::VIDEO_METRICS_SCAN_NOW),
        String::new(),
        Target::Scan,
    ));
    if let Some(on) = r.consent {
        items.push(Item::Pair(Pair {
            spot: "consent".to_owned(),
            label: texts.t(key::VIDEO_METRICS_ASK_YOUTUBE),
            value: on_off(on, texts),
            kind: Kind::Setting,
            target: Target::Consent(on),
            tone: (!on).then_some(tokens::color::STATE_WARNING),
            about: Some(key::ABOUT_VIDEO_METRICS_ASK_YOUTUBE),
        }));
        items.push(Item::Note(
            texts.t_with(
                key::VIDEO_METRICS_ASK_YOUTUBE_NOTE,
                &[("hosts", &ronoctua_video_metrics::HOSTS.join(", "))],
            ),
            tokens::color::TEXT_DIM,
        ));
    }
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_CATALOGUE)));
    let count = |n: Option<u64>| n.map(|n| n.to_string()).unwrap_or_default();
    items.push(setting(
        "videos",
        texts.t(key::VIDEO_METRICS_VIDEOS),
        count(r.videos_count),
        Target::Go(Page::Videos),
    ));
    items.push(setting(
        "channels",
        texts.t(key::VIDEO_METRICS_CHANNELS),
        count((r.catalogue == Some(Ok(()))).then_some(r.channels.len() as u64)),
        Target::Go(Page::Channels),
    ));
    items.push(setting(
        "rules",
        texts.t(key::VIDEO_METRICS_RULES),
        String::new(),
        Target::Go(Page::Rules),
    ));
    items.push(setting(
        "weights",
        texts.t(key::VIDEO_METRICS_WEIGHTS),
        String::new(),
        Target::Go(Page::Weights),
    ));
    if r.skipped.iter().any(|(_, n)| *n > 0) {
        items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_LEFT_OUT)));
        for (skip, n) in &r.skipped {
            items.push(Item::Pair(Pair {
                spot: format!("skip:{}", skip.id()),
                label: texts.t(words::skip_key(*skip)),
                value: n.to_string(),
                kind: Kind::Fact,
                target: Target::Nothing,
                tone: None,
                about: None,
            }));
        }
    }
}

/// Every channel, and adding one.
fn channels(items: &mut Vec<Item>, r: &Readings, texts: &ronoctua_i18n::Texts) {
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_CHANNELS)));
    if let Some(Err(why)) = r.catalogue
        && why != Why::NotThere
    {
        items.push(Item::Note(
            texts.t(words::why_key(why)),
            tokens::color::STATE_ERROR,
        ));
    }
    items.push(setting(
        "add-channel",
        texts.t(key::VIDEO_METRICS_ADD_A_CHANNEL),
        String::new(),
        Target::AddChannel,
    ));
    if r.channels.is_empty() {
        items.push(Item::Note(
            texts.t(key::VIDEO_METRICS_NO_CHANNELS),
            tokens::color::TEXT_DIM,
        ));
        return;
    }
    for c in &r.channels {
        let when = date_or_never(c.last_try_at_s, r, texts);
        items.push(Item::Pair(Pair {
            spot: format!("channel:{}", c.id.0),
            label: c.title.clone(),
            value: texts.t_args(
                key::VIDEO_METRICS_CHANNEL_ROW,
                &[
                    ("score", Arg::Number(i64::from(c.score))),
                    (
                        "videos",
                        Arg::Number(i64::try_from(c.videos).unwrap_or(i64::MAX)),
                    ),
                    ("when", Arg::Text(&when)),
                ],
            ),
            kind: Kind::ListedSetting,
            target: Target::Go(Page::Channel(c.id.clone())),
            tone: c.last_error.as_ref().map(|_| tokens::color::STATE_ERROR),
            about: None,
        }));
    }
}

/// One channel: its facts, its score, its Shorts, every rule in its channel
/// version, and removing it.
fn channel(items: &mut Vec<Item>, id: &ChannelId, r: &Readings, texts: &ronoctua_i18n::Texts) {
    let Some(c) = r.channel(id) else {
        items.push(Item::Heading(id.0.clone()));
        refused_or_reading(items, r, texts);
        return;
    };
    items.push(Item::Heading(c.title.clone()));
    items.push(fact(
        "address",
        texts.t(key::VIDEO_METRICS_ADDRESS),
        c.url.clone(),
    ));
    if let Some(h) = &c.handle {
        items.push(fact(
            "handle",
            texts.t(key::VIDEO_METRICS_HANDLE),
            h.clone(),
        ));
    }
    items.push(fact(
        "added",
        texts.t(key::VIDEO_METRICS_ADDED),
        date_or_never(Some(c.added_at_s), r, texts),
    ));
    items.push(fact(
        "last-try",
        texts.t(key::VIDEO_METRICS_LAST_SCAN),
        date_or_never(c.last_try_at_s, r, texts),
    ));
    items.push(fact(
        "reference",
        texts.t(key::VIDEO_METRICS_REFERENCE),
        c.reference_at_s.map_or_else(
            || texts.t(key::VIDEO_METRICS_REFERENCE_NONE),
            |s| crate::chrome::when_text(s, &r.zone, false, texts),
        ),
    ));
    if let Some(e) = &c.last_error {
        items.push(Item::Pair(Pair {
            spot: "last-error".to_owned(),
            label: texts.t(key::VIDEO_METRICS_LAST_ERROR),
            value: e.clone(),
            kind: Kind::Fact,
            target: Target::Copy(e.clone()),
            tone: Some(tokens::color::STATE_ERROR),
            about: None,
        }));
    }
    items.push(setting(
        "its-videos",
        texts.t(key::VIDEO_METRICS_ITS_VIDEOS),
        c.videos.to_string(),
        Target::ChannelVideos(id.clone()),
    ));
    items.push(Item::Pair(Pair {
        spot: "score".to_owned(),
        label: texts.t(key::VIDEO_METRICS_SCORE),
        value: c.score.to_string(),
        kind: Kind::Setting,
        target: Target::ChannelScore(id.clone(), c.score),
        tone: None,
        about: Some(key::ABOUT_VIDEO_METRICS_SCORE),
    }));
    items.push(setting(
        "shorts",
        texts.t(key::VIDEO_METRICS_ITS_SHORTS),
        texts.t(match c.block_shorts {
            None => key::VIDEO_METRICS_SHORTS_FOLLOW,
            Some(true) => key::VIDEO_METRICS_SHORTS_BLOCKED,
            Some(false) => key::VIDEO_METRICS_SHORTS_ALLOWED,
        }),
        Target::ChannelShorts(id.clone(), c.block_shorts),
    ));
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_ITS_RULES)));
    rule_rows(items, &Some(id.clone()), r, texts);
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_REMOVE)));
    items.push(setting(
        "remove-channel",
        texts.t(key::VIDEO_METRICS_REMOVE_CHANNEL),
        String::new(),
        Target::RemoveChannel(id.clone(), c.title.clone()),
    ));
}

/// The global rules.
fn rules(items: &mut Vec<Item>, scope: &Scope, r: &Readings, texts: &ronoctua_i18n::Texts) {
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_RULES)));
    // THE SWITCH SAYS WHAT WAS READ, or that it is being read — never an
    // "off" nobody read, which a turn would write back with the first
    // scan's limits as defaults.
    match r.settings {
        Some(settings) => {
            let block = settings.block_shorts;
            items.push(setting(
                "block-shorts",
                texts.t(key::VIDEO_METRICS_BLOCK_SHORTS),
                on_off(block, texts),
                Target::BlockShorts(block),
            ));
            items.push(Item::Note(
                texts.t(key::VIDEO_METRICS_BLOCK_SHORTS_NOTE),
                tokens::color::TEXT_DIM,
            ));
        }
        None => refused_or_reading(items, r, texts),
    }
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_SCORE)));
    rule_rows(items, scope, r, texts);
}

/// The rule of a single kind in force in `scope`: the scope's own version
/// before the one everywhere — switched off, it is still the one in force
/// there, for it switches the kind off for the channel (`score::in_force`),
/// and the page that switched it off must find it to switch it on again.
fn single_of<'a>(rules: &'a [Rule], single: Single, scope: &Scope) -> Option<&'a Rule> {
    let of_kind = || rules.iter().filter(move |x| single.is(&x.kind));
    of_kind()
        .find(|x| scope.is_some() && x.channel == *scope)
        .or_else(|| of_kind().find(|x| x.channel.is_none()))
}

/// The value a rule's row says: its points — or "off" — and, in a channel's
/// scope, whether it is the channel's own or the one everywhere.
fn rule_value(rule: &Rule, scope: &Scope, texts: &ronoctua_i18n::Texts) -> String {
    let points = if rule.enabled {
        match rule.kind {
            RuleKind::Recency { days } => texts.t_with(
                key::VIDEO_METRICS_RECENCY_VALUE,
                &[
                    ("points", &words::points(i64::from(rule.points), texts)),
                    ("days", &words::days(days, texts)),
                ],
            ),
            _ => words::points(i64::from(rule.points), texts),
        }
    } else {
        texts.t(key::VIDEO_METRICS_OFF)
    };
    if scope.is_none() {
        return points;
    }
    texts.t_with(
        key::VIDEO_METRICS_RULE_IN,
        &[
            ("value", &points),
            (
                "scope",
                &texts.t(if rule.channel.is_some() {
                    key::VIDEO_METRICS_SCOPE_OWN
                } else {
                    key::VIDEO_METRICS_SCOPE_EVERYWHERE
                }),
            ),
        ],
    )
}

/// The rows of every rule in force in a scope: the four single ones, the
/// length bands, the words — each opening its page — and adding a band or
/// words.
fn rule_rows(items: &mut Vec<Item>, scope: &Scope, r: &Readings, texts: &ronoctua_i18n::Texts) {
    let Some(rules) = r.rules_of(scope) else {
        refused_or_reading(items, r, texts);
        return;
    };
    for single in Single::ALL {
        let found = single_of(rules, single, scope);
        let label = match found.map(|x| &x.kind) {
            Some(RuleKind::Preferred { min_s, max_s }) => texts.t_with(
                key::VIDEO_METRICS_PREFERRED_ROW,
                &[("band", &words::band(*min_s, *max_s, texts))],
            ),
            _ => texts.t(words::kind_key(&single.fresh(scope).kind)),
        };
        items.push(Item::Pair(Pair {
            spot: format!("single:{}", single.id()),
            label,
            value: found.map_or_else(
                || texts.t(key::VIDEO_METRICS_NONE),
                |x| rule_value(x, scope, texts),
            ),
            kind: Kind::Setting,
            target: Target::Go(Page::Rule(scope.clone(), Slot::Single(single))),
            tone: None,
            about: None,
        }));
    }
    // THE BANDS: a channel with its own list has that list, and one without
    // has the list everywhere, opened where it lives.
    let bands: Vec<&Rule> = rules
        .iter()
        .filter(|x| matches!(x.kind, RuleKind::Band { .. }))
        .collect();
    for b in &bands {
        let (min_s, max_s) = band_of_rule(b).unwrap_or(BAND);
        items.push(Item::Pair(Pair {
            spot: format!("rule:{}", b.id.map_or(0, |i| i.0)),
            label: texts.t_with(
                key::VIDEO_METRICS_BAND_ROW,
                &[("band", &words::band(min_s, max_s, texts))],
            ),
            value: rule_value(b, scope, texts),
            kind: Kind::Setting,
            target: rule_page_of(b, scope),
            tone: None,
            about: None,
        }));
    }
    items.push(setting(
        "add-band",
        texts.t(if scope.is_some() {
            key::VIDEO_METRICS_ADD_BAND_HERE
        } else {
            key::VIDEO_METRICS_ADD_BAND
        }),
        String::new(),
        Target::AddBand(scope.clone()),
    ));
    if scope.is_some() && !bands.is_empty() && bands.iter().all(|b| b.channel.is_none()) {
        items.push(Item::Note(
            texts.t(key::VIDEO_METRICS_BANDS_NOTE),
            tokens::color::TEXT_DIM,
        ));
    }
    for t in rules
        .iter()
        .filter(|x| matches!(x.kind, RuleKind::Term { .. }))
    {
        let RuleKind::Term { pattern, .. } = &t.kind else {
            continue;
        };
        items.push(Item::Pair(Pair {
            spot: format!("rule:{}", t.id.map_or(0, |i| i.0)),
            label: texts.t_with(key::VIDEO_METRICS_TERM_ROW, &[("words", pattern)]),
            value: rule_value(t, scope, texts),
            kind: Kind::Setting,
            target: rule_page_of(t, scope),
            tone: None,
            about: None,
        }));
    }
    items.push(setting(
        "add-term",
        texts.t(if scope.is_some() {
            key::VIDEO_METRICS_ADD_TERM_HERE
        } else {
            key::VIDEO_METRICS_ADD_TERM
        }),
        String::new(),
        Target::AddTerm(scope.clone()),
    ));
}

/// The page a band or words open on: its own scope's — so a rule from
/// everywhere, listed on a channel's page, opens where it lives.
fn rule_page_of(rule: &Rule, scope: &Scope) -> Target {
    let Some(id) = rule.id else {
        return Target::Nothing;
    };
    let lives = if rule.channel.is_some() {
        scope.clone()
    } else {
        None
    };
    Target::Go(Page::Rule(lives, Slot::Id(id)))
}

/// One rule's page: every value it has, whether it counts, and removing it.
fn rule_page(
    items: &mut Vec<Item>,
    scope: &Scope,
    slot: &Slot,
    r: &Readings,
    texts: &ronoctua_i18n::Texts,
) {
    let Some(rules) = r.rules_of(scope) else {
        refused_or_reading(items, r, texts);
        return;
    };
    let found = match slot {
        Slot::Single(s) => single_of(rules, *s, scope).cloned(),
        Slot::Id(id) => rules.iter().find(|x| x.id == Some(*id)).cloned(),
    };
    // IN A CHANNEL'S SCOPE A RULE FROM EVERYWHERE IS EDITED AS THE CHANNEL'S
    // OWN: the first change makes its version, starting from these values.
    let inherited = scope.is_some() && found.as_ref().is_some_and(|x| x.channel.is_none());
    let rule = match (found, slot) {
        (Some(x), _) if inherited => Rule {
            id: None,
            channel: scope.clone(),
            ..x
        },
        (Some(x), _) => x,
        (None, Slot::Single(s)) => s.fresh(scope),
        (None, Slot::Id(_)) => {
            items.push(Item::Heading(texts.t(key::VIDEO_METRICS_RULES)));
            items.push(Item::Note(
                texts.t(words::why_key(Why::NotFound)),
                tokens::color::TEXT_DIM,
            ));
            return;
        }
    };
    let title = match &rule.kind {
        RuleKind::Term { pattern, .. } => {
            texts.t_with(key::VIDEO_METRICS_TERM_ROW, &[("words", pattern)])
        }
        kind => texts.t(words::kind_key(kind)),
    };
    items.push(Item::Heading(title));
    if let Some(channel) = scope.as_ref().and_then(|id| r.channel(id)) {
        items.push(Item::Note(
            texts.t_with(
                if inherited || rule.id.is_none() {
                    key::VIDEO_METRICS_RULE_INHERITED_NOTE
                } else if !rule.enabled && rule.kind.is_single() {
                    // ITS OWN, SWITCHED OFF: the kind counts for nothing on
                    // this channel, and removing it brings the one
                    // everywhere back.
                    key::VIDEO_METRICS_RULE_OWN_OFF_NOTE
                } else {
                    key::VIDEO_METRICS_RULE_OWN_NOTE
                },
                &[("name", &channel.title)],
            ),
            tokens::color::TEXT_DIM,
        ));
    }
    let points = |label: &'static str| {
        Item::Pair(Pair {
            spot: "points".to_owned(),
            label: texts.t(label),
            value: words::points(i64::from(rule.points), texts),
            kind: Kind::Setting,
            target: Target::Points(rule.clone()),
            tone: None,
            about: Some(key::ABOUT_VIDEO_METRICS_POINTS),
        })
    };
    match &rule.kind {
        RuleKind::Recency { days } => {
            items.push(points(key::VIDEO_METRICS_RECENCY_POINTS));
            items.push(setting(
                "days",
                texts.t(key::VIDEO_METRICS_RECENCY_DAYS),
                words::days(*days, texts),
                Target::Days(rule.clone()),
            ));
        }
        RuleKind::Short | RuleKind::Live => items.push(points(key::VIDEO_METRICS_RULE_POINTS)),
        RuleKind::Band { min_s, max_s } | RuleKind::Preferred { min_s, max_s } => {
            items.push(setting(
                "band",
                texts.t(key::VIDEO_METRICS_RULE_LENGTH),
                words::band(*min_s, *max_s, texts),
                Target::Band(rule.clone()),
            ));
            items.push(points(key::VIDEO_METRICS_RULE_POINTS));
        }
        RuleKind::Term {
            fields,
            pattern,
            how,
        } => {
            items.push(setting(
                "words",
                texts.t(key::VIDEO_METRICS_TERM_WORDS),
                pattern.clone(),
                Target::Words(rule.clone()),
            ));
            items.push(setting(
                "match",
                texts.t(key::VIDEO_METRICS_TERM_MATCH),
                texts.t(words::match_key(*how)),
                Target::Match(rule.clone()),
            ));
            for field in words::FIELDS {
                let Some(label) = words::field_key(field) else {
                    continue;
                };
                items.push(setting(
                    &format!("field:{}", field.0),
                    texts.t(label),
                    on_off(fields.has(field), texts),
                    Target::Field(rule.clone(), field),
                ));
            }
            items.push(points(key::VIDEO_METRICS_RULE_POINTS));
        }
    }
    items.push(setting(
        "on",
        texts.t(key::VIDEO_METRICS_RULE_ON),
        on_off(rule.enabled, texts),
        Target::On(rule.clone()),
    ));
    if let Some(id) = rule.id {
        items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_REMOVE)));
        items.push(setting(
            "remove-rule",
            texts.t(key::VIDEO_METRICS_REMOVE_RULE),
            String::new(),
            Target::RemoveRule(id),
        ));
    }
}

/// The rating weights.
fn weights(items: &mut Vec<Item>, r: &Readings, texts: &ronoctua_i18n::Texts) {
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_WEIGHTS)));
    items.push(Item::Note(
        texts.t(key::VIDEO_METRICS_WEIGHTS_NOTE),
        tokens::color::TEXT_DIM,
    ));
    let Some(w) = r.weights else {
        refused_or_reading(items, r, texts);
        return;
    };
    for rating in Rating::ALL {
        items.push(Item::Pair(Pair {
            spot: format!("weight:{}", rating.id()),
            label: texts.t(words::rating_key(rating)),
            value: words::points(i64::from(w.of(rating)), texts),
            kind: Kind::Setting,
            target: Target::Weight(rating),
            tone: None,
            about: Some(key::ABOUT_VIDEO_METRICS_POINTS),
        }));
    }
}

/// One video: its picture, every field, the person's own, and what can be
/// done with it.
fn video(items: &mut Vec<Item>, id: &VideoId, r: &Readings, texts: &ronoctua_i18n::Texts) {
    let Some(v) = r.video_of(id) else {
        items.push(Item::Heading(id.0.clone()));
        if r.video
            .as_ref()
            .is_some_and(|(read, v)| read == id && v.is_none())
        {
            items.push(Item::Note(
                texts.t(words::why_key(Why::NotFound)),
                tokens::color::TEXT_DIM,
            ));
        } else {
            refused_or_reading(items, r, texts);
        }
        return;
    };
    let row = &v.row;
    items.push(Item::Heading(row.title.clone()));
    if let Some(file) = &row.thumb_file {
        items.push(Item::Figure(file.clone()));
    }
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_THE_VIDEO)));
    items.push(setting(
        "channel",
        texts.t(key::VIDEO_METRICS_CHANNEL),
        row.channel.clone(),
        Target::Go(Page::Channel(row.channel_id.clone())),
    ));
    items.push(fact(
        "published",
        texts.t(key::VIDEO_METRICS_PUBLISHED),
        row.published_at_s
            .map(|s| crate::chrome::when_text(s, &r.zone, false, texts))
            .unwrap_or_default(),
    ));
    if let Some(d) = row.duration_s {
        items.push(fact(
            "duration",
            texts.t(key::VIDEO_METRICS_DURATION),
            words::length(d),
        ));
    }
    items.push(fact(
        "kind",
        texts.t(key::VIDEO_METRICS_KIND),
        texts.t(if v.live {
            key::VIDEO_METRICS_KIND_LIVE
        } else if row.is_short {
            key::VIDEO_METRICS_KIND_SHORT
        } else {
            key::VIDEO_METRICS_KIND_LONG
        }),
    ));
    items.push(fact(
        "score",
        texts.t(key::VIDEO_METRICS_SCORE),
        words::points(row.score, texts),
    ));
    items.push(fact(
        "static",
        texts.t(key::VIDEO_METRICS_ITS_POINTS),
        words::points(v.static_points, texts),
    ));
    if let Some((w, h)) = v.size_px {
        items.push(fact(
            "size",
            texts.t(key::VIDEO_METRICS_SIZE),
            texts.t_args(
                key::VIDEO_METRICS_SIZE_VALUE,
                &[
                    ("width", Arg::Number(i64::from(w))),
                    ("height", Arg::Number(i64::from(h))),
                ],
            ),
        ));
    }
    let languages: Vec<&str> = [v.languages.0.as_deref(), v.languages.1.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    if !languages.is_empty() {
        items.push(fact(
            "languages",
            texts.t(key::VIDEO_METRICS_LANGUAGES),
            languages.join(", "),
        ));
    }
    if v.age_limit > 0 {
        items.push(fact(
            "age-limit",
            texts.t(key::VIDEO_METRICS_AGE_LIMIT),
            v.age_limit.to_string(),
        ));
    }
    if !v.captions.is_empty() {
        items.push(fact(
            "captions",
            texts.t(key::VIDEO_METRICS_CAPTIONS),
            v.captions.join(", "),
        ));
    }
    if !v.categories.is_empty() {
        items.push(fact(
            "categories",
            texts.t(key::VIDEO_METRICS_CATEGORIES),
            v.categories.join(", "),
        ));
    }
    if !v.chapters.is_empty() {
        items.push(fact(
            "chapters",
            texts.t(key::VIDEO_METRICS_CHAPTERS),
            v.chapters.len().to_string(),
        ));
    }
    if let Some(n) = v.views {
        items.push(fact(
            "views",
            texts.t(key::VIDEO_METRICS_VIEWS),
            n.to_string(),
        ));
    }
    items.push(fact(
        "first-seen",
        texts.t(key::VIDEO_METRICS_FIRST_SEEN),
        crate::chrome::when_text(v.first_seen_at_s, &r.zone, false, texts),
    ));
    items.push(fact(
        "address",
        texts.t(key::VIDEO_METRICS_ADDRESS),
        row.url.clone(),
    ));
    if !v.tags.is_empty() {
        items.push(fact(
            "tags",
            texts.t(key::VIDEO_METRICS_TAGS),
            v.tags.join(", "),
        ));
    }
    if !v.description.trim().is_empty() {
        items.push(Item::Heading(texts.t(key::VIDEO_METRICS_DESCRIPTION)));
        items.push(Item::Note(v.description.clone(), Kind::Fact.tones(false).1));
    }
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_YOURS)));
    items.push(setting(
        "rating",
        texts.t(key::VIDEO_METRICS_RATING),
        texts
            .t(v.rating
                .map_or(key::VIDEO_METRICS_RATING_NONE, words::rating_key)),
        Target::Rating(id.clone(), v.rating),
    ));
    let tags = v.second_tags.join(", ");
    items.push(setting(
        "second-tags",
        texts.t(key::VIDEO_METRICS_YOUR_TAGS),
        if tags.is_empty() {
            texts.t(key::VIDEO_METRICS_NONE)
        } else {
            tags.clone()
        },
        Target::SecondTags(id.clone(), tags),
    ));
    items.push(Item::Pair(Pair {
        spot: "second-description".to_owned(),
        label: texts.t(key::VIDEO_METRICS_YOUR_DESCRIPTION),
        value: if v.second_description.trim().is_empty() {
            texts.t(key::VIDEO_METRICS_NONE)
        } else {
            String::new()
        },
        kind: Kind::Setting,
        target: Target::SecondDescription(id.clone(), v.second_description.clone()),
        tone: None,
        about: Some(key::ABOUT_VIDEO_METRICS_SECOND_DESCRIPTION),
    }));
    if !v.second_description.trim().is_empty() {
        items.push(Item::Note(
            v.second_description.clone(),
            Kind::Fact.tones(false).1,
        ));
    }
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_WITH_IT)));
    items.push(setting(
        "player",
        texts.t(key::CMD_VIDEO_METRICS_PLAYER),
        String::new(),
        Target::Player(id.clone()),
    ));
    items.push(setting(
        "kiosk",
        texts.t(key::CMD_VIDEO_METRICS_KIOSK),
        String::new(),
        Target::Kiosk(id.clone()),
    ));
    items.push(setting(
        "download",
        texts.t(key::CMD_VIDEO_METRICS_DOWNLOAD),
        String::new(),
        Target::Download(id.clone()),
    ));
    items.push(setting(
        "hide",
        texts.t(key::VIDEO_METRICS_HIDE_ROW),
        texts.t(if row.hidden {
            key::VIDEO_METRICS_HIDDEN
        } else {
            key::VIDEO_METRICS_SHOWN
        }),
        Target::Hide(id.clone(), row.hidden),
    ));
    items.push(setting(
        "delete",
        texts.t(key::CMD_VIDEO_METRICS_DELETE),
        String::new(),
        Target::Delete(id.clone()),
    ));
}

// ---------------------------------------------------------------------------
// Placing: the items become rows.
// ---------------------------------------------------------------------------

/// One piece of text, elided to `room`.
fn piece(at: u16, text: String, color: tokens::Color, room: u16) -> Field {
    Field {
        at,
        text: ronoctua_render::layout::elide_end(&text, room),
        color,
        hit: FilesHit::Nothing,
        explain: None,
        ink: Ink::Grid,
    }
}

/// A row with no more than its fields.
fn plain(fields: Vec<Field>, heading: bool, spot: String) -> Row {
    Row {
        fields,
        heading,
        target: Target::Nothing,
        spot,
        continues: false,
        cells: Vec::new(),
        thumbs: Vec::new(),
        caret: None,
        about: None,
    }
}

/// The items, placed: the values of the whole page in one column after its
/// widest label (`chrome::section_column`), labels wrapped and never cut,
/// notes wrapped, air before every title but the first — and the cursor's
/// row in the cursor's colour.
fn lay_out(items: Vec<Item>, left: u16, room: u16, at: &str) -> Vec<Row> {
    let end = left + room;
    // THE COLUMN IS MEASURED ON THE ROWS THAT HAVE A VALUE: a row that is
    // only a label — "Delete it for good: it never comes back" — has nothing
    // to align, and would push every value on the page to the right.
    let pairs: Vec<(u16, u16, u16)> = items
        .iter()
        .filter_map(|i| match i {
            Item::Pair(p) if !p.value.is_empty() => Some((
                left,
                ronoctua_render::layout::width(&p.label),
                ronoctua_render::layout::width(&p.value),
            )),
            _ => None,
        })
        .collect();
    let column = crate::chrome::section_column(left, room, &pairs);
    let mut out: Vec<Row> = Vec::new();
    let mut n = 0usize;
    let mut spot = |kind: &str| {
        n += 1;
        format!("{kind}#{n}")
    };
    for item in items {
        match item {
            Item::Heading(text) => {
                if !out.is_empty() {
                    out.push(plain(crate::chrome::AIR, true, spot("air")));
                }
                // A TITLE IS DATA WHEN IT IS A NAME (a channel, a video),
                // and it wraps like one: never cut.
                for line in crate::chrome::note_lines(&text, room) {
                    out.push(plain(
                        vec![piece(left, line, tokens::color::TEXT_HEADING, room)],
                        true,
                        spot("title"),
                    ));
                }
            }
            Item::Note(text, color) => {
                for paragraph in text.split('\n') {
                    for line in crate::chrome::note_lines(paragraph, room) {
                        out.push(plain(
                            if line.is_empty() {
                                Vec::new()
                            } else {
                                vec![piece(left, line, color, room)]
                            },
                            true,
                            spot("note"),
                        ));
                    }
                }
            }
            Item::Pair(p) => {
                let focused = p.spot == at;
                let lines = crate::chrome::aligned_pair(
                    left, column, end, &p.label, &p.value, p.kind, focused,
                );
                for (k, fields) in lines.into_iter().enumerate() {
                    let fields = if k == 0 {
                        crate::chrome::tinted(fields, p.tone, focused)
                    } else {
                        fields
                    };
                    out.push(Row {
                        fields,
                        heading: false,
                        target: if k == 0 {
                            p.target.clone()
                        } else {
                            Target::Nothing
                        },
                        spot: if k == 0 { p.spot.clone() } else { spot("rest") },
                        continues: k > 0,
                        cells: Vec::new(),
                        thumbs: Vec::new(),
                        caret: None,
                        about: p.about,
                    });
                }
            }
            Item::Answer { yes, label } => {
                let spot = answer_spot(yes).to_owned();
                let focused = spot == at;
                let colour = Kind::Setting.tones(focused).0;
                out.push(Row {
                    fields: vec![piece(left, label, colour, room)],
                    heading: false,
                    target: Target::Answer(yes),
                    spot,
                    continues: false,
                    cells: Vec::new(),
                    thumbs: Vec::new(),
                    caret: None,
                    about: None,
                });
            }
            Item::Figure(file) => {
                let lines = tokens::space::VIDEOMETRICS_THUMBLINES;
                let cells = tokens::space::VIDEOMETRICS_THUMBCELLS.min(room);
                for line in 0..lines {
                    let mut row = plain(Vec::new(), true, spot("figure"));
                    row.thumbs.push(Thumb {
                        line,
                        lines,
                        left,
                        cells,
                        file: file.clone(),
                    });
                    out.push(row);
                }
            }
            Item::Entry(t) => {
                let focused = t.spot == at;
                let (label, value) = if t.hidden {
                    Kind::Disabled.tones(focused)
                } else {
                    Kind::ListedSetting.tones(focused)
                };
                let lines = crate::chrome::note_lines(&t.title, room)
                    .into_iter()
                    .map(|l| (l, label))
                    .chain(
                        t.meta
                            .iter()
                            .flat_map(|m| crate::chrome::note_lines(m, room))
                            .map(|l| (l, value)),
                    );
                for (k, (line, colour)) in lines.enumerate() {
                    let mut row = plain(
                        vec![piece(left, line, colour, room)],
                        false,
                        if k == 0 { t.spot.clone() } else { spot("rest") },
                    );
                    if k == 0 {
                        row.target = t.target.clone();
                    } else {
                        row.continues = true;
                    }
                    out.push(row);
                }
            }
            Item::Shelf(tiles, grid) => shelf(&mut out, &tiles, grid, left, at, &mut spot),
        }
    }
    out
}

/// One shelf of the grid: the pictures' lines, the titles' two, the line
/// that says what each scores, the one that says when, and a line of air.
/// Each tile sits [`TILE_AIR`] inside its cell, so the hover that lights the
/// cell has the same air on both sides of it. The FIRST line carries the
/// cells a click takes — the box the hover lights, never the air between two
/// — and every line after it continues it.
fn shelf(
    out: &mut Vec<Row>,
    tiles: &[Tile],
    grid: ronoctua_design::grid::Grid,
    left: u16,
    at: &str,
    spot: &mut impl FnMut(&str) -> String,
) {
    let lines = tokens::space::VIDEOMETRICS_THUMBLINES;
    let title_lines = tokens::space::VIDEOMETRICS_TITLELINES;
    let width = tokens::space::VIDEOMETRICS_THUMBCELLS;
    let first = out.len();
    // WHERE EACH CELL STARTS, and its tile one air inside it.
    let placed: Vec<(u16, &Tile)> = tiles
        .iter()
        .enumerate()
        .filter_map(|(i, t)| grid.at(i).map(|(_, column)| (left + column, t)))
        .collect();
    let inside = |from: u16| from + TILE_AIR;
    let total = SHELF_LINES;
    for line in 0..total {
        let mut row = plain(Vec::new(), false, spot("shelf"));
        row.continues = line > 0;
        for (from, t) in &placed {
            let focused = t.spot == at;
            if line < lines {
                if let Some(file) = &t.file {
                    row.thumbs.push(Thumb {
                        line,
                        lines,
                        left: inside(*from),
                        cells: width,
                        file: file.clone(),
                    });
                } else if line == lines / 2 {
                    // NO PICTURE YET: the glyph of a video in its place,
                    // centred, until the scan fetches one.
                    let glyph = tokens::icon::VIDEO_NO_THUMB.to_string();
                    row.fields.push(piece(
                        inside(*from) + width / 2,
                        glyph,
                        tokens::color::TEXT_DIM,
                        1,
                    ));
                }
            } else if line < lines + title_lines {
                let k = usize::from(line - lines);
                let wrapped = crate::chrome::note_lines(&t.title, width);
                let text = if k + 1 == usize::from(title_lines) && wrapped.len() > k + 1 {
                    // THE LAST LINE OF THE TITLE carries the rest, cut: a
                    // tile's title is two lines, and its page has it whole.
                    ronoctua_render::layout::elide_end(&wrapped[k..].join(" "), width)
                } else {
                    wrapped.get(k).cloned().unwrap_or_default()
                };
                if !text.is_empty() {
                    let (label, _) = if t.hidden {
                        Kind::Disabled.tones(focused)
                    } else {
                        Kind::ListedSetting.tones(focused)
                    };
                    row.fields.push(piece(inside(*from), text, label, width));
                }
            } else if let Some(meta) = t.meta.get(usize::from(line - lines - title_lines)) {
                // THE SCORE, THEN THE AGE, a line each.
                row.fields.push(piece(
                    inside(*from),
                    meta.clone(),
                    Kind::ListedFact.tones(focused).1,
                    width,
                ));
            }
        }
        if line == 0 {
            row.cells = placed
                .iter()
                .map(|(from, t)| Cell {
                    from: *from,
                    width: TILE,
                    target: t.target.clone(),
                    spot: t.spot.clone(),
                })
                .collect();
            row.spot = placed
                .first()
                .map_or_else(|| spot("shelf"), |(_, t)| format!("shelf:{}", t.spot));
        }
        out.push(row);
    }
    // A CLICK ON ANY LINE OF THE SHELF is read at its first (`Row::owner`),
    // where the cells are.
    debug_assert!(out.len() - first == usize::from(total));
}

/// The rows of the description being edited: its title, each drawn line —
/// the caret on its line — and, while `Esc` asks, the question under it,
/// "no" first. Every line is a row a click is read back from.
fn text_rows(
    area: &crate::text_area::TextArea,
    left: u16,
    room: u16,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Row> {
    let mut out = vec![plain(
        vec![piece(
            left,
            texts.t(key::VIDEO_METRICS_YOUR_DESCRIPTION),
            tokens::color::TEXT_HEADING,
            room,
        )],
        true,
        "text-title".to_owned(),
    )];
    let text = area.text();
    let lines: Vec<&str> = text.split('\n').collect();
    let (caret_at, caret_cell) = area.caret_drawn(room);
    let value = Kind::Setting.tones(false).1;
    for (n, d) in area.drawn(room).into_iter().enumerate() {
        let said = lines
            .get(d.line)
            .and_then(|l| l.get(d.from..d.to))
            .unwrap_or_default()
            .trim_end_matches(' ');
        let mut row = plain(
            vec![piece(left, said.to_owned(), value, room)],
            false,
            format!("text-line-{n}"),
        );
        row.target = Target::Text(d);
        row.caret = (n == caret_at && area.asking().is_none()).then_some(left + caret_cell);
        out.push(row);
    }
    if let Some(yes) = area.asking() {
        out.push(plain(crate::chrome::AIR, true, "text-air".to_owned()));
        for line in crate::chrome::note_lines(&texts.t(key::ASK_TEXT_AREA_DISCARD), room) {
            out.push(plain(
                vec![piece(left, line, tokens::color::TEXT_SECONDARY, room)],
                true,
                "text-asked".to_owned(),
            ));
        }
        for (answer, k) in [(false, key::TEXT_AREA_NO), (true, key::TEXT_AREA_YES)] {
            let colour = Kind::Setting.tones(answer == yes).0;
            out.push(plain(
                vec![piece(left, texts.t(k), colour, room)],
                false,
                format!("text-answer-{answer}"),
            ));
        }
    }
    out
}

/// The row the cursor of the description being edited is on — the caret's,
/// or the question's answer — which the door keeps in sight.
#[must_use]
pub fn text_cursor(rows: &[Row]) -> Option<usize> {
    rows.iter().position(|r| r.caret.is_some()).or_else(|| {
        rows.iter().position(|r| {
            r.spot.starts_with("text-answer-")
                && r.fields
                    .first()
                    .is_some_and(|f| f.color == tokens::color::STATE_ACCENT)
        })
    })
}

/// What a click on the text being edited landed on, at `column` (a field's
/// `at`): a line, or an answer of its question.
#[must_use]
pub fn text_hit(rows: &[Row], n: usize, column: u16) -> Option<crate::text_area::Hit> {
    let row = rows.get(n)?;
    match (&row.target, row.spot.as_str()) {
        (Target::Text(at), _) => Some(crate::text_area::Hit::Text {
            at: *at,
            cell: column.saturating_sub(row.fields.first().map_or(0, |f| f.at)),
        }),
        (_, "text-answer-false") => Some(crate::text_area::Hit::Answer(false)),
        (_, "text-answer-true") => Some(crate::text_area::Hit::Answer(true)),
        _ => None,
    }
}

impl View {
    /// A click on the description being edited — the caret placed, or an
    /// answer of its question chosen, then given. Answers whether the text
    /// was left.
    pub fn text_clicked(&mut self, hit: crate::text_area::Hit) -> bool {
        let Some((_, area)) = self.editing.as_mut() else {
            return false;
        };
        if area.clicked(hit) == crate::text_area::Step::Discard {
            self.editing = None;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_keymap::{Key, Sym};

    /// A moment the fixtures are written against: 2026-09-30, noon.
    const NOW: i64 = 1_790_769_600;
    /// The width most tests draw at.
    const COLUMNS: u16 = 120;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    fn channel(n: u8) -> Channel {
        Channel {
            id: ChannelId(format!("UC{n:022}")),
            handle: Some(format!("@channel{n}")),
            title: format!("Channel {n}"),
            url: format!("https://www.youtube.com/@channel{n}"),
            avatar_url: None,
            score: 5000,
            added_at_s: NOW - 40 * 86_400,
            reference_at_s: Some(NOW - 86_400),
            last_try_at_s: Some(NOW - 3600),
            last_error: None,
            block_shorts: None,
            videos: 10,
        }
    }

    fn ranked(n: u8, of: &Channel) -> RankedVideo {
        RankedVideo {
            id: VideoId(format!("video{n:06}")),
            url: format!("https://www.youtube.com/watch?v=video{n:06}"),
            title: format!("A video, number {n}"),
            channel: of.title.clone(),
            channel_id: of.id.clone(),
            score: 9000 - i64::from(n) * 100,
            thumb_file: None,
            duration_s: Some(600),
            published_at_s: Some(NOW - i64::from(n) * 86_400),
            is_short: false,
            hidden: false,
        }
    }

    fn whole(row: RankedVideo) -> Video {
        Video {
            row,
            description: "What the video is about.".to_owned(),
            first_seen_at_s: NOW - 3600,
            live: false,
            size_px: Some((1920, 1080)),
            languages: (Some("en".to_owned()), None),
            age_limit: 0,
            chapters: Vec::new(),
            captions: vec!["en".to_owned()],
            categories: vec!["Education".to_owned()],
            tags: vec!["nix".to_owned()],
            second_tags: Vec::new(),
            second_description: String::new(),
            rating: None,
            static_points: 500,
            views: None,
        }
    }

    /// The rules everywhere, as the catalogue starts them, and a band and a
    /// term the person added.
    fn global_rules() -> Vec<Rule> {
        vec![
            Rule {
                id: Some(RuleId(1)),
                channel: None,
                kind: RuleKind::Recency { days: 30 },
                points: 2000,
                enabled: true,
            },
            Rule {
                id: Some(RuleId(2)),
                channel: None,
                kind: RuleKind::Short,
                points: -1000,
                enabled: true,
            },
            Rule {
                id: Some(RuleId(3)),
                channel: None,
                kind: RuleKind::Band {
                    min_s: 240,
                    max_s: 1200,
                },
                points: 300,
                enabled: true,
            },
            Rule {
                id: Some(RuleId(4)),
                channel: None,
                kind: RuleKind::Term {
                    fields: Fields::TITLE,
                    pattern: "nix".to_owned(),
                    how: TermMatch::Word,
                },
                points: 700,
                enabled: true,
            },
        ]
    }

    /// Everything the catalogue could have said, for every page.
    fn readings() -> Readings {
        let (a, b) = (channel(1), channel(2));
        let rows: Vec<RankedVideo> = (0..10).map(|n| ranked(n, &a)).collect();
        Readings {
            catalogue: Some(Ok(())),
            last_scan: Some(Some(ScanRecord {
                started_at_s: NOW - 7200,
                ended_at_s: Some(NOW - 7000),
                channels: 2,
                channels_failed: 0,
                added: 10,
                skipped: 5,
                detailed: 10,
                pending: 0,
                thumbs: 10,
                stopped: None,
                said: None,
            })),
            videos_count: Some(10),
            skipped: Skip::ALL.iter().map(|s| (*s, 1)).collect(),
            consent: Some(true),
            channels: vec![a, b],
            rules: Some((None, global_rules())),
            settings: Some(Settings::default()),
            weights: Some(Weights::default()),
            listed: Some(Listed {
                query: Query {
                    filter: Filter::default(),
                    page: 0,
                    pictures: false,
                },
                total: 10,
                rows: rows.clone(),
            }),
            video: Some((rows[0].id.clone(), Some(whole(rows[0].clone())))),
            now_s: NOW,
            transfers_keys: "SUPER t t".to_owned(),
            ..Readings::default()
        }
    }

    /// The row whose spot is `spot`.
    fn row_of(rows: &[Row], spot: &str) -> usize {
        rows.iter()
            .position(|r| r.spot == spot)
            .unwrap_or_else(|| panic!("no row `{spot}` among {:?}", spots(rows)))
    }

    fn spots(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|r| r.spot.as_str()).collect()
    }

    /// The cursor put on the row `spot`, by a first click.
    fn onto(v: &mut View, r: &Readings, spot: &str) -> Vec<Row> {
        let t = texts("en-GB");
        let rows = v.rows(r, COLUMNS, &t);
        let n = row_of(&rows, spot);
        v.clicked(n, 0, &rows);
        v.rows(r, COLUMNS, &t)
    }

    /// A press, with the rows as drawn.
    fn press(v: &mut View, r: &Readings, p: Press) -> Errand {
        let rows = v.rows(r, COLUMNS, &texts("en-GB"));
        v.press(p, &rows, r)
    }

    fn every_page() -> Vec<Page> {
        let a = channel(1).id;
        vec![
            Page::Home,
            Page::Channels,
            Page::Channel(a.clone()),
            Page::Rules,
            Page::Rule(None, Slot::Single(Single::Recency)),
            Page::Rule(None, Slot::Single(Single::Preferred)),
            Page::Rule(None, Slot::Id(RuleId(3))),
            Page::Rule(None, Slot::Id(RuleId(4))),
            Page::Rule(Some(a), Slot::Single(Single::Short)),
            Page::Weights,
            Page::Videos,
            Page::Video(VideoId("video000000".to_owned())),
        ]
    }

    /// FAILS IF: a page draws a key that has no words (`[key]` on screen), a
    /// field reaches past the region's right edge at a narrow, a middling or a
    /// wide door — in either language — or the cursor is drawn on no row.
    #[test]
    fn every_page_draws_its_words_inside_the_door() {
        let mut r = readings();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for page in every_page() {
                for layout in [Layout::List, Layout::Grid] {
                    let (mut v, _) = View::fresh(page.clone());
                    v.layout = layout;
                    // A CHANNEL'S PAGE reads its own scope's rules.
                    r.rules = Some(match &page {
                        Page::Channel(id) | Page::Rule(Some(id), _) => {
                            (Some(id.clone()), global_rules())
                        }
                        _ => (None, global_rules()),
                    });
                    for columns in [44, 80, 200] {
                        let rows = v.rows(&r, columns, &t);
                        assert!(!rows.is_empty(), "{language} {page:?}: nothing drawn");
                        for row in &rows {
                            for f in &row.fields {
                                assert!(
                                    !f.text.contains("[video-metrics") && !f.text.contains("[ask-"),
                                    "{language} {page:?}: a key with no words: {}",
                                    f.text
                                );
                                let end = f.at + ronoctua_render::layout::width(&f.text);
                                assert!(
                                    end <= columns,
                                    "{language} {page:?} at {columns}: `{}` ends at {end}",
                                    f.text
                                );
                            }
                        }
                        let at = v.cursor(&rows);
                        assert!(rows[at].stops(), "{page:?}: the cursor rests on a title");
                    }
                }
            }
        }
    }

    /// FAILS IF: a page just opened draws no cursor — its first row that acts
    /// or informs is where a key would act, and the cursor is a colour.
    #[test]
    fn a_page_just_opened_draws_its_cursor() {
        let mut r = readings();
        let t = texts("en-GB");
        for page in every_page() {
            // A CHANNEL'S PAGE reads its own scope's rules.
            r.rules = Some(match &page {
                Page::Channel(id) | Page::Rule(Some(id), _) => (Some(id.clone()), global_rules()),
                _ => (None, global_rules()),
            });
            let (v, _) = View::fresh(page.clone());
            let rows = v.rows(&r, COLUMNS, &t);
            let at = v.cursor(&rows);
            let lit = [
                Kind::Fact,
                Kind::Setting,
                Kind::ListedSetting,
                Kind::ListedFact,
            ]
            .map(|k| k.tones(true).0);
            assert!(
                rows[at]
                    .fields
                    .first()
                    .is_some_and(|f| lit.contains(&f.color)),
                "{page:?}: the row the cursor is on, {:?}, is not in the cursor's colour",
                rows[at].spot
            );
        }
    }

    /// FAILS IF: the cursor follows an INDEX when the scanner reorders the
    /// list — it would land on another video under the person's hand.
    #[test]
    fn the_cursor_keeps_its_video_when_the_list_reorders() {
        let mut r = readings();
        let (mut v, _) = View::fresh(Page::Videos);
        let rows = onto(&mut v, &r, "video:video000003");
        assert_eq!(rows[v.cursor(&rows)].spot, "video:video000003");
        if let Some(l) = r.listed.as_mut() {
            l.rows.reverse();
        }
        let t = texts("en-GB");
        let rows = v.rows(&r, COLUMNS, &t);
        v.settle(&rows);
        assert_eq!(rows[v.cursor(&rows)].spot, "video:video000003");
        // AND A VIDEO THAT LEFT THE LIST leaves the cursor on a row near it.
        if let Some(l) = r.listed.as_mut() {
            l.rows.retain(|x| x.id.0 != "video000003");
        }
        let rows = v.rows(&r, COLUMNS, &t);
        v.settle(&rows);
        assert!(rows[v.cursor(&rows)].stops());
    }

    /// FAILS IF: deleting a video is not asked, the question is not drawn
    /// UNDER the row it was asked from with its "no" first and the cursor on
    /// it, `RET` there deletes, or the "yes" does not — and leaves the
    /// video's page, which is about nothing — or the page the person is
    /// back on is not read again once the deletion is done: it would list
    /// the deleted video, and the feed would go on reading the page gone.
    #[test]
    fn a_deletion_is_asked_with_no_first() {
        let r = readings();
        let id = VideoId("video000000".to_owned());
        let (mut v, _) = View::fresh(Page::Videos);
        onto(&mut v, &r, &video_spot(&id));
        press(&mut v, &r, Press::Enter);
        onto(&mut v, &r, "delete");
        assert_eq!(press(&mut v, &r, Press::Delete), Errand::Nothing);
        let t = texts("en-GB");
        let rows = v.rows(&r, COLUMNS, &t);
        let no = row_of(&rows, "answer-no");
        let asked_from = row_of(&rows, "delete");
        assert!(
            asked_from < no && no <= asked_from + 4,
            "the question is not under the row it was asked from: {:?}",
            spots(&rows)
        );
        assert!(no < row_of(&rows, "answer-yes"), "the no is not first");
        assert_eq!(v.cursor(&rows), no, "the cursor was not born on the no");
        assert_eq!(
            rows[no].fields[0].color,
            Kind::Setting.tones(true).0,
            "the cursor is a colour"
        );
        assert_eq!(press(&mut v, &r, Press::Enter), Errand::Nothing);
        assert_eq!(v.page(), &Page::Video(id.clone()), "a no left the page");
        let rows = v.rows(&r, COLUMNS, &t);
        assert_eq!(
            rows[v.cursor(&rows)].spot,
            "delete",
            "a no did not give the cursor back to the row it was asked from"
        );
        press(&mut v, &r, Press::Delete);
        press(&mut v, &r, Press::Down);
        assert_eq!(
            press(&mut v, &r, Press::Enter),
            Errand::Edit(Edit::Delete(id.clone()))
        );
        assert_eq!(v.page(), &Page::Videos, "the deleted video's page stayed");
        let (said, next) = v.took(Done::Edited(Edit::Delete(id), Ok(())), &r, &t);
        assert!(matches!(said, Some(Said::Normal(_))));
        assert!(
            matches!(
                &next,
                Errand::Look(Look {
                    page: Page::Videos,
                    query: Some(_)
                })
            ),
            "the list the person is back on was not read again: {next:?}"
        );
        // AND ONCE IT IS, the cursor is on the video that took its place,
        // not on the page's first row.
        let mut r = r;
        if let Some(l) = r.listed.as_mut() {
            l.rows.retain(|x| x.id.0 != "video000000");
        }
        let rows = v.rows(&r, COLUMNS, &t);
        v.settle(&rows);
        assert_eq!(rows[v.cursor(&rows)].spot, "video:video000001");
    }

    /// FAILS IF: hiding is not asked, or showing a hidden video again is —
    /// nothing is lost by showing, so nothing is asked.
    #[test]
    fn hiding_is_asked_and_showing_is_not() {
        let mut r = readings();
        let id = VideoId("video000000".to_owned());
        let (mut v, _) = View::fresh(Page::Video(id.clone()));
        assert_eq!(press(&mut v, &r, Press::Hide), Errand::Nothing);
        assert!(v.confirm.is_some(), "hiding was not asked");
        assert_eq!(press(&mut v, &r, Press::Escape), Errand::Nothing);
        assert!(v.confirm.is_none(), "Esc did not answer no");
        if let Some((_, Some(video))) = r.video.as_mut() {
            video.row.hidden = true;
        }
        assert_eq!(
            press(&mut v, &r, Press::Hide),
            Errand::Edit(Edit::Hidden(id, false))
        );
    }

    /// FAILS IF: a channel's score stops moving a hundred a step, leaves
    /// 0–10000, or is not read the ways a person types a number — or a word
    /// typed as a number is taken instead of said.
    #[test]
    fn a_score_steps_a_hundred_inside_its_bounds_and_is_typed() {
        let mut r = readings();
        let a = channel(1).id;
        r.rules = Some((Some(a.clone()), global_rules()));
        let (mut v, _) = View::fresh(Page::Channel(a.clone()));
        onto(&mut v, &r, "score");
        assert_eq!(
            press(&mut v, &r, Press::More),
            Errand::Edit(Edit::ChannelScore(a.clone(), 5100))
        );
        assert_eq!(
            press(&mut v, &r, Press::Less),
            Errand::Edit(Edit::ChannelScore(a.clone(), 4900))
        );
        r.channels[0].score = 10_000;
        assert_eq!(
            press(&mut v, &r, Press::More),
            Errand::Edit(Edit::ChannelScore(a.clone(), 10_000))
        );
        let t = texts("en-GB");
        let asked = press(&mut v, &r, Press::Enter);
        assert!(
            matches!(&asked, Errand::Ask { ask: Ask::ChannelScore, initial: Some(n) } if n == "10000"),
            "{asked:?}"
        );
        assert_eq!(
            v.typed(Ask::ChannelScore.command(), "7 500", &r, &t),
            Errand::Edit(Edit::ChannelScore(a.clone(), 7500))
        );
        press(&mut v, &r, Press::Enter);
        assert!(matches!(
            v.typed(Ask::ChannelScore.command(), "lots", &r, &t),
            Errand::Say(_)
        ));
        press(&mut v, &r, Press::Enter);
        assert_eq!(
            v.typed(Ask::ChannelScore.command(), "99999", &r, &t),
            Errand::Edit(Edit::ChannelScore(a, 10_000))
        );
    }

    /// FAILS IF: a rule from everywhere, changed on a channel's page, changes
    /// the rule everywhere — it must become the channel's own version.
    #[test]
    fn a_rule_changed_on_a_channel_becomes_its_own() {
        let mut r = readings();
        let a = channel(1).id;
        r.rules = Some((Some(a.clone()), global_rules()));
        let (mut v, _) = View::fresh(Page::Rule(Some(a.clone()), Slot::Single(Single::Recency)));
        onto(&mut v, &r, "points");
        let Errand::Edit(Edit::PutRule(rule)) = press(&mut v, &r, Press::More) else {
            panic!("the points did not turn");
        };
        assert_eq!(rule.channel, Some(a), "the change went everywhere");
        assert_eq!(rule.id, None, "the global rule's id was reused");
        assert_eq!(rule.points, 2100);
        // AND RECENCY IS NEVER TAKEN BELOW NOTHING.
        let (mut v, _) = View::fresh(Page::Rule(None, Slot::Single(Single::Recency)));
        r.rules = Some((
            None,
            vec![Rule {
                points: 0,
                ..global_rules()[0].clone()
            }],
        ));
        onto(&mut v, &r, "points");
        let Errand::Edit(Edit::PutRule(rule)) = press(&mut v, &r, Press::Less) else {
            panic!("the points did not turn");
        };
        assert_eq!(rule.points, 0);
    }

    /// FAILS IF: the grid stops walking cell by cell and shelf by shelf, a
    /// click on a cell does not select THAT video, or the second click does
    /// not open it (invariant 19).
    #[test]
    fn the_grid_walks_by_cell_and_the_second_click_opens() {
        let r = readings();
        let t = texts("en-GB");
        let (mut v, _) = View::fresh(Page::Videos);
        v.layout = Layout::Grid;
        let columns = 100;
        let rows = v.rows(&r, columns, &t);
        let shelves: Vec<&Row> = rows.iter().filter(|x| !x.cells.is_empty()).collect();
        assert!(shelves.len() >= 2, "ten videos at 100 cells made one shelf");
        let across = shelves[0].cells.len();
        assert!(across >= 2);
        let first = shelves[0].cells[0].clone();
        let n = row_of(&rows, &shelves[0].spot);
        // A CLICK ON THE SECOND CELL, on a lower line of the shelf.
        let second = shelves[0].cells[1].clone();
        assert!(!v.clicked(n + 2, second.from, &rows), "a first click acted");
        assert_eq!(v.at, second.spot);
        assert!(
            v.clicked(n + 1, second.from + 1, &rows),
            "the second click did not act"
        );
        let rows = v.rows(&r, columns, &t);
        v.press(Press::Less, &rows, &r);
        assert_eq!(v.at, first.spot, "← did not go to the cell before");
        let rows = v.rows(&r, columns, &t);
        v.press(Press::Down, &rows, &r);
        assert_eq!(v.at, shelves[1].cells[0].spot, "↓ did not go a shelf down");
        let rows = v.rows(&r, columns, &t);
        assert!(matches!(
            v.press(Press::Enter, &rows, &r),
            Errand::Look(Look {
                page: Page::Video(_),
                ..
            })
        ));
    }

    /// FAILS IF: a key is taught where it does nothing — a video's keys off
    /// a video, the list's keys off the list — or not taught where it works.
    #[test]
    fn keys_are_taught_only_where_they_work() {
        let r = readings();
        let t = texts("en-GB");
        let taught = |v: &View| -> Vec<&'static str> {
            let rows = v.rows(&r, COLUMNS, &t);
            let mut ids: Vec<&'static str> =
                v.hint(&rows, &t).iter().filter_map(|p| p.id).collect();
            ids.dedup();
            ids
        };
        let (home, _) = View::fresh(Page::Home);
        let on_home = taught(&home);
        for id in [
            "video-metrics-player",
            "video-metrics-delete",
            "video-metrics-layout",
        ] {
            assert!(!on_home.contains(&id), "`{id}` taught on the home page");
        }
        let (video, _) = View::fresh(Page::Video(VideoId("video000000".to_owned())));
        let on_video = taught(&video);
        for id in [
            "video-metrics-player",
            "video-metrics-kiosk",
            "video-metrics-download",
            "video-metrics-hide",
            "video-metrics-delete",
        ] {
            assert!(on_video.contains(&id), "`{id}` not taught on a video");
        }
        let (mut list, _) = View::fresh(Page::Videos);
        onto(&mut list, &r, "video:video000001");
        let on_list = taught(&list);
        for id in [
            "video-metrics-layout",
            "video-metrics-page-on",
            "video-metrics-player",
        ] {
            assert!(on_list.contains(&id), "`{id}` not taught on the list");
        }
    }

    /// FAILS IF: the second description is not edited on the rows, `C-s`
    /// does not hand the text to be written, a kept text asks before it is
    /// let go, or a changed one does not.
    #[test]
    fn the_second_description_is_edited_where_it_is_shown() {
        let r = readings();
        let id = VideoId("video000000".to_owned());
        let (mut v, _) = View::fresh(Page::Video(id.clone()));
        onto(&mut v, &r, "second-description");
        assert_eq!(press(&mut v, &r, Press::Enter), Errand::Nothing);
        assert!(v.editing(), "RET did not open the text");
        for c in "Mine".chars() {
            v.text_key(Key::simple(Sym::Char(c)), 60);
        }
        v.text_key(Key::simple(Sym::Enter), 60);
        v.text_key(Key::simple(Sym::Char('!')), 60);
        let ctrl = |c| Key {
            mods: ronoctua_keymap::Mods {
                ctrl: true,
                ..ronoctua_keymap::Mods::default()
            },
            sym: Sym::Char(c),
        };
        let kept = v.text_key(ctrl('s'), 60);
        assert_eq!(
            kept,
            Errand::Edit(Edit::SecondDescription(id.clone(), "Mine\n!".to_owned()))
        );
        let t = texts("en-GB");
        let (said, _) = v.took(
            Done::Edited(Edit::SecondDescription(id, "Mine\n!".to_owned()), Ok(())),
            &r,
            &t,
        );
        assert!(matches!(said, Some(Said::Normal(_))));
        assert!(v.editing(), "keeping it closed the text");
        assert_eq!(v.text_key(Key::simple(Sym::Esc), 60), Errand::Nothing);
        assert!(!v.editing(), "a kept text asked before it was let go");
        // A CHANGED ONE ASKS, "no" first.
        onto(&mut v, &r, "second-description");
        press(&mut v, &r, Press::Enter);
        v.text_key(Key::simple(Sym::Char('x')), 60);
        v.text_key(Key::simple(Sym::Esc), 60);
        assert!(v.editing(), "a changed text was thrown away unasked");
        let rows = v.rows(&r, COLUMNS, &t);
        assert!(rows.iter().any(|x| x.spot == "text-answer-false"));
    }

    /// FAILS IF: a video the queue has is added again unasked, the question
    /// is not "no" first, is not drawn under the row the person asked from,
    /// is not followed by the door (it would sit off screen while `a`
    /// seemed to do nothing), or its "yes" does not add it again.
    #[test]
    fn a_video_the_queue_has_is_asked_about() {
        let r = readings();
        let t = texts("en-GB");
        let id = VideoId("video000000".to_owned());
        let (mut v, _) = View::fresh(Page::Video(id));
        onto(&mut v, &r, "player");
        let Errand::Play(play) = press(&mut v, &r, Press::Enter) else {
            panic!("RET on the player's row did not play");
        };
        assert!(play.once, "the player was not asked for it once");
        assert!(!v.follow_owed(), "a key's own follow was owed twice");
        let (said, next) = v.took(
            Done::Queued(
                play.clone(),
                Err(Queued::Already(ronoctua_player::queue::Name::Titled(
                    "3.2".to_owned(),
                ))),
            ),
            &r,
            &t,
        );
        assert_eq!((said, next), (None, Errand::Nothing));
        assert!(
            v.follow_owed(),
            "the door was not told to follow the question"
        );
        assert!(!v.follow_owed(), "the follow was owed past the first ask");
        let rows = v.rows(&r, COLUMNS, &t);
        let no = row_of(&rows, "answer-no");
        assert_eq!(v.cursor(&rows), no);
        assert!(
            row_of(&rows, "player") < no && no < row_of(&rows, "kiosk"),
            "the question is not under the row asked from: {:?}",
            spots(&rows)
        );
        assert_eq!(press(&mut v, &r, Press::Enter), Errand::Nothing);
        v.took(
            Done::Queued(
                play.clone(),
                Err(Queued::Already(ronoctua_player::queue::Name::Titled(
                    "3.2".to_owned(),
                ))),
            ),
            &r,
            &t,
        );
        press(&mut v, &r, Press::Down);
        assert_eq!(
            press(&mut v, &r, Press::Enter),
            Errand::Play(ToPlay {
                once: false,
                ..play
            })
        );
    }

    /// FAILS IF: a page of the list turns past either end, or a filter that
    /// changed keeps the page it was on.
    #[test]
    fn the_pages_of_the_list_stop_at_their_ends() {
        let mut r = readings();
        if let Some(l) = r.listed.as_mut() {
            l.total = u64::from(PER_PAGE) * 2 + 1;
        }
        let (mut v, _) = View::fresh(Page::Videos);
        assert_eq!(press(&mut v, &r, Press::PageBack), Errand::Nothing);
        for _ in 0..5 {
            press(&mut v, &r, Press::PageOn);
        }
        assert_eq!(v.page, 2, "past the last page");
        onto(&mut v, &r, "filter-shorts");
        press(&mut v, &r, Press::More);
        assert_eq!(v.page, 0, "a new filter kept the page");
        assert_eq!(v.filter.shorts, Shorts::Without);
    }

    /// FAILS IF: a key of the view has no press, a question no command, or a
    /// key read by its notation reaches another press than its id's.
    #[test]
    fn every_key_and_question_is_reachable() {
        for (notation, id, _) in OF_THE_VIEW {
            let key = Key::parse(notation).expect("a key the keymap reads");
            assert_eq!(press_of_key(key), press_of(id), "`{notation}`");
            assert!(press_of(id).is_some(), "`{id}` presses nothing");
        }
        for ask in Ask::ALL {
            assert_eq!(Ask::of_command(ask.command()), Some(ask));
        }
        assert_eq!(press_of_key(Key::simple(Sym::Esc)), Some(Press::Escape));
        // THE WAY BACK: Esc walks the pages, and closes at home.
        let r = readings();
        let (mut v, _) = View::fresh(Page::Channels);
        assert!(matches!(press(&mut v, &r, Press::Escape), Errand::Look(_)));
        assert_eq!(v.page(), &Page::Home);
        assert_eq!(press(&mut v, &r, Press::Escape), Errand::Close);
    }

    /// FAILS IF: a single rule switched off on a channel's page comes back
    /// as the rule everywhere — "on", with no way to reach the channel's own
    /// — rather than as the channel's, off, with its Remove; or switching it
    /// on again makes a third rule instead of turning that one.
    #[test]
    fn a_rule_switched_off_on_a_channel_stays_its_own() {
        let mut r = readings();
        let a = channel(1).id;
        let t = texts("en-GB");
        r.rules = Some((Some(a.clone()), global_rules()));
        let (mut v, _) = View::fresh(Page::Rule(Some(a.clone()), Slot::Single(Single::Short)));
        onto(&mut v, &r, "on");
        let Errand::Edit(Edit::PutRule(off)) = press(&mut v, &r, Press::Enter) else {
            panic!("In force did not turn");
        };
        assert_eq!((off.channel.as_ref(), off.enabled), (Some(&a), false));
        // THE CATALOGUE READS IT BACK as the channel's own, switched off — and
        // a list that still carries the one everywhere beside it draws the
        // channel's.
        let own = Rule {
            id: Some(RuleId(9)),
            ..off
        };
        let mut rules = global_rules();
        rules.push(own);
        r.rules = Some((Some(a.clone()), rules));
        let rows = v.rows(&r, COLUMNS, &t);
        let on = row_of(&rows, "on");
        assert!(
            rows[on]
                .fields
                .iter()
                .any(|f| f.text == t.t(key::VIDEO_METRICS_OFF)),
            "the switch the person turned off reads on"
        );
        row_of(&rows, "remove-rule");
        let Errand::Edit(Edit::PutRule(back)) = press(&mut v, &r, Press::Enter) else {
            panic!("In force did not turn back");
        };
        assert_eq!((back.id, back.enabled), (Some(RuleId(9)), true));
        // AND THE CHANNEL'S PAGE says it is the channel's, off.
        let (c, _) = View::fresh(Page::Channel(a));
        let rows = c.rows(&r, COLUMNS, &t);
        let short = &rows[row_of(&rows, "single:short")];
        let said: String = short.fields.iter().map(|f| f.text.as_str()).collect();
        assert!(
            said.contains(&t.t(key::VIDEO_METRICS_OFF))
                && said.contains(&t.t(key::VIDEO_METRICS_SCOPE_OWN)),
            "{said}"
        );
    }

    /// FAILS IF: "published since" is worked out again at every draw — the
    /// page then stops matching the reading the feed holds once the clock
    /// passes a boundary, and says "reading" for ever — or it is cut at a
    /// day's start rather than counted back from when it was chosen.
    #[test]
    fn a_published_filter_is_a_moment_fixed_when_chosen() {
        let r = readings();
        let (mut v, _) = View::fresh(Page::Videos);
        onto(&mut v, &r, "filter-since");
        let before = now_s();
        press(&mut v, &r, Press::More);
        let after = now_s();
        let first = v.look().query.expect("the list's query").filter;
        let at = first.published_after_s.expect("a moment");
        assert!(
            (before - 86_400..=after - 86_400).contains(&at),
            "not a day back from now: {at}"
        );
        assert_eq!(
            v.look().query.map(|q| q.filter),
            Some(first),
            "the moment moved between two looks"
        );
    }

    /// FAILS IF: a question run BY NAME asks about nothing — and its answer
    /// is then dropped in silence — instead of asking about the row the
    /// cursor is on, or saying there is none; or an answer with no row to
    /// take it is not said.
    #[test]
    fn a_question_by_name_is_about_the_cursors_row_or_said() {
        let mut r = readings();
        let t = texts("en-GB");
        let (mut v, _) = View::fresh(Page::Home);
        let rows = v.rows(&r, COLUMNS, &t);
        assert!(matches!(
            v.asked_by_name(Ask::Points, &rows, &r, &t),
            Errand::Say(_)
        ));
        assert!(
            matches!(
                v.typed(Ask::Points.command(), "500", &r, &t),
                Errand::Say(_)
            ),
            "an answer with no row was dropped in silence"
        );
        r.rules = Some((None, global_rules()));
        let (mut v, _) = View::fresh(Page::Rule(None, Slot::Single(Single::Recency)));
        let rows = onto(&mut v, &r, "points");
        assert!(matches!(
            v.asked_by_name(Ask::Points, &rows, &r, &t),
            Errand::Ask {
                ask: Ask::Points,
                initial: Some(_)
            }
        ));
        assert!(matches!(
            v.typed(Ask::Points.command(), "700", &r, &t),
            Errand::Edit(Edit::PutRule(Rule { points: 700, .. }))
        ));
        // A SEARCH NEEDS NO ROW, and its answer is shown where it is seen.
        let (mut v, _) = View::fresh(Page::Home);
        let rows = v.rows(&r, COLUMNS, &t);
        assert!(matches!(
            v.asked_by_name(Ask::Search, &rows, &r, &t),
            Errand::Ask {
                ask: Ask::Search,
                ..
            }
        ));
        v.typed(Ask::Search.command(), "nix", &r, &t);
        assert_eq!(v.page(), &Page::Videos);
    }

    /// FAILS IF: a switch or a weight is drawn — or written — from a value
    /// nobody read: turning "Block YouTube Shorts" before the switches were
    /// read wrote the defaults over the first scan's limits.
    #[test]
    fn nothing_unread_is_drawn_or_written() {
        let mut r = readings();
        r.settings = None;
        r.weights = None;
        let t = texts("en-GB");
        let (v, _) = View::fresh(Page::Rules);
        let rows = v.rows(&r, COLUMNS, &t);
        assert!(!rows.iter().any(|x| x.spot == "block-shorts"));
        let (mut v, _) = View::fresh(Page::Rules);
        assert_eq!(
            v.turn(&Target::BlockShorts(false), true, &r),
            Errand::Nothing
        );
        assert_eq!(
            v.turn(&Target::Weight(Rating::Positive), true, &r),
            Errand::Nothing
        );
    }

    /// FAILS IF: a scan that ended nowhere — its process killed — is drawn as
    /// one running, or one running is not.
    #[test]
    fn a_scan_is_running_only_while_it_holds_the_catalogue() {
        let mut r = readings();
        let t = texts("en-GB");
        if let Some(Some(s)) = r.last_scan.as_mut() {
            s.ended_at_s = None;
        }
        let (v, _) = View::fresh(Page::Home);
        let said = |r: &Readings| -> String {
            let rows = v.rows(r, COLUMNS, &t);
            rows[row_of(&rows, "last-scan")]
                .fields
                .iter()
                .map(|f| f.text.as_str())
                .collect()
        };
        let since = date_or_never(Some(NOW - 7200), &r, &t);
        r.scanning = Some(false);
        let cut = t.t_with(key::VIDEO_METRICS_SCAN_CUT, &[("when", &since)]);
        assert!(
            said(&r).contains(&cut),
            "a killed scan was drawn as running"
        );
        r.scanning = Some(true);
        let running = t.t_with(key::VIDEO_METRICS_SCAN_RUNNING, &[("when", &since)]);
        assert!(
            said(&r).contains(&running),
            "a scan running was not drawn so"
        );
    }

    /// FAILS IF: every way a scan was or was not asked has no sentence of
    /// its own, a download handed over does not open the queue, or a grid
    /// tile touches its cell's edge — the hover lights the cell, and a
    /// picture flush with its left edge reads as cut — or a click in the air
    /// between two cells takes one.
    #[test]
    fn what_errands_came_to_is_said_and_a_tile_keeps_its_air() {
        let r = readings();
        let t = texts("pt-BR");
        let mut v = View::default();
        let mut sentences = Vec::new();
        for s in [
            Scanned::Brain,
            Scanned::Systemd,
            Scanned::Nowhere,
            Scanned::Saver,
            Scanned::OtherInstance,
        ] {
            let (said, _) = v.took(Done::Scan(s), &r, &t);
            let (Some(Said::Normal(text)) | Some(Said::Long(text))) = said else {
                panic!("{s:?} said nothing");
            };
            assert!(!text.contains("[video-metrics"), "{s:?}: {text}");
            sentences.push(text);
        }
        sentences.dedup();
        assert_eq!(sentences.len(), 5, "two answers share a sentence");
        assert_eq!(
            v.took(Done::Downloading("A".to_owned(), true), &r, &t).1,
            Errand::Transfers
        );

        let (mut v, _) = View::fresh(Page::Videos);
        v.layout = Layout::Grid;
        let rows = v.rows(&r, 100, &t);
        let shelf = rows
            .iter()
            .position(|x| !x.cells.is_empty())
            .expect("a shelf");
        let cells = rows[shelf].cells.clone();
        for line in &rows[shelf..shelf + usize::from(SHELF_LINES)] {
            for f in &line.fields {
                let cell = cells
                    .iter()
                    .find(|c| crate::chrome::in_cell(f.at, c.from, c.width))
                    .expect("a field outside every cell");
                let end = f.at + ronoctua_render::layout::width(&f.text);
                assert!(
                    f.at >= cell.from + TILE_AIR && end <= cell.from + cell.width - TILE_AIR,
                    "`{}` touches its cell's edge",
                    f.text
                );
            }
        }
        assert!(cells.len() >= 2);
        let gap = cells[0].from + cells[0].width;
        assert!(gap < cells[1].from, "no air between two cells");
        let at = v.at.clone();
        assert!(
            !v.clicked(shelf, gap, &rows),
            "the air between two cells acted"
        );
        assert_eq!(v.at, at, "the air between two cells took the cursor");
    }
}
