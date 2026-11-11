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
//! Home (the scan, the catalogue, the ways to it in other apps — the
//! brain's page on the scan, the player's lists —, what was left out), the
//! channels, one channel (its facts, its videos here and on YouTube, its
//! score, every rule in its channel version, removing it), the global rules (recency, the Short penalty, blocking Shorts, the
//! length bands, the preferred band, words), one rule, the rating weights,
//! the videos (by score, as a list or a grid of thumbnails, with filters and
//! pages), one video (every field, the person's rating, tags and second
//! description, and what can be done with it) — and a list of subscriptions
//! (Google Takeout's) imported, its channels decided one by one and
//! registered, and the channels here exported as such a list.
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

use crate::key;
use ronoctua_i18n::Arg;
use ronoctua_tokens as tokens;
use ronoctua_video_metrics::person::Registered;
use ronoctua_video_metrics::read::{
    Channel, Decision, Filter, Forever, Hidden, Order, RankedVideo, ScanRecord, SetAside, Shorts,
    Staged, Video,
};
use ronoctua_video_metrics::{
    ChannelId, Fields, Keep, Rating, Rule, RuleId, RuleKind, Settings, Skip, TermMatch, VideoId,
    Weights, Why, takeout,
};

use crate::chrome::Kind;
use crate::fields::{Field, Hit as FilesHit, Ink};
use crate::player_rows::Whole;
use crate::video_metrics_words as words;

/// The keys this view answers to, and where each one works. The hint line is
/// GENERATED from it (`chrome::hint_of_marked`), so a key that moves here
/// moves on screen, and a key is taught only where it does something.
///
/// `+` and `-` are the design's keys for a score; they are the arrows' twins
/// on every value, and the hint teaches the arrows (a line shows an id once).
///
/// THE PAGE'S QUICK KEYS ARE `[` AND `]` (the owner, 2026-11-10), BEFORE
/// `PgUp`/`PgDn`: a line shows an id under its FIRST key, and only that key
/// is offered after `LEADER Alt` in a window (`chrome::published`) — the
/// page keys stay direct there, and are offered nothing. The page row's
/// sentence teaches all of them, as each door reaches them
/// ([`View::page_keys`]).
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "video-metrics-down", Where::Anywhere),
    ("<up>", "video-metrics-up", Where::Anywhere),
    ("RET", "video-metrics-enter", Where::Anywhere),
    ("<left>", "video-metrics-less", Where::OnAValue),
    ("<right>", "video-metrics-more", Where::OnAValue),
    ("-", "video-metrics-less", Where::OnAValue),
    ("+", "video-metrics-more", Where::OnAValue),
    ("[", "video-metrics-page-back", Where::OnTheVideos),
    ("]", "video-metrics-page-on", Where::OnTheVideos),
    ("<prior>", "video-metrics-page-back", Where::OnTheVideos),
    ("<next>", "video-metrics-page-on", Where::OnTheVideos),
    ("<home>", "video-metrics-page-first", Where::OnTheVideos),
    ("<end>", "video-metrics-page-last", Where::OnTheVideos),
    ("L", "video-metrics-layout", Where::OnTheVideos),
    ("/", "video-metrics-search", Where::OnTheVideos),
    ("a", "video-metrics-player", Where::OnAVideo),
    ("K", "video-metrics-kiosk", Where::OnAnAddress),
    ("n", "video-metrics-nyxt", Where::OnAnAddress),
    ("D", "video-metrics-download", Where::OnAVideo),
    ("h", "video-metrics-hide", Where::OnAVideo),
    ("d", "video-metrics-delete", Where::OnAVideo),
    ("s", "video-metrics-set-aside", Where::OnAVideo),
    ("f", "video-metrics-keep", Where::OnAVideo),
];

/// THE KEYS AFTER `LEADER Alt` that the rule for every window does not give
/// (`chrome::leader_alt_key`) — the capitals: the layout is `y`, as the
/// player's is; the kiosk `k`; the download `g`, for get.
pub const LEADER_ALT_KEYS: &[(&str, &str)] = &[("L", "y"), ("K", "k"), ("D", "g")];

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
    /// ON WHAT HAS A PAGE ON YOUTUBE, which a browser opens: a video, where
    /// [`Where::OnAVideo`] is — or a channel, its page or its row on the
    /// channels' list, whose videos' page opens.
    OnAnAddress,
}

/// The command that opens the view in the footer's region.
pub const OPEN_COMMAND: &str = "video-metrics-open";
/// The command that opens it in a window.
pub const WINDOW_COMMAND: &str = "video-metrics-window";
/// The command that asks for a scan now.
pub const SCAN_COMMAND: &str = "video-metrics-scan";

/// THE FOOTER'S COMMAND THAT OPENS THE BRAIN'S PAGE — on a module's, given
/// its id: where the scan's schedule is read and changed
/// ([`Errand::Brain`]).
pub const BRAIN_OPEN: &str = "brain-open";

/// THE PLAYER'S COMMAND THAT OPENS ITS WINDOW — on a page of the ranking,
/// given its word (`player_seam::ranking::OPEN_RANKING`): a window of its
/// own, even with one open ([`Errand::Player`]).
pub const PLAYER_WINDOW: &str = "player-window";

/// A LIST OF THE PLAYER'S that the home page opens in a window of the
/// player's own: the catalogue's videos by score, or the ones set aside,
/// newest first — the player's two pages of this catalogue
/// (`player_ranking`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InThePlayer {
    /// "Videos by score".
    Ranking,
    /// The videos set aside.
    SetAside,
}

impl InThePlayer {
    /// The word the player's window opens on it by
    /// (`player_window::opening_of`).
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Ranking => crate::player_seam::ranking::OPEN_RANKING,
            Self::SetAside => crate::player_seam::ranking::OPEN_SET_ASIDE,
        }
    }
}

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
    /// The first page of videos.
    PageFirst,
    /// The last page of videos.
    PageLast,
    /// A list, or a grid of thumbnails.
    Layout,
    /// Search the videos' words.
    Search,
    /// The video in the player.
    Player,
    /// The video in Chrome, as a kiosk — or a channel's videos.
    Kiosk,
    /// The video in Nyxt — or a channel's videos.
    Nyxt,
    /// The video downloaded, by the transfers' queue.
    Download,
    /// The video hidden, or shown again.
    Hide,
    /// The video deleted for good.
    Delete,
    /// The video set aside for later, or taken out of the set-aside ones.
    SetAside,
    /// The video's own mark never to be deleted, put on or taken off.
    Keep,
    /// A key the file picker claims while it is up over the import page
    /// ([`View::picker_takes`]) — every letter, and its own keys: the door
    /// hands it as this before any other reading of the key.
    Pick(ronoctua_keymap::Key),
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
        "video-metrics-page-first" => Press::PageFirst,
        "video-metrics-page-last" => Press::PageLast,
        "video-metrics-layout" => Press::Layout,
        "video-metrics-search" => Press::Search,
        "video-metrics-player" => Press::Player,
        "video-metrics-kiosk" => Press::Kiosk,
        "video-metrics-nyxt" => Press::Nyxt,
        "video-metrics-download" => Press::Download,
        "video-metrics-hide" => Press::Hide,
        "video-metrics-delete" => Press::Delete,
        "video-metrics-set-aside" => Press::SetAside,
        "video-metrics-keep" => Press::Keep,
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

/// Every command this view's keys and rows reach — for `bindings::reachable`,
/// which is what says a command has a key.
pub fn reached() -> impl Iterator<Item = &'static str> {
    OF_THE_VIEW
        .iter()
        .map(|(_, id, _)| *id)
        .chain(Ask::ALL.iter().map(|a| a.command()))
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "video-metrics",
    commands: &[
        // `LEADER v` — THE VIDEO CATALOGUE (`video_metrics`): its two doors, a
        // scan now, the view's keys and the values it asks to be typed.
        ronoctua_cmd::Command {
            id: "video-metrics-open",
            label: crate::key::CMD_VIDEO_METRICS_OPEN,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-window",
            label: crate::key::CMD_VIDEO_METRICS_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-scan",
            label: crate::key::CMD_VIDEO_METRICS_SCAN,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-up",
            label: crate::key::CMD_VIDEO_METRICS_UP,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-down",
            label: crate::key::CMD_VIDEO_METRICS_DOWN,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-less",
            label: crate::key::CMD_VIDEO_METRICS_LESS,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-more",
            label: crate::key::CMD_VIDEO_METRICS_MORE,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-enter",
            label: crate::key::CMD_VIDEO_METRICS_ENTER,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-page-back",
            label: crate::key::CMD_VIDEO_METRICS_PAGE_BACK,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-page-on",
            label: crate::key::CMD_VIDEO_METRICS_PAGE_ON,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-page-first",
            label: crate::key::CMD_VIDEO_METRICS_PAGE_FIRST,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-page-last",
            label: crate::key::CMD_VIDEO_METRICS_PAGE_LAST,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-layout",
            label: crate::key::CMD_VIDEO_METRICS_LAYOUT,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-search",
            label: crate::key::CMD_VIDEO_METRICS_SEARCH,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-player",
            label: crate::key::CMD_VIDEO_METRICS_PLAYER,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-kiosk",
            label: crate::key::CMD_VIDEO_METRICS_KIOSK,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-nyxt",
            label: crate::key::CMD_VIDEO_METRICS_NYXT,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-download",
            label: crate::key::CMD_VIDEO_METRICS_DOWNLOAD,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-hide",
            label: crate::key::CMD_VIDEO_METRICS_HIDE,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-delete",
            label: crate::key::CMD_VIDEO_METRICS_DELETE,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-set-aside",
            label: crate::key::CMD_VIDEO_METRICS_SET_ASIDE,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-keep",
            label: crate::key::CMD_VIDEO_METRICS_KEEP,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-add-channel",
            label: crate::key::CMD_VIDEO_METRICS_ADD_CHANNEL,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-channel-score",
            label: crate::key::CMD_VIDEO_METRICS_CHANNEL_SCORE,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-points",
            label: crate::key::CMD_VIDEO_METRICS_POINTS,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-days",
            label: crate::key::CMD_VIDEO_METRICS_DAYS,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-band",
            label: crate::key::CMD_VIDEO_METRICS_BAND,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-words",
            label: crate::key::CMD_VIDEO_METRICS_WORDS,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-second-tags",
            label: crate::key::CMD_VIDEO_METRICS_SECOND_TAGS,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-weight",
            label: crate::key::CMD_VIDEO_METRICS_WEIGHT,
        },
        ronoctua_cmd::Command {
            id: "video-metrics-keep-days",
            label: crate::key::CMD_VIDEO_METRICS_KEEP_DAYS,
        },
    ],
    bindings: &[
        // `v` — THE VIDEO CATALOGUE, on a top-level letter the design left free:
        // the doubled key is the footer's door, `w` the window's, `s` a scan now.
        ("v v", "video-metrics-open", None),
        ("v w", "video-metrics-window", None),
        ("v s", "video-metrics-scan", None),
    ],
    groups: &[("v", crate::key::GROUP_VIDEO_METRICS)],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::video_metrics::reached().collect()
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
    /// After how many days videos are deleted — everywhere, or a channel's.
    KeepDays,
}

impl Ask {
    /// Every one.
    pub const ALL: [Self; 10] = [
        Self::AddChannel,
        Self::ChannelScore,
        Self::Points,
        Self::Days,
        Self::Band,
        Self::Words,
        Self::Search,
        Self::SecondTags,
        Self::Weight,
        Self::KeepDays,
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
            Self::KeepDays => "video-metrics-keep-days",
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
    /// One channel — of the catalogue, or of a list imported and not
    /// registered yet.
    Channel(ChannelId),
    /// A list of subscriptions imported (Google Takeout's), its channels to
    /// decide and register — and the channels here exported as one.
    Import,
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

/// HOW THE VIDEOS ARE LAID OUT — a ring of five (the owner, 2026-11-10 and
/// 2026-11-11), turned by the "Layout" row's `←`/`→`, its click and `L`: the
/// channel under a thumbnail is a layout of the ring and not a switch beside
/// it, so one choice has one way to it. Kept for the session, as the filters
/// are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// One row a video: its title on the whole width, what it scores under.
    #[default]
    List,
    /// THE LIST WITH THUMBNAILS — the player's list: each video's picture at
    /// its head, its title wrapped whole beside it and what it scores under
    /// the title, a line of air after it ([`strip_entry`]).
    Strip,
    /// A grid of thumbnails.
    Grid,
    /// The grid, each thumbnail saying its channel under its title.
    GridWithChannel,
    /// WIDE THUMBNAILS (the owner, 2026-11-11): a tile as wide as two of the
    /// grid's, its picture the grid's, what it scores, its age and its marks
    /// beside the picture, and under it the title on two lines of the whole
    /// tile and the channel on one — a title read nearly whole.
    Wide,
}

impl Layout {
    /// The five, in the ring's order.
    pub const ALL: [Self; 5] = [
        Self::List,
        Self::Strip,
        Self::Grid,
        Self::GridWithChannel,
        Self::Wide,
    ];

    /// Its words, the "Layout" row's value.
    const fn key(self) -> &'static str {
        match self {
            Self::List => key::VIDEO_METRICS_LAYOUT_LIST,
            Self::Strip => key::VIDEO_METRICS_LAYOUT_LIST_THUMBNAILS,
            Self::Grid => key::VIDEO_METRICS_LAYOUT_GRID,
            Self::GridWithChannel => key::VIDEO_METRICS_LAYOUT_GRID_CHANNEL,
            Self::Wide => key::VIDEO_METRICS_LAYOUT_WIDE,
        }
    }

    /// Whether it draws the videos' thumbnails — what the feed decodes with
    /// the page ([`Query::pictures`]).
    #[must_use]
    pub const fn pictures(self) -> bool {
        !matches!(self, Self::List)
    }

    /// Whether the videos stand in a grid's shelves, where `←`/`→` walk
    /// cell by cell.
    #[must_use]
    pub const fn grid(self) -> bool {
        matches!(self, Self::Grid | Self::GridWithChannel | Self::Wide)
    }

    /// Whether a grid's thumbnail says its channel under its title.
    #[must_use]
    pub const fn channel(self) -> bool {
        matches!(self, Self::GridWithChannel | Self::Wide)
    }
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
    i64::try_from(crate::when::seconds_now()).unwrap_or(i64::MAX)
}

/// How the transfers' queue is opened from the leader, as a sentence
/// teaches it: the leader's label and its keys (`SUPER t t`,
/// `bindings::taught`) — never the keys alone, which read `t t` where every
/// other door says `SUPER t t` (`keys-are-never-text`). What every door
/// fills [`Readings::transfers_keys`] with.
#[must_use]
pub fn transfers_keys() -> String {
    crate::bindings::taught("transfers-open", None).unwrap_or_default()
}

/// How much a step of `←`/`→` moves a score, a weight or a rule's points.
pub const POINTS_STEP: i32 = 100;

/// Which page of which list the door is asked to read: its filter, its page
/// and how many it holds, and whether the thumbnails are decoded with it
/// (a layout that draws them, [`Layout::pictures`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Query {
    /// What the list shows.
    pub filter: Filter,
    /// Which page, from 0.
    pub page: u32,
    /// How many videos a page holds (`paging::PER_PAGES`).
    pub per_page: u32,
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
    /// How many of them are set aside, hidden ones included.
    pub set_aside_count: Option<u64>,
    /// How many the next scan takes out for their time (`Catalog::expiring`).
    pub expiring: Option<u64>,
    /// The tombstones, by reason.
    pub skipped: Option<Vec<(Skip, u64)>>,
    /// Whether the hosts a scan asks are let.
    pub consent: Option<bool>,
    /// Every channel.
    pub channels: Option<Vec<Channel>>,
    /// The channels of a list imported, not registered yet.
    pub staged: Option<Vec<Staged>>,
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
    /// How many of them are set aside, hidden ones included.
    pub set_aside_count: Option<u64>,
    /// How many the next scan takes out for their time, under the keeps in
    /// force when it was read.
    pub expiring: Option<u64>,
    /// The tombstones, by reason.
    pub skipped: Vec<(Skip, u64)>,
    /// Whether YouTube's hosts are let.
    pub consent: Option<bool>,
    /// Every channel, by score.
    pub channels: Vec<Channel>,
    /// THE CHANNELS OF A LIST IMPORTED and not registered yet, by title —
    /// each with what registering does with it. Not channels of the
    /// catalogue: no scan reads them, and the channels' list leaves them
    /// out. Their pages are the channels' own ([`Readings::channel`]).
    pub staged: Vec<Staged>,
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
    /// sentence a download says ([`transfers_keys`], filled by the door).
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
        if let Some(n) = r.set_aside_count {
            self.set_aside_count = Some(n);
        }
        if let Some(n) = r.expiring {
            self.expiring = Some(n);
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
        if let Some(s) = r.staged {
            self.staged = s;
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

    /// A channel, by id — of the catalogue, or a staged one: A CHANNEL OF A
    /// LIST IMPORTED HAS THE PAGE EVERY CHANNEL HAS, and its rules' pages,
    /// where everything a channel is given is given to it before it is
    /// registered ([`Readings::staged_as`] says what it waits for).
    fn channel(&self, id: &ChannelId) -> Option<&Channel> {
        self.channels
            .iter()
            .find(|c| &c.id == id)
            .or_else(|| self.staged_as(id).map(|s| &s.channel))
    }

    /// The staged channel by `id`, and what registering does with it —
    /// `None` for a channel of the catalogue, or none.
    fn staged_as(&self, id: &ChannelId) -> Option<&Staged> {
        self.staged.iter().find(|s| &s.channel.id == id)
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

    /// A video's title, from its page or the list, when either holds it.
    fn title_of(&self, id: &VideoId) -> Option<String> {
        self.video_of(id)
            .map(|v| &v.row)
            .or_else(|| self.ranked(id))
            .map(|v| v.title.clone())
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
    /// Set a video aside for later, or take it out of the set-aside ones.
    SetAside(VideoId, bool),
    /// A video's own mark never to be deleted, on or off.
    KeepVideo(VideoId, bool),
    /// How long a channel's videos stay (`None`: as everywhere).
    ChannelKeep(ChannelId, Option<Keep>),
    /// Delete a video for good.
    Delete(VideoId),
    /// Let YouTube's hosts be asked, or not.
    Consent(bool),
    /// STAGE THE CHANNELS OF THE LIST IN THIS FILE — Google Takeout's
    /// subscriptions: each waits for a decision, and nothing of it is scanned
    /// until it is registered.
    Import(PathBuf),
    /// What registering does with one staged channel.
    Decide(ChannelId, Decision),
    /// The same decision, for every staged channel.
    DecideAll(Decision),
    /// What was decided, registered: the kept join the catalogue, the
    /// discarded go, the undecided stay.
    Register,
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
    /// A page of YouTube — a video's, a channel's videos — in Chrome, as a
    /// kiosk (`kiosk-open`, the footer's).
    Kiosk(String),
    /// The same in Nyxt, started by its entry in the launcher
    /// (`launch::in_nyxt`).
    Nyxt(String),
    /// The brain's page on this catalogue's scan — when it runs, what it
    /// waits for, what it did ([`BRAIN_OPEN`], the footer's).
    Brain,
    /// A window of the player's, of its own, on the page this word opens
    /// ([`InThePlayer::word`], [`PLAYER_WINDOW`]).
    Player(&'static str),
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
    /// THE REGISTERED CHANNELS WRITTEN as a list in Google's shape, here —
    /// on the door's feed, off the loop.
    Export(PathBuf),
    /// A sentence of the file picker's — a typed path that names nothing, a
    /// name it will not save under — worded by the door, which holds the
    /// language.
    Explain(ronoctua_files::mode::Explanation),
}

/// What came of an errand the feed carried out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Done {
    /// A write, and what the catalogue answered.
    Edited(Edit, Result<(), Why>),
    /// A channel added — its name — or why not.
    Added(Result<String, Unadded>),
    /// The consent to ask YouTube could not be kept: the key of why.
    NotLet(&'static str),
    /// Where a scan was asked.
    Scan(Scanned),
    /// What the player said to a video, or that nobody answered.
    Queued(ToPlay, Result<(), Queued>),
    /// A download handed to the queue, or the queue not there.
    Downloading(String, bool),
    /// A list imported — what staging it did — or why not.
    Imported(Result<Imported, Unimported>),
    /// What was decided, registered — and, when the consent to scan the
    /// kept could not be kept, the key of why: the channels are in all the
    /// same — or why nothing was registered.
    Registered(Result<(Registered, Option<&'static str>), Why>),
    /// The registered channels exported: how many, and where — or why not.
    Exported(Result<(u32, PathBuf), Unexported>),
}

/// WHAT IMPORTING A LIST DID, as the import page says it while the view is
/// open ([`View::imported`]): the channels it left waiting, the ones it
/// found registered already, the lines it could not read as a channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    /// Channels now waiting for a decision.
    pub staged: u32,
    /// Channels of the list already in the catalogue — never staged: their
    /// ids and the catalogue's titles.
    pub already: Vec<(ChannelId, String)>,
    /// Lines of the file whose first field is no channel's id.
    pub left_out: u32,
}

/// Why a list was not imported, as the door can say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unimported {
    /// The file could not be opened or read.
    Unreadable,
    /// Past `takeout::MOST_BYTES`: something else than a list.
    TooLarge,
    /// Read, and not a list (`takeout::Unread`).
    Unread(takeout::Unread),
    /// The catalogue refused.
    Refused(Why),
}

/// Why the channels were not exported, as the door can say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unexported {
    /// The catalogue refused to list them.
    Refused(Why),
    /// The file could not be written where it was asked.
    NotWritten,
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
    /// Register what was decided: how many are kept, how many discarded.
    Register(u32, u32),
    /// Give every staged channel this decision while theirs differ — what
    /// each was given is lost, and no turn gives it back: how many channels
    /// it changes.
    DecideAll(Decision, u32),
}

/// A question, and where its cursor is.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Confirm {
    what: Pending,
    /// Which answer the cursor is on (`true` is yes), and whether a PERSON
    /// put it there: born on the "no" — the screen's cursor, so the first
    /// click on it only picks it (invariant 19, `aim::Aim`), and one press
    /// of a double click meant for the row above never answers.
    aim: crate::aim::Aim<bool>,
    /// The spot of the row it was asked from: the question is drawn under
    /// it, and the cursor goes back to it once answered.
    under: String,
}

impl Confirm {
    /// The answer the cursor is on — `true` is yes.
    fn yes(&self) -> bool {
        self.aim.on().copied().unwrap_or(false)
    }
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
    /// How long a channel's videos stay, as it is now (`None`: as everywhere).
    ChannelKeep(ChannelId, Option<Keep>),
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
    /// How long videos stay everywhere, as it is now.
    KeepEverywhere(Keep),
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
    /// The list's set-aside videos.
    FilterSetAside,
    /// The list's words.
    FilterText,
    /// How far back the list reaches.
    FilterSince,
    /// How the videos are laid out — a ring of four ([`Layout`]).
    Layout,
    /// The page of the list.
    PageOf,
    /// How many videos a page of it holds — a ring.
    PerPage,
    /// AN ARROW OF THE LIST'S PAGES, beside its title and at its end: the
    /// page after (`true`) or before — at once, as a glyph acts.
    PageTurn(bool),
    /// The set-aside videos: the list, only them, the last set aside first.
    SetAsideVideos,
    /// A video: its page.
    Video(VideoId),
    /// A video's rating.
    Rating(VideoId, Option<Rating>),
    /// An arrow of a video's rating beside its score: the rating it gives,
    /// and whether it is the one the video has (a click takes that away).
    Rate(VideoId, Rating, bool),
    /// A video set aside, or not — as it is now.
    SetAside(VideoId, bool),
    /// A video's own mark never to be deleted — as it is now.
    VideoKeep(VideoId, bool),
    /// A video's second tags.
    SecondTags(VideoId, String),
    /// A video's second description: edit it here.
    SecondDescription(VideoId, String),
    /// The video in the player.
    Player(VideoId),
    /// The video in Chrome.
    Kiosk(VideoId),
    /// The video in Nyxt.
    Nyxt(VideoId),
    /// A channel's videos — its `/videos` page on YouTube — in Chrome.
    ChannelKiosk(ChannelId),
    /// A channel's videos in Nyxt.
    ChannelNyxt(ChannelId),
    /// A CHANNEL OF A LIST IMPORTED, not registered yet, and what
    /// registering does with it: the arrows turn that, `RET` opens its page.
    Staged(ChannelId, Decision),
    /// What registering does with the staged channel whose page is open.
    Decide(ChannelId, Decision),
    /// The same decision for every staged channel — `None` while theirs
    /// differ.
    DecideAll(Option<Decision>),
    /// Google Takeout, where the list of subscriptions is exported, in
    /// Chrome.
    Takeout,
    /// The list to import, chosen in the file picker.
    PickImport,
    /// Where the channels are saved as a list, chosen in the file picker.
    PickExport,
    /// What was decided, registered — asked first.
    Register,
    /// A row of the file picker, while it is up over the import page.
    Pick(crate::file_picker::Target),
    /// The brain's page on the scan.
    Brain,
    /// A list of the player's, in a window of its own.
    InThePlayer(InThePlayer),
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
    /// ([`TILE_AIR`]). Two cells side by side share the air between their
    /// tiles, and THE CELL BEFORE IT TAKES IT — every door finds a cell as
    /// the first of the shelf's under the pointer, for the light and the
    /// click alike; past the last, nothing.
    pub width: u16,
    /// What it is.
    pub target: Target,
    /// Its identity.
    pub spot: String,
    /// What it is called where it is explained — the tile's title: a shelf
    /// is several things on one line, and the line has no name of its own.
    pub name: String,
    /// What the pointer resting on it says.
    pub about: &'static str,
}

/// A GLYPH ON A ROW that acts on the FIRST click — a video's quick marks, a
/// channel's ways out — and the box the hover lights: its own cells, never
/// the row's ([`Row::lit_cells`]).
///
/// APART FROM A SHELF'S [`Cell`]s, which the cursor stops on: `cursor`,
/// `settle`, `walk`, `sideways` and `press` read the cells as the cursor's
/// stops, and a control among them would need every one of the five taught
/// to step over it. A list of its own keeps the five as they are, and the
/// arrows never land on a control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    /// Its first cell, as a field's `at`.
    pub from: u16,
    /// How many cells it takes — the glyph's, and nothing of the air.
    pub width: u16,
    /// What is drawn.
    pub glyph: char,
    /// What the click does, at once.
    pub target: Target,
    /// What it is called where it is explained.
    pub name: String,
    /// What the pointer resting on it says.
    pub about: &'static str,
    /// WHAT HOLDS ONCE IT IS SWITCHED OFF — the ∞ of a video kept by its own
    /// mark, the bookmark of a set-aside one: said under its sentence in the
    /// quietest ink, as a row set "as everywhere" says what it follows
    /// ([`Follows::said`]). Read off [`lasting`] without the mark, so it
    /// never promises "it goes" of a video its channel, the catalogue or
    /// its own mark still keeps.
    pub follows: Option<String>,
    /// Whether it says the video's state now — the rating it has, set
    /// aside: drawn lit.
    pub lit: bool,
}

/// WHAT THE POINTER IS ON, at a cell of a line ([`View::pointed`]).
#[derive(Debug, Clone, Copy)]
enum Pointed<'a> {
    /// A glyph: its line, its place among the line's glyphs, and it.
    Glyph(&'a Row, usize, &'a Control),
    /// A video of a shelf.
    Cell(&'a Cell),
    /// A row: its first line's index, and it.
    Row(usize, &'a Row),
}

impl Pointed<'_> {
    /// Its identity — the foot's subject, and the pointer's rest's key.
    fn subject(&self) -> String {
        match self {
            Pointed::Glyph(line, i, _) => format!("{}/{i}", line.spot),
            Pointed::Cell(cell) => cell.spot.clone(),
            Pointed::Row(_, row) => row.spot.clone(),
        }
    }
}

/// WHAT A ROW SET "AS EVERYWHERE" FOLLOWS NOW (the owner, 2026-11-09): the
/// value everywhere, in the quietest ink, after the row's own — "as
/// everywhere (blocked)" — so what "everywhere" is today is read on the row
/// and not on another page; and, where the row is explained (the window's
/// foot, the footer's line), the sentence that says so.
///
/// WORDED WHERE THE ROW IS BUILT, which knows what it follows — a switch
/// everywhere, a rule of the page everywhere — and PLACED by [`lay_out`],
/// after the value's tint: a state's colour never reaches it, nor does the
/// cursor's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Follows {
    /// After the value, a cell of air between: `(blocked)`.
    pub beside: String,
    /// Under the row's sentence where it is explained, in the same quiet ink.
    pub said: String,
}

impl Follows {
    /// What a choice "as everywhere" follows: the value everywhere, `now`,
    /// in its own words.
    fn now(now: &str, texts: &ronoctua_i18n::Texts) -> Self {
        Self {
            beside: texts.t_with(key::VIDEO_METRICS_DEFAULT_NOW, &[("value", now)]),
            said: texts.t_with(key::VIDEO_METRICS_FOLLOWS_NOW, &[("value", now)]),
        }
    }
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
    /// The glyphs on this line that act on the first click.
    pub controls: Vec<Control>,
    /// The thumbnails drawn over this line.
    pub thumbs: Vec<Thumb>,
    /// The cell a text caret stands before, on a line being edited.
    pub caret: Option<u16>,
    /// What the pointer resting here says, when it is more than its label.
    pub about: Option<&'static str>,
    /// What it is called where it is explained — its label, a video's
    /// title, an answer's words — whole, where the row may have wrapped it.
    pub name: String,
    /// What the row follows, set "as everywhere" ([`Follows`]).
    pub default: Option<Follows>,
    /// A GROUND OF ITS OWN under the line — a question's block
    /// (`chrome::QUESTION_GROUND`), edge to edge and with no outline: each
    /// door paints it under the line's light and the cursor's
    /// (`chrome::row_grounds` in the window, `region::Told::ground` in the
    /// footer). `None`: the page's ground, as every row.
    pub ground: Option<tokens::Color>,
    /// WHERE THE WORDS BEGIN on a line beside a picture — every line of a
    /// video of the list with thumbnails ([`Layout::Strip`]), as a field's
    /// `at`: the light and the click start there and never go over the
    /// picture ([`View::on_the_words`]; `region::LineLights::UnderX` in the
    /// footer, a box in the window), and a line with glyphs has its words'
    /// box from there ([`Row::words_box`]).
    /// `None`: the words begin at the row's `left`, as every other line's.
    pub words_at: Option<u16>,
    /// THE NAMES DRAWN ON THE LINE — a video's title, its channel, cut to a
    /// tile or wrapped over a row's lines (`player_rows::Whole`, the player's
    /// own): where the pointer rests on one, what explains the thing names it
    /// by it, whole ([`View::explained_at`]) — the window's foot, the
    /// footer's explanation line.
    pub wholes: Vec<Whole>,
}

impl Row {
    /// Whether the cursor stops on it.
    #[must_use]
    pub const fn stops(&self) -> bool {
        !self.heading && !self.continues
    }

    /// WHETHER ITS WORDS ARE ITS OWN, beside the glyphs of a line of it
    /// ([`Row::words_box`]): a row the cursor stops on — but a shelf of the
    /// grid, whose words are its videos', each in its cell ([`Cell`]): a
    /// line of its glyphs has only the glyphs.
    #[must_use]
    pub fn has_words(&self) -> bool {
        self.stops() && self.cells.is_empty()
    }

    /// Whether a click takes it — a row the cursor stops on, a line that
    /// continues one, a shelf of the grid (by the cell under the pointer), a
    /// line of controls (by the glyph under it) — and so whether a resting
    /// pointer lights it. Never a title, a note, the air between sections or
    /// a line of a picture.
    #[must_use]
    pub fn takes(&self) -> bool {
        !self.heading || !self.cells.is_empty() || !self.controls.is_empty()
    }

    /// THE ROW'S WORDS on a line with controls, `(from, width)` as a field's
    /// `at`: from `left` — or from where the words begin beside a picture
    /// ([`Row::words_at`]) — up to, and not over, the cell of air before the
    /// first control, on a line whose ROW the cursor stops on (`owner_stops`:
    /// the row's, [`Row::owner`], so a line that continues one, a list
    /// entry's meta line, has words too). `None` on a line of controls
    /// alone, and on one with no room before its first control.
    ///
    /// ONE BOX FOR THE LIGHT, THE FOOT AND THE CLICK: [`Row::lit_cells`]
    /// lights it, and [`View::on_the_words`] reads it for what the foot
    /// explains and what a click takes — the air before the first glyph,
    /// the margin before `left` and a picture beside the words are nobody's
    /// in all three.
    #[must_use]
    pub fn words_box(&self, left: u16, owner_stops: bool) -> Option<(u16, u16)> {
        let first = self.controls.iter().map(|c| c.from).min()?;
        let from = self.words_at.unwrap_or(left);
        (owner_stops && first > from + 1).then(|| (from, first - 1 - from))
    }

    /// THE BOXES THE POINTER LIGHTS on a line with controls, `(from, width)`
    /// as a field's `at`: the row's words ([`Row::words_box`]), then each
    /// control's own. Nothing between: the air is nobody's.
    #[must_use]
    pub fn lit_cells(&self, left: u16, owner_stops: bool) -> Vec<(u16, u16)> {
        self.words_box(left, owner_stops)
            .into_iter()
            .chain(self.controls.iter().map(|c| (c.from, c.width)))
            .collect()
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
    crate::chrome::extent(columns)
}

/// The air a tile's cell keeps on EACH side of the tile — the grid's own gap
/// (`ronoctua_design::GAP`): the hover lights the cell, and a picture, a
/// title and a score that touched its left edge with all the air on the
/// right read as cut. THE TILES STAND AS THE PLAYER'S DO, a gap apart from
/// the page's own left edge on (`ronoctua_design::Grid`), so a window as
/// wide holds as many side by side (the owner, 2026-11-11: the player's
/// listing held four where this one held three) — and two cells side by
/// side SHARE the air between their tiles, which the pointer finds in the
/// cell before it ([`Cell`]).
pub const TILE_AIR: u16 = ronoctua_design::GAP;

/// HOW WIDE A TILE IS, without its cell's air: the thumbnail's box,
/// `space.videoMetrics.thumbCells` — and a wide one ([`Layout::Wide`]) the
/// room of two of them side by side, the gap between included, so a shelf
/// of wide tiles holds exactly half the thumbnails a shelf of the grid
/// does.
#[must_use]
pub const fn tile_cells(layout: Layout) -> u16 {
    let thumb = tokens::space::VIDEOMETRICS_THUMBCELLS;
    match layout {
        Layout::Wide => thumb * 2 + ronoctua_design::GAP,
        Layout::List | Layout::Strip | Layout::Grid | Layout::GridWithChannel => thumb,
    }
}

/// The grid's shelves, at a width: how many tiles side by side, each
/// [`tile_cells`] wide and a gap apart — through `ronoctua_design::Grid`,
/// the one grid arithmetic of the tree, and the player's listing's.
#[must_use]
pub fn grid_of(room: u16, layout: Layout) -> ronoctua_design::grid::Grid {
    ronoctua_design::grid::Grid::fitted(room, tile_cells(layout), 0, u16::MAX, shelf_lines(layout))
}

/// How many lines a grid's shelf takes: the picture's, two for the title,
/// one for its channel when the grid says it ([`Layout::channel`]), one
/// for what it scores and the arrows of its rating, one for when it was
/// published and its marks, one of air. The score and the age are a line
/// EACH: each carries its glyphs, and a tile never cuts a score for them.
/// A WIDE TILE says the score and the age beside its picture, so its
/// shelf is the picture's lines, the title's, the channel's and the air.
#[must_use]
pub const fn shelf_lines(layout: Layout) -> u16 {
    let under = tokens::space::VIDEOMETRICS_THUMBLINES + tokens::space::VIDEOMETRICS_TITLELINES;
    match layout {
        Layout::Wide => under + 2,
        Layout::List | Layout::Strip | Layout::Grid | Layout::GridWithChannel => {
            under + layout.channel() as u16 + 3
        }
    }
}

/// The air between two glyphs of a strip, and between a strip and the
/// words before it — the horizontal air beside anything (`ronoctua_design`):
/// a Nerd glyph has no side bearing, and two side by side touch.
pub const GLYPH_AIR: u16 = ronoctua_design::GAP;

/// How many pictures a page of thumbnails — the grid's, the list's with
/// thumbnails ([`Layout::pictures`]) — may put on screen: never more than
/// the renderer's frame keeps for the file manager's grid, which is the room
/// this view's pictures take while it is up.
pub const PICTURES_MOST: u32 = tokens::space::VIDEOMETRICS_THUMBSMOST as u32;

const _: () = assert!(
    tokens::space::VIDEOMETRICS_THUMBSMOST <= tokens::space::FILES_THUMBMOST,
    "a page of thumbnails beyond what the renderer's frame keeps would lose pictures"
);
const _: () = assert!(
    crate::paging::PER_PAGES[crate::paging::PER_PAGES.len() - 1] <= PICTURES_MOST,
    "a page of thumbnails is a page of the list: all of it must be drawable, at its largest"
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
    /// HOW MANY VIDEOS A PAGE OF IT HOLDS — a ring, the session's
    /// (`paging::PerPage`).
    per_page: crate::paging::PerPage,
    /// A TURNED PAGE OWES ITS TITLE THE TOP OF THE DOOR — a page key, an
    /// arrow, a new size (the owner, 2026-11-10: "ao virar, a tela vai para
    /// o título logo antes da lista"): paid once the reading for the page
    /// asked is in ([`View::settle_on_list`]), with the cursor on its first
    /// video.
    top_owed: bool,
    /// THE LIST'S TITLE HOLDS THE TOP OF THE DOOR — put there by a turned
    /// page ([`View::settle_on_list`]), for as long as the Videos page is on
    /// screen: the list keeps the room under it that lets it be the first
    /// line drawn, however short the page ([`View::rows`]), and a gesture
    /// that began with it there leaves it there while the cursor stays under
    /// it ([`View::title_kept`]). The player's ranking holds its own alike
    /// (`player::View::kept_top`).
    title_held: bool,
    /// HOW MANY LINES OF ROWS THE DOOR SHOWS, as it last told
    /// ([`View::page_is`]): the room a held title keeps under it.
    page_lines: std::cell::Cell<usize>,
    /// THE MOST LINES THE DOOR'S PAGE CAN HOLD, when it shows fewer now and
    /// says so ([`View::page_holds`]) — the footer's region; that room is
    /// never less.
    page_most: std::cell::Cell<usize>,
    /// THE VIEW IS A WINDOW'S, where no keyboard reaches a shortcut and
    /// `LEADER Alt` does: a sentence that teaches a key teaches it so
    /// ([`View::page_keys`]). The footer's region teaches the bare key.
    windowed: bool,
    /// How the videos are laid out — a ring of four, the list at first.
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
    /// Where the rows last built begin, as a field's `at` — stored where it
    /// is measured too, for the words of a line with glyphs, which the
    /// pointer's questions read without a width ([`View::on_the_words`]).
    left: std::cell::Cell<u16>,
    /// The cursor moved without a key — a question the view put on the page
    /// from what an errand came to — and the door owes it a follow
    /// ([`View::follow_owed`]).
    follow_owed: bool,
    /// THE FILE PICKER, while it is up over the import page — the list to
    /// import, or where the channels are saved (`file_picker::Hosted`): its
    /// rows are drawn in place of the page's, every key it claims is its,
    /// and both doors host the same one.
    picking: Option<crate::file_picker::Hosted<Picking>>,
    /// The door's wake, for the picker's folder and devices
    /// ([`View::watch_with`]): `None` in a photograph, which watches nothing.
    wake: Option<crate::wake::Wake>,
    /// The picker's cursor's line when its rows were last built — STORED
    /// WHERE IT IS MEASURED, as [`View::width`] is: the picker keeps its
    /// own cursor, and the doors ask the view's.
    picked_at: std::cell::Cell<usize>,
    /// WHAT THE LAST IMPORT DID — the lines it left out, the channels it
    /// found registered already: the view's to say while it is open, on the
    /// import page. What it staged is the catalogue's, and outlives it.
    imported: Option<Imported>,
}

/// What the file picker is up for, over the import page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picking {
    /// The list of subscriptions to import.
    Import,
    /// Where the registered channels are saved as a list.
    Export,
}

/// The id a text area of this view answers under.
const DESCRIPTION: &str = "video-metrics-second-description";

/// What every list of subscriptions ends in — Google's, and the one written.
const LIST_EXTENSION: &str = "csv";

/// WHAT THE PICKER OF A LIST SHOWS, a ring `TAB` turns: the lists of
/// channels, then every file (`file_picker::Filter::ANY`, the last of every
/// ring).
const LISTS: &[crate::file_picker::Filter] = &[
    crate::file_picker::Filter {
        label: key::VIDEO_METRICS_FILTER_CSV,
        keeps: a_list,
    },
    crate::file_picker::Filter::ANY,
];

/// A LIST OF CHANNELS, by its name or its type — what Takeout writes is a
/// `.csv`, and a file a person renamed is still one when its type says so —
/// drawn with the glyph of its type, as every file of the picker is.
fn a_list(path: &std::path::Path, mime: Option<&'static str>) -> Option<char> {
    let named = path
        .extension()
        .is_some_and(|x| x.to_string_lossy().eq_ignore_ascii_case(LIST_EXTENSION));
    (named || mime == Some("text/csv"))
        .then(|| mime.map_or(tokens::icon::FILE, ronoctua_files::mime::icon_of))
}

/// WHAT A PATH THE PICKER ANSWERED IS FOR: the list to import, staged on the
/// door's feed; or the file the channels are written to. The first path —
/// the picker takes one at a time for both (`Mode::Open { many: false }`,
/// `Mode::Save`).
fn picked(asked: Picking, paths: &[PathBuf]) -> Errand {
    let Some(path) = paths.first().cloned() else {
        return Errand::Nothing;
    };
    match asked {
        Picking::Import => Errand::Edit(Edit::Import(path)),
        Picking::Export => Errand::Export(path),
    }
}

/// GOOGLE TAKEOUT'S PAGE as its row shows it: its host, which a person reads
/// and could type — the address whole is what Chrome opens.
fn takeout_shown() -> String {
    takeout::TAKEOUT_PAGE
        .split_once("://")
        .map_or(takeout::TAKEOUT_PAGE, |(_, rest)| rest)
        .trim_end_matches('/')
        .to_owned()
}

impl View {
    /// WHETHER A PAGE BELOW THE FIRST IS OPEN — where the window's first
    /// line says its way back, with no header (`door::corner`). The trail
    /// always holds the home page at its foot ([`View::fresh`]), so a page
    /// below it is a second step, as [`View::back`] counts it: an empty
    /// trail is never what "home" looks like.
    #[must_use]
    pub fn below_the_first(&self) -> bool {
        self.trail.len() > 1
    }

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
            per_page: self.per_page.get(),
            pictures: self.layout.pictures(),
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

    /// THE VIDEOS LAID OUT SO, as the ring leaves them — for a test of a
    /// door, which reaches no field of the view and would otherwise turn the
    /// ring until it got there.
    #[cfg(test)]
    pub(crate) const fn laid_out(&mut self, layout: Layout) {
        self.layout = layout;
    }

    /// Whether the cursor moved without a key since the door last asked —
    /// a question the view put on the page from what an errand came to. The
    /// door follows it as it follows a key: it is the consequence of one.
    pub fn follow_owed(&mut self) -> bool {
        std::mem::take(&mut self.follow_owed)
    }

    /// THE LIST'S TITLE FIRST — paid ONCE, when a turned page owed it the top
    /// of the door (a page key, an arrow, a new size) and the reading of the
    /// page ASKED is in, its filter, its page and its size: the cursor goes
    /// to the list's first video — its row, or its shelf's first cell — and
    /// the answer is the line of the title, for the door to put first
    /// (`Ui::place_top` in the window, `Paging::Top` in the footer's
    /// region), so what the person turned to is what they see. `None` while
    /// the page is still being read — the page drawn stays as it is — and
    /// once paid.
    pub fn settle_on_list(&mut self, rows: &[Row], r: &Readings) -> Option<usize> {
        if !self.top_owed {
            return None;
        }
        if *self.page() != Page::Videos || self.editing.is_some() || self.picking.is_some() {
            self.top_owed = false;
            return None;
        }
        // THE LAYOUT IS NOT ASKED ABOUT: a list read with its pictures is the
        // same list.
        let read = r
            .listed
            .as_ref()
            .zip(self.look().query)
            .is_some_and(|(l, q)| {
                l.query.filter == q.filter
                    && l.query.page == q.page
                    && l.query.per_page == q.per_page
            });
        if !read {
            return None;
        }
        self.top_owed = false;
        let title = rows
            .iter()
            .position(|x| x.spot == crate::paging::LIST_TITLE)?;
        self.title_held = true;
        if let Some((i, row)) = rows
            .iter()
            .enumerate()
            .skip(title + 1)
            .find(|(_, x)| x.stops())
        {
            self.at = row
                .cells
                .first()
                .map_or_else(|| row.spot.clone(), |c| c.spot.clone());
            self.last = i;
        }
        Some(title)
    }

    /// THE VIEW IS A WINDOW'S: what a sentence teaches of a key, it teaches
    /// as `LEADER Alt` reaches it there ([`View::page_keys`]).
    pub fn in_a_window(&mut self) {
        self.windowed = true;
    }

    /// THE KEYS THAT TURN THE LIST'S PAGES, as this view's door reaches them —
    /// built from [`OF_THE_VIEW`], never words in a `.ftl`
    /// (`keys-are-never-text`): each command's first key — the quick one its
    /// hint teaches, `[` and `]` — then the page keys, `PgUp`/`PgDn`, and
    /// `Home`/`End`. In a window the quick keys are `LEADER Alt`'s (`⌥[`);
    /// the page keys stay direct in both doors. The page row's sentence's
    /// arguments, by name.
    #[must_use]
    pub fn page_keys(&self) -> [(&'static str, String); 6] {
        let keys = |id: &str| {
            crate::player::keys_said(
                OF_THE_VIEW.iter().map(|(k, i, _)| (*k, *i)),
                id,
                &[LEADER_ALT_KEYS],
                self.windowed,
            )
        };
        let [back, on, first, last] = [
            "video-metrics-page-back",
            "video-metrics-page-on",
            "video-metrics-page-first",
            "video-metrics-page-last",
        ]
        .map(keys);
        let nth = |keys: &[String], n: usize| keys.get(n).cloned().unwrap_or_default();
        [
            ("earlier", nth(&back, 0)),
            ("prior", nth(&back, 1)),
            ("later", nth(&on, 0)),
            ("next", nth(&on, 1)),
            ("first", nth(&first, 0)),
            ("last", nth(&last, 0)),
        ]
    }

    /// A ROW'S SENTENCE, said — and the page row's with the keys that turn
    /// the page, as this door reaches them ([`View::page_keys`]).
    fn said(&self, k: &'static str, texts: &ronoctua_i18n::Texts) -> String {
        if k != key::ABOUT_VIDEO_METRICS_PAGE {
            return texts.t(k);
        }
        let keys = self.page_keys();
        let args: Vec<(&str, &str)> = keys.iter().map(|(n, v)| (*n, v.as_str())).collect();
        texts.t_with(k, &args)
    }

    /// Whether a description is being edited — every key is its.
    #[must_use]
    pub const fn editing(&self) -> bool {
        self.editing.is_some()
    }

    /// WHETHER THE FILE PICKER IS UP over the import page — every key it
    /// claims is its ([`View::picker_takes`]), and the window keeps none of
    /// its letters from it.
    #[must_use]
    pub const fn picking(&self) -> bool {
        self.picking.is_some()
    }

    /// The door's wake, for the file picker's folder and devices: a door
    /// with a loop hands it once, and every picker this view opens after is
    /// watched through it (`file_picker::FilePicker::watch_with`). A
    /// photograph hands none, and its picker watches nothing.
    pub fn watch_with(&mut self, wake: crate::wake::Wake) {
        self.wake = Some(wake);
    }

    /// What the wake brought the file picker, taken — `true` when its rows
    /// changed and the door draws again.
    pub fn picker_woke(&mut self) -> bool {
        self.picking
            .as_mut()
            .is_some_and(crate::file_picker::Hosted::look)
    }

    /// How many lines of rows the door shows, for the file picker's `PgUp`
    /// and `PgDn` — told before a key is handed over — and the room a held
    /// title keeps under it ([`View::title_kept`]): a window tells it again
    /// with every rows it builds, so a window made taller keeps it too.
    pub fn page_is(&self, lines: usize) {
        self.page_lines.set(lines);
        if let Some(h) = self.picking.as_ref() {
            h.picker.page_is(u16::try_from(lines).unwrap_or(u16::MAX));
        }
    }

    /// The most lines the door's page can hold, when it shows fewer now —
    /// the footer's region (`region::Page::most`), a row short while the
    /// explanation under its hint is up, and as short as its rows under its
    /// ceiling. The room a held title keeps under it is never less
    /// ([`View::title_kept`]): the region grows back between gestures with
    /// nothing told, and room sized off the lines it showed at the gesture
    /// let the row above the title slide back into sight. The picker's half
    /// a screen is still the lines shown ([`View::page_is`]).
    pub fn page_holds(&self, most: usize) {
        self.page_most.set(most);
    }

    /// Whether the file picker is up and takes `key` — the door hands it as
    /// [`Press::Pick`] before anything else reads it.
    #[must_use]
    pub fn picker_takes(&self, key: ronoctua_keymap::Key) -> bool {
        self.picking.as_ref().is_some_and(|h| h.picker.claims(key))
    }

    /// What was copied, for the file picker — as what was typed, a
    /// `file://` address as the path it names. Answers whether a frame is
    /// owed.
    pub fn picker_pasted(&mut self, text: &str) -> bool {
        let Some(h) = self.picking.as_mut() else {
            return false;
        };
        let _ = h.pasted(text);
        true
    }

    /// THE FILE PICKER, up over the import page for `asked`: a list of
    /// channels to import, or the file they are saved to — Google's name
    /// for it first (`takeout::FILE_NAME`). In the person's downloads, where
    /// Takeout's list lands, or where this picker was left last time.
    fn choose(&mut self, asked: Picking) {
        use crate::file_picker::{Hosted, Mode, Request, Start, Xdg};
        let (mode, title) = match asked {
            Picking::Import => (Mode::Open { many: false }, key::VIDEO_METRICS_PICK_IMPORT),
            Picking::Export => (
                Mode::Save {
                    name: takeout::FILE_NAME.to_owned(),
                    extension: Some(LIST_EXTENSION),
                },
                key::VIDEO_METRICS_PICK_EXPORT,
            ),
        };
        self.picking = Some(Hosted::open(
            Request {
                purpose: "video-metrics-list",
                mode,
                filters: LISTS,
                start: Start::Place(Xdg::DOWNLOAD),
                addresses: false,
                extras: &[],
                alternate: None,
            },
            asked,
            title,
            self.wake.as_ref(),
        ));
    }

    /// A key for the file picker — and what it answered, as the view's
    /// errand: a list to stage, a file to write the channels to. Going back
    /// brings the page's rows back, the cursor on the row that opened it.
    fn pick_key(&mut self, key: ronoctua_keymap::Key) -> Errand {
        use crate::file_picker::{Hosting, Picked};
        let Some(h) = self.picking.as_mut() else {
            return Errand::Nothing;
        };
        match h.key(key) {
            Hosting::Nothing => Errand::Nothing,
            Hosting::Back => {
                self.picking = None;
                Errand::Nothing
            }
            Hosting::Say(e) => Errand::Explain(e),
            Hosting::Paste => Errand::Paste,
            Hosting::Picked(asked, Picked::Paths { paths, .. }) => {
                self.picking = None;
                picked(asked, &paths)
            }
            // NEITHER AN ADDRESS NOR AN EXTRA is asked for here
            // (`Request::addresses`, `Request::extras`): none comes back.
            Hosting::Picked(..) => Errand::Nothing,
        }
    }

    /// The row the cursor is on, among `rows` — the file picker's own, while
    /// it is up: the line it drew its cursor on.
    #[must_use]
    pub fn cursor(&self, rows: &[Row]) -> usize {
        if self.picking.is_some() {
            return self.picked_at.get().min(rows.len().saturating_sub(1));
        }
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
    /// reading arrived. It never moves the page. Under the file picker the
    /// page's cursor is kept where it was: the picker has its own.
    pub fn settle(&mut self, rows: &[Row]) {
        if self.picking.is_some() {
            return;
        }
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
        if let Some(h) = self.picking.as_ref() {
            return h.picker.hint(texts);
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
            let asking = self.editing.as_ref().and_then(|(_, a)| a.asking());
            return text_cursor(rows, asking).unwrap_or(0);
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
            Where::OnAnAddress => {
                self.confirm.is_none()
                    && (self.video_here(target).is_some() || self.channel_here(target).is_some())
            }
        }
    }

    /// Whether the cursor is on a cell of the grid.
    fn on_the_grid(&self, target: &Target) -> bool {
        self.layout.grid() && *self.page() == Page::Videos && matches!(target, Target::Video(_))
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

    /// THE CHANNEL THE CURSOR IS ABOUT: the page's, or the row of the
    /// channels' list it is on — whose videos' page on YouTube a browser
    /// opens ([`View::on_a_channel`]).
    fn channel_here(&self, target: Option<&Target>) -> Option<ChannelId> {
        match (self.page(), target) {
            (Page::Channel(id), _) => Some(id.clone()),
            (Page::Channels, Some(Target::Go(Page::Channel(id)))) => Some(id.clone()),
            // A CHANNEL OF THE LIST IMPORTED — staged, or found registered
            // already — is looked at before it is decided: its videos'
            // page, in either browser.
            (Page::Import, Some(Target::Staged(id, _) | Target::Go(Page::Channel(id)))) => {
                Some(id.clone())
            }
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
    /// what its target is ([`about_of`]).
    #[must_use]
    pub fn about(&self, rows: &[Row], n: usize, texts: &ronoctua_i18n::Texts) -> Option<String> {
        sentence_of(rows.get(Row::owner(rows, n))?).map(|k| self.said(k, texts))
    }

    /// WHAT THE THING ON LINE `n` IS, as a door explains it — the window's
    /// foot, the footer's explanation line (`door::Explained`): its name,
    /// what the hand does to it ([`crate::door::foot::gesture`]) and its
    /// sentence, with what a row set "as everywhere" follows now under it in
    /// the quietest ink ([`Follows`]).
    ///
    /// `column` is where the pointer is across the line, as a field's `at`:
    /// on a shelf it says WHICH video (`None` there is none — a shelf is
    /// several things, and its line names none of them), and a glyph under
    /// it ([`Control`]) acts at once. `None` for a line no click takes — a
    /// title, a note, the air, a picture's line —, which explains nothing.
    /// ON A NAME'S WORDS — a video's title, its channel ([`Row::wholes`]) —
    /// the thing is named by that name whole ([`View::name_at`]).
    #[must_use]
    pub fn explained_at(
        &self,
        rows: &[Row],
        n: usize,
        column: Option<u16>,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<crate::door::Explained> {
        let pointed = self.pointed(rows, n, column)?;
        let mut said = self.said_at(pointed, rows, texts);
        // ON A NAME'S WORDS — a title, a channel — the thing is NAMED by it,
        // whole: the room cut it, and the channel is not the video's title.
        if !matches!(pointed, Pointed::Glyph(..))
            && let Some(name) = whole_on(rows, n, column)
        {
            name.clone_into(&mut said.name);
        }
        Some(said)
    }

    /// [`View::explained_at`]'s thing, explained by what it is.
    fn said_at(
        &self,
        pointed: Pointed<'_>,
        rows: &[Row],
        texts: &ronoctua_i18n::Texts,
    ) -> crate::door::Explained {
        use crate::door::Gesture;
        match pointed {
            // A GLYPH acts on the first click — and says, quietly under its
            // sentence, what holds once it is switched off.
            Pointed::Glyph(_, _, c) => crate::door::Explained {
                subject: pointed.subject(),
                name: c.name.clone(),
                gesture: self.gesture(&c.target, Gesture::AtOnce, texts),
                said: std::iter::once((texts.t(c.about), tokens::color::TEXT_SECONDARY))
                    .chain(
                        c.follows
                            .iter()
                            .map(|f| (f.clone(), tokens::color::TEXT_DIM)),
                    )
                    .collect(),
            },
            Pointed::Cell(cell) => {
                let how = if cell.spot == self.at {
                    Gesture::Ret
                } else {
                    Gesture::Selects
                };
                self.explained_cell(cell, how, texts)
            }
            Pointed::Row(owner, row) => {
                let how = if self.drawn_cursor(rows) == owner {
                    Gesture::Ret
                } else {
                    Gesture::Selects
                };
                self.explained_row(row, how, texts)
            }
        }
    }

    /// THE NAME WHOLE at cell `column` of line `n` — a video's title, its
    /// channel, cut to a tile or wrapped over a row's lines ([`Row::wholes`])
    /// — where the pointer is on a thing that names itself by it: a video of
    /// a shelf or a row, never a glyph, and only on the name's own cells.
    #[must_use]
    pub fn name_at<'r>(&self, rows: &'r [Row], n: usize, column: Option<u16>) -> Option<&'r str> {
        match self.pointed(rows, n, column)? {
            Pointed::Glyph(..) => None,
            Pointed::Cell(_) | Pointed::Row(..) => whole_on(rows, n, column),
        }
    }

    /// WHAT THE THING AT CELL `column` OF LINE `n` IS, by the identity the
    /// foot explains it under (`Explained::subject`) — a row, a video of a
    /// shelf, a glyph; `None` on what explains nothing. What a door keys the
    /// pointer's REST on: the thing, never the line — a shelf's line holds
    /// several videos, and one video spans the shelf's lines.
    #[must_use]
    pub fn thing_at(&self, rows: &[Row], n: usize, column: Option<u16>) -> Option<String> {
        self.pointed(rows, n, column).map(|p| p.subject())
    }

    /// Whether cell `column` of line `n` is a GLYPH — a picture, which the
    /// footer's one line names before explaining (`Explained::joined_named`).
    #[must_use]
    pub fn on_a_glyph(&self, rows: &[Row], n: usize, column: Option<u16>) -> bool {
        matches!(self.pointed(rows, n, column), Some(Pointed::Glyph(..)))
    }

    /// WHAT IS AT CELL `column` OF LINE `n` ([`View::explained_at`]'s
    /// `column`) — the ONE answer the foot's explanation and the rest's key
    /// ([`View::thing_at`]) read: a glyph under it; then the row's words,
    /// only where [`View::on_the_words`] says (the box the pointer lights);
    /// on a shelf, the cell under it; any other line, its row.
    fn pointed<'a>(&self, rows: &'a [Row], n: usize, column: Option<u16>) -> Option<Pointed<'a>> {
        let line = rows.get(n)?;
        if !self.takes_one(rows, n) {
            return None;
        }
        if let Some(col) = column
            && let Some((i, c)) = line
                .controls
                .iter()
                .enumerate()
                .find(|(_, c)| crate::chrome::in_cell(col, c.from, c.width))
        {
            return Some(Pointed::Glyph(line, i, c));
        }
        if !self.on_the_words(rows, n, column) {
            return None;
        }
        let owner = Row::owner(rows, n);
        let row = rows.get(owner)?;
        if !row.cells.is_empty() {
            let col = column?;
            return row
                .cells
                .iter()
                .find(|c| crate::chrome::in_cell(col, c.from, c.width))
                .map(Pointed::Cell);
        }
        Some(Pointed::Row(owner, row))
    }

    /// WHETHER CELL `column` OF LINE `n` IS ITS ROW'S WORDS — what the
    /// pointer lights there ([`Row::lit_cells`]), the foot explains and a
    /// click takes ([`View::clicked`]): every cell of a line with no glyphs
    /// — but on a line beside a picture, only from where its words begin
    /// ([`Row::words_at`]); on one with glyphs, only its words' box
    /// ([`Row::words_box`]) — never a glyph, which acts on its own, nor the
    /// air before it, the margin or a line of glyphs alone. With no column
    /// (`None`), whether the line has words at all.
    ///
    /// THE PICTURE IS NOBODY'S on every line of its video, as it already
    /// was on the line of glyphs, whose words' box starts past it (the plan,
    /// 2026-11-10: "a luz e o clique valem a partir do texto … nunca sobre a
    /// figura"): a click on it once took the video on the title's lines and
    /// nothing on the line of glyphs — a picture clickable but for a stripe
    /// through it.
    ///
    /// ON A SHELF, a line's glyphs are theirs and every other cell of it is
    /// its videos' — the cell under the pointer is the video, on the line of
    /// glyphs as on the picture's (the owner, 2026-11-11): a wide tile says
    /// its score beside its picture, and a stripe of nobody's would cut the
    /// picture.
    #[must_use]
    pub fn on_the_words(&self, rows: &[Row], n: usize, column: Option<u16>) -> bool {
        let Some(line) = rows.get(n) else {
            return false;
        };
        if line.controls.is_empty() {
            return line
                .words_at
                .is_none_or(|at| column.is_none_or(|c| c >= at));
        }
        if rows
            .get(Row::owner(rows, n))
            .is_some_and(|o| !o.cells.is_empty())
        {
            return column.is_none_or(|c| {
                !line
                    .controls
                    .iter()
                    .any(|g| crate::chrome::in_cell(c, g.from, g.width))
            });
        }
        let owner_stops = rows.get(Row::owner(rows, n)).is_some_and(Row::has_words);
        line.words_box(self.left.get(), owner_stops)
            .is_some_and(|(from, width)| {
                column.is_none_or(|c| crate::chrome::in_cell(c, from, width))
            })
    }

    /// WHAT THE CURSOR IS ON, as a door explains it ([`View::explained_at`]):
    /// its row, or its cell of a shelf — which `RET` does, as a second
    /// click on it would. The caret's line, or the answer, while a text is
    /// edited. `None` where the cursor rests on nothing a key takes (a page
    /// still being read).
    #[must_use]
    pub fn explained_cursor(
        &self,
        rows: &[Row],
        texts: &ronoctua_i18n::Texts,
    ) -> Option<crate::door::Explained> {
        use crate::door::Gesture;
        let n = self.drawn_cursor(rows);
        let row = rows.get(n)?;
        if self.editing.is_none()
            && let Some(cell) = row.cells.iter().find(|c| c.spot == self.at)
        {
            return Some(self.explained_cell(cell, Gesture::Ret, texts));
        }
        if !self.takes_one(rows, n) {
            return None;
        }
        Some(self.explained_row(row, Gesture::Ret, texts))
    }

    /// A row, explained: its name, the gesture `how` and its sentence — and
    /// what it follows "as everywhere", under it.
    fn explained_row(
        &self,
        row: &Row,
        how: crate::door::Gesture,
        texts: &ronoctua_i18n::Texts,
    ) -> crate::door::Explained {
        let mut said: Vec<(String, tokens::Color)> = sentence_of(row)
            .map(|k| (self.said(k, texts), tokens::color::TEXT_SECONDARY))
            .into_iter()
            .collect();
        if let Some(f) = &row.default {
            said.push((f.said.clone(), tokens::color::TEXT_DIM));
        }
        crate::door::Explained {
            subject: row.spot.clone(),
            name: row.name.clone(),
            gesture: self.gesture(&row.target, how, texts),
            said,
        }
    }

    /// A cell of a shelf, explained: its video.
    fn explained_cell(
        &self,
        cell: &Cell,
        how: crate::door::Gesture,
        texts: &ronoctua_i18n::Texts,
    ) -> crate::door::Explained {
        crate::door::Explained {
            subject: cell.spot.clone(),
            name: cell.name.clone(),
            gesture: self.gesture(&cell.target, how, texts),
            said: vec![(texts.t(cell.about), tokens::color::TEXT_SECONDARY)],
        }
    }

    /// WHAT THE HAND DOES to a thing whose target is `target`, taken `how`
    /// — the door's words for the click or the key, and the view's for what
    /// it does ([`does_of`]). None for a thing that does nothing but be
    /// selected (a title of a fact, a line of the text being edited), and,
    /// while a question is on the page, for everything but its answers: a
    /// second click anywhere else gives the question the answer its cursor
    /// is on (`View::confirm_press`), and promising the row's own would lie.
    /// A GLYPH keeps its word under a question: its click lets the question
    /// go — "no", which loses nothing — and does what it says
    /// ([`View::click_control`]).
    fn gesture(
        &self,
        target: &Target,
        how: crate::door::Gesture,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<String> {
        if self.confirm.is_some()
            && how != crate::door::Gesture::AtOnce
            && !matches!(target, Target::Answer(_))
        {
            return None;
        }
        let does = does_of(target)?;
        Some(crate::door::foot::gesture(texts, how, &texts.t(does)))
    }

    /// WHICH OF `rows` A CLICK TAKES, row by row — and so the only rows a
    /// resting pointer lights, in both doors: the region's `Row::takes` and
    /// the window's `Content::takes` read this, and [`View::clicked`] reads
    /// the same [`Row::takes`]. While a description is edited, only its
    /// lines and its question's answers take one ([`text_hit`]): a click
    /// anywhere else does nothing.
    ///
    /// ONE ANSWER FOR THE LIGHT AND THE CLICK. The region said it in a line
    /// of its own, and the window lit by another, which knew nothing of a
    /// row of several lines.
    #[must_use]
    pub fn takes(&self, rows: &[Row]) -> Vec<bool> {
        (0..rows.len()).map(|n| self.takes_one(rows, n)).collect()
    }

    /// [`View::takes`], for line `n` alone.
    #[must_use]
    pub fn takes_one(&self, rows: &[Row], n: usize) -> bool {
        if self.editing.is_some() {
            text_hit(rows, n, 0).is_some()
        } else {
            rows.get(n).is_some_and(Row::takes)
        }
    }

    /// Puts the cursor where a click landed — on a row, or on the cell of a
    /// shelf at `column` (a field's `at`), and on no row that takes no click
    /// ([`Row::takes`]). Answers whether the cursor was ALREADY there: that
    /// is the second click, which the door turns into `RET` (invariant 19).
    /// A click on a line being edited places the caret.
    pub fn clicked(&mut self, n: usize, column: u16, rows: &[Row]) -> bool {
        if !rows.get(n).is_some_and(Row::takes) {
            return false;
        }
        // A ROW OF THE FILE PICKER is chosen there: its cursor, and the
        // second click its `RET`.
        if let Some(h) = self.picking.as_mut() {
            return match rows.get(n).map(|r| &r.target) {
                Some(Target::Pick(t)) => h.clicked(t.clone()),
                _ => false,
            };
        }
        // ON A LINE WITH GLYPHS, only its words select the row — the box the
        // pointer lights and the foot explains. A glyph acts on its own, and
        // the air before it takes nothing.
        if !self.on_the_words(rows, n, Some(column)) {
            return false;
        }
        let owner = Row::owner(rows, n);
        let Some(row) = rows.get(owner) else {
            return false;
        };
        // A SHELF: the cell under the column, on any of its lines — and the
        // air between two cells, or past the last, takes nothing: it lights
        // nothing either (`region::LineLights::UnderCell`).
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
        // THE FILE PICKER takes its own keys, and the view's that walk,
        // answer and go back — a click on the hint, a second click's `RET`,
        // the pointer's way back. The page's other keys have nothing to act
        // on under it.
        if self.picking.is_some() {
            use ronoctua_keymap::{Key, Sym};
            let key = match press {
                Press::Pick(key) => key,
                Press::Up => Key::simple(Sym::Up),
                Press::Down => Key::simple(Sym::Down),
                Press::Less => Key::simple(Sym::Left),
                Press::More => Key::simple(Sym::Right),
                Press::Enter => Key::simple(Sym::Enter),
                Press::Escape => Key::simple(Sym::Esc),
                Press::PageBack => Key::simple(Sym::PageUp),
                Press::PageOn => Key::simple(Sym::PageDown),
                Press::PageFirst => Key::simple(Sym::Home),
                Press::PageLast => Key::simple(Sym::End),
                _ => return Errand::Nothing,
            };
            return self.pick_key(key);
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
            Press::PageFirst => self.page_to(Some(0), r),
            Press::PageLast => self.page_to(None, r),
            Press::Layout => self.relayout(true),
            Press::Search => self.ask(
                Ask::Search,
                self.filter.text.clone(),
                Some(Target::FilterText),
            ),
            // A VIDEO'S KEYS on a video; a browser's on a channel too —
            // its videos' page.
            Press::Player
            | Press::Kiosk
            | Press::Nyxt
            | Press::Download
            | Press::Hide
            | Press::Delete
            | Press::SetAside
            | Press::Keep => match (
                self.video_here(Some(&target)),
                self.channel_here(Some(&target)),
            ) {
                (Some(id), _) => self.on_a_video(press, &id, r),
                (None, Some(id)) => on_a_channel(press, &id, r),
                (None, None) => Errand::Nothing,
            },
            // A KEY OF THE PICKER'S with no picker up — it went while the key
            // was on its way — is nobody's.
            Press::Pick(_) => Errand::Nothing,
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
        // THE TITLE HELD IS THE PAGE'S, and goes with it.
        self.title_held = false;
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
        self.title_held = false;
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
        self.picking = None;
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
            Target::Nyxt(id) => self.on_a_video(Press::Nyxt, id, r),
            Target::ChannelKiosk(id) => on_a_channel(Press::Kiosk, id, r),
            Target::ChannelNyxt(id) => on_a_channel(Press::Nyxt, id, r),
            // OUT OF THE VIEW, to another program's door: the brain's page
            // on the scan, a window of the player's on one of its lists.
            Target::Brain => Errand::Brain,
            Target::InThePlayer(list) => Errand::Player(list.word()),
            Target::Download(id) => self.on_a_video(Press::Download, id, r),
            Target::Hide(id, _) => self.on_a_video(Press::Hide, id, r),
            Target::Delete(id) => self.on_a_video(Press::Delete, id, r),
            // THE SET-ASIDE VIDEOS, the last set aside first — the hidden
            // ones among them too: set aside for later, a video is the
            // person's to come back to, hidden or not.
            Target::SetAsideVideos => {
                self.filter = Filter {
                    set_aside: SetAside::Only,
                    hidden: Hidden::Any,
                    order: Order::NewestSetAside,
                    ..Filter::default()
                };
                self.since = Since::Any;
                self.page = 0;
                self.go(Page::Videos)
            }
            // A KEEP'S RING IS TYPED: any number of days, and not only the
            // ring's — from the days in force, a channel's "as everywhere"
            // from the ones everywhere.
            Target::KeepEverywhere(now) => {
                self.ask(Ask::KeepDays, days_of(Some(*now)), Some(target.clone()))
            }
            Target::ChannelKeep(_, now) => self.ask(
                Ask::KeepDays,
                days_of(now.or_else(|| r.settings.map(|s| s.keep))),
                Some(target.clone()),
            ),
            // AN ARROW OF THE RATING gives it — and the one the video has
            // takes it away.
            Target::Rate(id, rating, lit) => {
                Errand::Edit(Edit::Rating(id.clone(), (!*lit).then_some(*rating)))
            }
            // A STAGED CHANNEL'S PAGE, where everything a channel is given is
            // given before it is registered; the arrows turn its decision
            // (`View::turn`) — a score's precedent: the arrows step, `RET`
            // does something else.
            Target::Staged(id, _) => self.go(Page::Channel(id.clone())),
            // WHERE THE LIST IS EXPORTED, in Chrome as a kiosk (the
            // footer's `kiosk-open`).
            Target::Takeout => Errand::Kiosk(takeout::TAKEOUT_PAGE.to_owned()),
            Target::PickImport => {
                self.choose(Picking::Import);
                Errand::Nothing
            }
            Target::PickExport => {
                self.choose(Picking::Export);
                Errand::Nothing
            }
            // REGISTERING IS ASKED, "no" first: the discarded go with what
            // was set on them, and the kept are scanned from then on.
            Target::Register => {
                let count = |d: Decision| {
                    u32::try_from(r.staged.iter().filter(|s| s.decision == d).count())
                        .unwrap_or(u32::MAX)
                };
                self.asks(Pending::Register(
                    count(Decision::Keep),
                    count(Decision::Discard),
                ))
            }
            // A ROW OF THE PICKER is the picker's: its keys reach it first.
            Target::Pick(_) => Errand::Nothing,
            // AN ARROW OF THE LIST'S PAGES, beside its title or at its end:
            // the page before or after, as `[` and `]` turn it.
            Target::PageTurn(on) => self.turn_page(*on, r),
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
            // FROM THE SWITCHES READ, as the Shorts' switch: the others
            // would be written back as the defaults.
            Target::KeepEverywhere(_) => {
                let Some(mut s) = r.settings else {
                    return Errand::Nothing;
                };
                s.keep = ring_step(&keep_ring(Some(s.keep)), &s.keep, on);
                Errand::Edit(Edit::Settings(s))
            }
            // "AS EVERYWHERE" FIRST, then the ring everywhere has.
            Target::ChannelKeep(id, now) => {
                let ring: Vec<Option<Keep>> = std::iter::once(None)
                    .chain(keep_ring(*now).into_iter().map(Some))
                    .collect();
                Errand::Edit(Edit::ChannelKeep(id.clone(), ring_step(&ring, now, on)))
            }
            Target::SetAside(id, now) => Errand::Edit(Edit::SetAside(id.clone(), !now)),
            Target::VideoKeep(id, now) => Errand::Edit(Edit::KeepVideo(id.clone(), !now)),
            // WHAT REGISTERING DOES WITH A STAGED CHANNEL: undecided, kept,
            // discarded — on its row of the list, and on its page.
            Target::Staged(id, now) | Target::Decide(id, now) => {
                Errand::Edit(Edit::Decide(id.clone(), ring_step(&Decision::ALL, now, on)))
            }
            // THE SAME FOR EVERY ONE. While theirs are alike the ring turns as
            // any ring does: a turn back gives each what it had. While theirs
            // differ, a turn would throw away what each one was given, and no
            // turn gives that back — so it is ASKED, "no" first, saying how
            // many it changes, from the first of the ring. `RET` reaches here
            // too, and a double click is a `RET`.
            Target::DecideAll(now) => {
                let ring = Decision::ALL.map(Some);
                match (ring_step(&ring, now, on), now) {
                    (Some(d), Some(_)) => Errand::Edit(Edit::DecideAll(d)),
                    (Some(d), None) => {
                        let changed = r.staged.iter().filter(|s| s.decision != d).count();
                        self.asks(Pending::DecideAll(
                            d,
                            u32::try_from(changed).unwrap_or(u32::MAX),
                        ))
                    }
                    (None, _) => Errand::Nothing,
                }
            }
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
            Target::FilterSetAside => {
                let ring = [SetAside::Any, SetAside::Only, SetAside::Without];
                self.filter.set_aside = ring_step(&ring, &self.filter.set_aside, on);
                // ONLY THEM, THE LAST SET ASIDE FIRST: a list of what was put
                // aside for later is read by when; any other, by score.
                self.filter.order = if self.filter.set_aside == SetAside::Only {
                    Order::NewestSetAside
                } else {
                    Order::Score
                };
                self.refiltered()
            }
            Target::Layout => self.relayout(on),
            Target::PageOf => self.turn_page(on, r),
            Target::PerPage => self.resize(on),
            Target::Hide(id, _) => self.on_a_video(Press::Hide, id, r),
            _ => Errand::Nothing,
        }
    }

    /// The list read again from its first page, the filter changed.
    fn refiltered(&mut self) -> Errand {
        self.page = 0;
        Errand::Look(self.look())
    }

    /// THE NEXT LAYOUT OF THE RING, or the one before (`on`) — the cursor
    /// stays on its video: its row, or its cell of a shelf, has its spot in
    /// every layout. Read again only when the thumbnails come or go: the
    /// three layouts that draw them draw the same ones, and a reading asked
    /// for nothing new would read the catalogue for nothing.
    fn relayout(&mut self, on: bool) -> Errand {
        if *self.page() != Page::Videos {
            return Errand::Nothing;
        }
        let pictures = self.layout.pictures();
        self.layout = ring_step(&Layout::ALL, &self.layout, on);
        if self.layout.pictures() == pictures {
            Errand::Nothing
        } else {
            Errand::Look(self.look())
        }
    }

    /// The page of the list before or after — never past either end.
    fn turn_page(&mut self, on: bool, r: &Readings) -> Errand {
        let to = if on {
            Some(self.page + 1)
        } else {
            self.page.checked_sub(1)
        };
        match to {
            Some(to) => self.page_to(Some(to), r),
            None => Errand::Nothing,
        }
    }

    /// PAGE `to` OF THE LIST — `None` for its last — never past either end:
    /// asked of the feed, and its title owed the top of the door once the
    /// page is read ([`View::settle_on_list`]). The cursor stays where it is
    /// until then — on its video of the page still drawn, which stays drawn
    /// while the next one is read.
    fn page_to(&mut self, to: Option<u32>, r: &Readings) -> Errand {
        if *self.page() != Page::Videos {
            return Errand::Nothing;
        }
        let pages = self.pages(r);
        let last = pages.saturating_sub(1);
        let to = to.map_or(last, |to| to.min(last));
        if to == self.page {
            return Errand::Nothing;
        }
        self.page = to;
        self.top_owed = true;
        Errand::Look(self.look())
    }

    /// HOW MANY VIDEOS A PAGE HOLDS, turned round its ring — and the page
    /// that holds the first video of the one on screen asked, its title
    /// owed the top of the door as a turned page's is.
    fn resize(&mut self, on: bool) -> Errand {
        if *self.page() != Page::Videos {
            return Errand::Nothing;
        }
        let from = self.per_page;
        self.per_page = from.turned(on);
        self.page = crate::paging::resized(self.page, from.get(), self.per_page.get());
        self.top_owed = true;
        Errand::Look(self.look())
    }

    /// How many pages the list takes, at the size asked — as its reading
    /// counts it, under the filter asked.
    fn pages(&self, r: &Readings) -> u32 {
        let total = r
            .listed
            .as_ref()
            .filter(|l| l.query.filter == self.filter)
            .map_or(0, |l| l.total);
        crate::paging::pages_of(total, self.per_page.get())
    }

    /// What a key for a video does to it.
    fn on_a_video(&mut self, press: Press, id: &VideoId, r: &Readings) -> Errand {
        let Some(row) = r.video_of(id).map(|v| &v.row).or_else(|| r.ranked(id)) else {
            return Errand::Nothing;
        };
        let (url, title, hidden) = (row.url.clone(), row.title.clone(), row.hidden);
        // SET ASIDE, AND ITS OWN MARK NEVER TO BE DELETED: switched at once,
        // from the state READ — nothing is lost either way, so nothing is
        // asked. The mark is the video's own, even where its channel keeps
        // every video (Reconciliation 4): it holds if the channel changes.
        let aside = row.set_aside_at_s.is_some();
        let own = row.forever == Forever::ByVideo;
        match press {
            Press::SetAside => return Errand::Edit(Edit::SetAside(id.clone(), !aside)),
            Press::Keep => return Errand::Edit(Edit::KeepVideo(id.clone(), !own)),
            _ => {}
        }
        match press {
            Press::Player => Errand::Play(ToPlay {
                url,
                title,
                once: true,
            }),
            Press::Kiosk => Errand::Kiosk(url),
            Press::Nyxt => Errand::Nyxt(url),
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
            aim: crate::aim::Aim::born(false),
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
            // A KEY THAT MOVED THE CURSOR is a person's gesture: a click on
            // where it now is answers.
            Press::Up | Press::Down | Press::Less | Press::More => {
                let yes = !c.yes();
                c.aim.moved(yes);
                self.at = answer_spot(yes).to_owned();
                Errand::Nothing
            }
            Press::Enter => {
                let yes = c.yes();
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
            Pending::Register(..) => Errand::Edit(Edit::Register),
            Pending::DecideAll(decision, _) => Errand::Edit(Edit::DecideAll(decision)),
        }
    }

    /// A click on an answer: `true` when it answers — the person's own
    /// pick, clicked again ([`Confirm::aim`]). The first click on the "no"
    /// the question was born on only picks it, as a click on the other
    /// answer only moves the cursor there.
    fn answered_by_click(&mut self, yes: bool) -> bool {
        let Some(c) = self.confirm.as_mut() else {
            return false;
        };
        let answers = c.aim.clicked(yes);
        self.at = answer_spot(yes).to_owned();
        answers
    }

    /// A click on an answer — on ANY line of it, a long answer wrapped
    /// being one answer ([`Row::owner`]): the first picks it, and the
    /// second, on the answer picked, gives it — what the door does with a
    /// second click on a row, `RET`.
    pub fn click_answers(&mut self, n: usize, rows: &[Row]) -> Option<Errand> {
        rows.get(n)?;
        let Target::Answer(yes) = rows.get(Row::owner(rows, n))?.target else {
            return None;
        };
        if self.answered_by_click(yes) {
            Some(self.answer(yes))
        } else {
            Some(Errand::Nothing)
        }
    }

    /// A CLICK ON A GLYPH ACTS AT ONCE ([`Control`]) — the first click, and
    /// not a second: an arrow of the rating gives it (the one the video has
    /// takes it away), the bookmark and the ∞ switch, the eye and the bin
    /// ask as their keys do, "no" first. The cursor goes to the video first
    /// — the entry's, or the shelf's cell over `column` (a field's `at`) —
    /// so what is asked is asked under it, and where the hand acted is where
    /// the keys go on. A question already on the page is let go first, as
    /// `Esc` lets it go: the hand moved on. `None` where no glyph is under
    /// `column`: the door goes on to the row's click. The same call from
    /// either door, before [`View::clicked`].
    pub fn click_control(
        &mut self,
        n: usize,
        column: u16,
        rows: &[Row],
        r: &Readings,
    ) -> Option<Errand> {
        if self.editing.is_some() {
            return None;
        }
        let target = rows
            .get(n)?
            .controls
            .iter()
            .find(|c| crate::chrome::in_cell(column, c.from, c.width))?
            .target
            .clone();
        let owner = Row::owner(rows, n);
        let spot = rows.get(owner).and_then(|o| {
            if o.cells.is_empty() {
                o.stops().then(|| o.spot.clone())
            } else {
                o.cells
                    .iter()
                    .find(|c| crate::chrome::in_cell(column, c.from, c.width))
                    .map(|c| c.spot.clone())
            }
        });
        if self.confirm.is_some() {
            let _ = self.answer(false);
        }
        if let Some(spot) = spot {
            self.at = spot;
            self.last = owner;
        }
        // WHAT `RET` DOES ON ITS TARGET, once: a glyph is the target's own
        // row, a click away (`View::enter`).
        Some(self.enter(&target, r))
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
            // A KEEP TYPED: one day to the most the catalogue keeps, and
            // anything else refused with the ring's word for "for ever" —
            // never clamped into a number the person did not type.
            (
                Ask::KeepDays,
                Some(target @ (Target::KeepEverywhere(_) | Target::ChannelKeep(..))),
            ) => {
                let Some(days) = number()
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|n| (1..=ronoctua_video_metrics::KEEP_DAYS_MOST).contains(n))
                else {
                    return Errand::Say(texts.t_args(
                        key::VIDEO_METRICS_TYPED_NOT_KEEP_DAYS,
                        &[
                            (
                                "most",
                                Arg::Number(i64::from(ronoctua_video_metrics::KEEP_DAYS_MOST)),
                            ),
                            ("never", Arg::Text(&texts.t(key::VIDEO_METRICS_KEEP_NEVER))),
                        ],
                    ));
                };
                match target {
                    Target::ChannelKeep(id, _) => {
                        Errand::Edit(Edit::ChannelKeep(id, Some(Keep::Days(days))))
                    }
                    // FROM THE SWITCHES READ, or not at all.
                    _ => match r.settings {
                        Some(mut s) => {
                            s.keep = Keep::Days(days);
                            Errand::Edit(Edit::Settings(s))
                        }
                        None => say(key::VIDEO_METRICS_TYPED_NO_ROW),
                    },
                }
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
        fn long(text: impl Into<crate::chrome::Named>) -> Option<Said> {
            Some(Said::Long(text.into()))
        }
        fn normal(text: impl Into<crate::chrome::Named>) -> Option<Said> {
            Some(Said::Normal(text.into()))
        }
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
                    // WHETHER A VIDEO IS EVER DELETED CHANGED: said, by its
                    // title, in the ranking's words for the same marks — the
                    // ∞ of a video its channel keeps reads the same before
                    // and after its own mark.
                    Edit::SetAside(id, aside) => (
                        r.title_of(&id).map(|title| {
                            Said::Normal(
                                texts
                                    .t_args_marked(
                                        if aside {
                                            key::PLAYER_RANKING_NOW_SET_ASIDE
                                        } else {
                                            key::PLAYER_RANKING_NO_LONGER_SET_ASIDE
                                        },
                                        &[("title", Arg::Name(&title))],
                                    )
                                    .into(),
                            )
                        }),
                        Errand::Nothing,
                    ),
                    Edit::KeepVideo(id, keep) => (
                        r.title_of(&id).map(|title| {
                            Said::Normal(
                                texts
                                    .t_args_marked(
                                        if keep {
                                            key::PLAYER_RANKING_NOW_FOREVER
                                        } else {
                                            key::PLAYER_RANKING_NO_LONGER_FOREVER
                                        },
                                        &[("title", Arg::Name(&title))],
                                    )
                                    .into(),
                            )
                        }),
                        Errand::Nothing,
                    ),
                    _ => (None, Errand::Nothing),
                }
            }
            Done::Edited(_, Err(why)) => (long(texts.t(words::why_key(why))), Errand::Nothing),
            Done::Added(Ok(name)) => (
                normal(texts.t_args_marked(
                    key::VIDEO_METRICS_CHANNEL_ADDED,
                    &[("name", Arg::Name(&name))],
                )),
                Errand::Nothing,
            ),
            Done::Added(Err(Unadded::Unresolved(k))) => (long(texts.t(k)), Errand::Nothing),
            Done::Added(Err(Unadded::Refused(why))) => {
                (long(texts.t(words::why_key(why))), Errand::Nothing)
            }
            Done::Added(Err(Unadded::NotLet(k))) | Done::NotLet(k) => {
                (long(texts.t(k)), Errand::Nothing)
            }
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
                normal(texts.t_args_marked(key::PLAYER_ADDED, &[("name", Arg::Name(&play.title))])),
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
                long(crate::player::trouble_named(&t, texts)),
                Errand::Nothing,
            ),
            Done::Queued(_, Err(Queued::Unreachable)) => (
                long(texts.t(key::VIDEO_METRICS_PLAYER_UNREACHABLE)),
                Errand::Nothing,
            ),
            // HANDED OVER: the queue is opened where it is seen going (the
            // design's `t t`), and the sentence says how to come back to it.
            Done::Downloading(title, true) => (
                normal(texts.t_args_marked(
                    key::VIDEO_METRICS_DOWNLOADING,
                    &[
                        ("title", Arg::Name(&title)),
                        ("keys", Arg::Text(&r.transfers_keys)),
                    ],
                )),
                Errand::Transfers,
            ),
            Done::Downloading(_, false) => (long(texts.t(key::TRANSFERS_OFFLINE)), Errand::Nothing),
            // A LIST STAGED: what it left waiting and what was here already —
            // and the page says the rest, the lines left out among it, while
            // the view is open. The feed reads the page again after every
            // write, the channels staged with it.
            Done::Imported(Ok(i)) => {
                let said = texts.t_args(
                    key::VIDEO_METRICS_IMPORTED,
                    &[
                        ("staged", Arg::Number(i64::from(i.staged))),
                        (
                            "already",
                            Arg::Number(i64::try_from(i.already.len()).unwrap_or(i64::MAX)),
                        ),
                    ],
                );
                self.imported = Some(i);
                (normal(said), Errand::Nothing)
            }
            Done::Imported(Err(e)) => (
                long(match e {
                    Unimported::Unreadable => texts.t(key::VIDEO_METRICS_IMPORT_UNREADABLE),
                    Unimported::TooLarge => texts.t(key::VIDEO_METRICS_IMPORT_TOO_LARGE),
                    Unimported::Unread(takeout::Unread::NotText) => {
                        texts.t(key::VIDEO_METRICS_IMPORT_NOT_TEXT)
                    }
                    Unimported::Unread(takeout::Unread::NotAList) => {
                        texts.t(key::VIDEO_METRICS_IMPORT_NOT_A_LIST)
                    }
                    Unimported::Unread(takeout::Unread::Unclosed { line }) => texts.t_args(
                        key::VIDEO_METRICS_IMPORT_UNCLOSED,
                        &[("line", Arg::Number(i64::from(line)))],
                    ),
                    Unimported::Refused(why) => texts.t(words::why_key(why)),
                }),
                Errand::Nothing,
            ),
            Done::Registered(Ok((done, unkept))) => {
                let registered = texts.t_args(
                    key::VIDEO_METRICS_REGISTERED,
                    &[
                        ("kept", Arg::Number(i64::from(done.kept))),
                        ("discarded", Arg::Number(i64::from(done.discarded))),
                    ],
                );
                match unkept {
                    None => (normal(registered), Errand::Nothing),
                    // REGISTERED, AND THE CONSENT TO SCAN THEM NOT KEPT: what
                    // went in first — how many, never "the channel" — then
                    // why a scan will not ask, as long as a trouble is said.
                    Some(k) => (
                        long(format!("{registered} {}", texts.t(k))),
                        Errand::Nothing,
                    ),
                }
            }
            Done::Registered(Err(why)) => (long(texts.t(words::why_key(why))), Errand::Nothing),
            Done::Exported(Ok((count, file))) => (
                normal(texts.t_args_marked(
                    key::VIDEO_METRICS_EXPORTED,
                    &[
                        ("count", Arg::Number(i64::from(count))),
                        ("file", Arg::Name(&file.to_string_lossy())),
                    ],
                )),
                Errand::Nothing,
            ),
            Done::Exported(Err(Unexported::Refused(why))) => {
                (long(texts.t(words::why_key(why))), Errand::Nothing)
            }
            Done::Exported(Err(Unexported::NotWritten)) => (
                long(texts.t(key::VIDEO_METRICS_EXPORT_FAILED)),
                Errand::Nothing,
            ),
        }
    }

    /// The rows to draw, at this width, from the readings as they are.
    #[must_use]
    pub fn rows(&self, r: &Readings, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let (left, room) = extent(columns);
        self.width.set(room);
        self.left.set(left);
        if let Some((_, area)) = &self.editing {
            return text_rows(area, left, room, texts);
        }
        if let Some(h) = &self.picking {
            return self.picker_rows(h, columns, texts);
        }
        let mut items: Vec<Item> = Vec::new();
        match self.page() {
            Page::Home => home(&mut items, r, texts),
            Page::Channels => channels(&mut items, r, texts),
            Page::Channel(id) => channel(&mut items, id, r, texts),
            Page::Import => import(&mut items, self.imported.as_ref(), r, texts),
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
            return self.room_for_the_title(rows);
        }
        let near = rows.get(self.nearest(&rows)).map(|x| {
            x.cells
                .first()
                .map_or_else(|| x.spot.clone(), |c| c.spot.clone())
        });
        self.room_for_the_title(match near {
            Some(spot) => lay_out(items, left, room, &spot),
            None => rows,
        })
    }

    /// THE ROOM THE LIST KEEPS UNDER ITS HELD TITLE: rows of plain air after
    /// the page's own, until a page of the door's lines ([`View::page_is`])
    /// — or of the most its page can hold, when it says more
    /// ([`View::page_holds`]) — starts at the title.
    ///
    /// # Why the list reserves it
    ///
    /// Every door puts a row first only as far as the list's end lets it — a
    /// page may not scroll past its last row — so a short page could not
    /// bring its title to the top: "By score 2 of 2" over two videos stayed
    /// where it was, the filters above it in sight (the owner, 2026-11-11).
    /// The air makes the end far enough, in both doors alike, and no door
    /// scrolls past what the list holds. It is the air between sections —
    /// nothing a click takes or the pointer lights, no stop of the cursor
    /// and no section's title ([`Row::takes`], `section_ring`) — and only
    /// while a turned page's title holds the top: a list never turned is
    /// drawn as it was.
    fn room_for_the_title(&self, mut rows: Vec<Row>) -> Vec<Row> {
        if !self.title_held || *self.page() != Page::Videos {
            return rows;
        }
        let Some(at) = rows
            .iter()
            .position(|r| r.spot == crate::paging::LIST_TITLE)
        else {
            return rows;
        };
        let lines = self.page_lines.get().max(self.page_most.get());
        for n in 0..(at + lines).saturating_sub(rows.len()) {
            rows.push(plain(crate::chrome::AIR, true, format!("air#room-{n}")));
        }
        rows
    }

    /// THE LIST'S TITLE A TURN PUT FIRST STAYS THERE — its line among `rows`,
    /// for the door to put first again after the follow that ends a gesture
    /// — when it was the first line drawn as the gesture began (`was`, the
    /// door's first row then) and the cursor's whole row is still on a page
    /// of `lines` lines that starts at it.
    ///
    /// # Why the follow does not have it
    ///
    /// The follow keeps two rows of margin over the cursor, and the cursor
    /// starts on the first video, right under the title: the next key that
    /// left it there — `]` on the last page, a quick mark — and a click that
    /// only chose that video, scrolled the title back down a line, and the
    /// list jumped under the person's eyes (the owner, 2026-11-11: the list
    /// stays aligned on its title). What is above the title is not the
    /// list's context. A cursor that goes above it, or below the page, is
    /// followed as always; and a page scrolled away from the title stays
    /// where it was put.
    #[must_use]
    pub fn title_kept(&self, rows: &[Row], was: usize, lines: usize) -> Option<usize> {
        if !self.title_held || *self.page() != Page::Videos {
            return None;
        }
        let at = rows
            .iter()
            .position(|r| r.spot == crate::paging::LIST_TITLE)?;
        let cursor = self.drawn_cursor(rows);
        let end = cursor + Row::lines_of(rows, cursor);
        (was == at && cursor >= at && end <= at + lines).then_some(at)
    }

    /// THE FILE PICKER'S ROWS, as the view's own while it is up: under its
    /// title, each a row a click takes but a title, a sentence and the air —
    /// named for the foot by its words (the folder's, the file's, the
    /// label of a row on top), and its cursor's line kept where the doors ask
    /// it ([`View::cursor`]).
    fn picker_rows(
        &self,
        h: &crate::file_picker::Hosted<Picking>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let mut focused = 0;
        let rows: Vec<Row> = h
            .rows(columns, texts, |p| p)
            .into_iter()
            .enumerate()
            .map(|(n, p)| {
                if p.focused {
                    focused = n;
                }
                let takes = !p.heading && p.target != crate::file_picker::Target::Nothing;
                // ITS NAME: the first of its words that is more than a glyph
                // — a file's name after its glyph, a row's label.
                let name = p
                    .fields
                    .iter()
                    .map(|f| f.text.trim())
                    .find(|t| t.chars().count() > 1)
                    .or_else(|| {
                        p.fields
                            .iter()
                            .map(|f| f.text.trim())
                            .find(|t| !t.is_empty())
                    })
                    .unwrap_or_default()
                    .to_owned();
                Row {
                    fields: p.fields,
                    heading: !takes,
                    target: if takes {
                        Target::Pick(p.target)
                    } else {
                        Target::Nothing
                    },
                    spot: p.spot,
                    continues: p.continues && takes,
                    cells: Vec::new(),
                    controls: Vec::new(),
                    thumbs: Vec::new(),
                    caret: p.caret,
                    about: None,
                    name,
                    default: None,
                    ground: None,
                    words_at: None,
                    wholes: Vec::new(),
                }
            })
            .collect();
        self.picked_at.set(focused);
        rows
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
                "filter-set-aside",
                texts.t(key::VIDEO_METRICS_FILTER_SET_ASIDE),
                texts.t(words::set_aside_key(self.filter.set_aside)),
                Target::FilterSetAside,
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
                texts.t(self.layout.key()),
                Target::Layout,
            ),
        ]);
        // THE FILTER ASKED, and no other: a reading of the filter before is
        // not this list, and says "reading" until this one arrives. ANOTHER
        // PAGE OF IT, or another size, is: the page still drawn stays drawn
        // while the next one is read (the ranking's rule), and the Page row,
        // the title's mark and its arrows say the page ASKED — so nothing
        // blinks to "reading" between two pages, and the arrows never jump
        // when the reading lands. The layout is not asked about — the rows
        // are the same.
        let listed = r.listed.as_ref().filter(|l| l.query.filter == self.filter);
        let Some(listed) = listed else {
            refused_or_reading(items, r, texts);
            return;
        };
        let pages = crate::paging::pages_of(listed.total, self.per_page.get());
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
        // HOW MANY A PAGE HOLDS, a ring of the session's, under the page.
        items.push(setting(
            "per-page",
            texts.t(key::VIDEO_METRICS_PER_PAGE),
            self.per_page.get().to_string(),
            Target::PerPage,
        ));
        // THE LIST'S TITLE SAYS ITS PAGE, quietly — "By score 1 of 2" — with
        // the arrows at its edge (the owner, 2026-11-10); a list of one page
        // says neither.
        let arrows = crate::paging::arrows(self.page, pages);
        items.push(Item::Titled {
            text: texts.t(match self.filter.order {
                Order::Score => key::VIDEO_METRICS_BY_SCORE,
                Order::NewestSetAside => key::VIDEO_METRICS_BY_SET_ASIDE,
            }),
            mark: crate::paging::mark(self.page, pages, texts),
            controls: page_arrows(arrows, texts),
        });
        if listed.rows.is_empty() {
            items.push(Item::Note(
                texts.t(key::VIDEO_METRICS_NO_VIDEOS).into(),
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
                    let mut tile = tile_of(v, r, texts);
                    tile.file = None;
                    items.push(Item::Entry(tile));
                }
            }
            // THE SAME ENTRY, ITS PICTURE AT ITS HEAD — where the door holds a
            // picture beside a title as wide again; narrower, the list's own
            // ([`lay_out`]).
            Layout::Strip => {
                for v in &listed.rows {
                    items.push(Item::Strip(tile_of(v, r, texts)));
                }
            }
            // WIDE TILES WHERE THE DOOR HOLDS ONE; narrower, the grid with
            // the channel, whose tile is a thumbnail's — as the list with
            // thumbnails is the list where a picture does not fit.
            Layout::Grid | Layout::GridWithChannel | Layout::Wide => {
                let layout = if self.layout == Layout::Wide && room < tile_cells(Layout::Wide) {
                    Layout::GridWithChannel
                } else {
                    self.layout
                };
                let grid = grid_of(room, layout);
                for shelf in listed.rows.chunks(usize::from(grid.across.max(1))) {
                    items.push(Item::Shelf(
                        shelf.iter().map(|v| tile_of(v, r, texts)).collect(),
                        grid,
                        layout,
                    ));
                }
            }
        }
        // AND THE ARROWS AGAIN AT THE LIST'S END, where a person who read it
        // to the bottom is — with the mark, in no title's ink: no section
        // begins there (`section_ring`).
        if let (Some(mark), Some(_)) = (crate::paging::mark(self.page, pages, texts), arrows) {
            items.push(Item::Pager {
                mark,
                controls: page_arrows(arrows, texts),
            });
        }
    }
}

/// THE TWO ARROWS OF A LIST'S PAGES, `[earlier, later]` — each a glyph
/// acting at once, named and explained as the page keys are; `None` for one
/// that cannot move, whose place stays empty ([`crate::paging::arrows`]).
fn page_arrows(arrows: Option<[bool; 2]>, texts: &ronoctua_i18n::Texts) -> [Option<Control>; 2] {
    let Some([earlier, later]) = arrows else {
        return [None, None];
    };
    let arrow = |on: bool, glyph_of: char, name: &'static str| {
        let target = Target::PageTurn(on);
        let about = about_of(&target).unwrap_or(key::ABOUT_VIDEO_METRICS_PAGE);
        glyph(glyph_of, target, texts.t(name), about, false)
    };
    [
        earlier.then(|| {
            arrow(
                false,
                tokens::icon::LIST_PAGE_EARLIER,
                key::VIDEO_METRICS_PAGE_BACK,
            )
        }),
        later.then(|| {
            arrow(
                true,
                tokens::icon::LIST_PAGE_LATER,
                key::VIDEO_METRICS_PAGE_ON,
            )
        }),
    ]
}

/// What the door says after a [`Done`], and for how long — with the names
/// it quotes told (`chrome::Named`): a title, a channel, a file is drawn in
/// a name's ink on the window's foot and on the footer's line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// A sentence.
    Normal(crate::chrome::Named),
    /// A refusal, read to the end.
    Long(crate::chrome::Named),
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

/// THE DAYS A KEEP'S RING OFFERS, before "never deleted": a week to a year,
/// the ninety the catalogue starts with among them (`Keep::DEFAULT`).
const RING_DAYS: [u32; 7] = [7, 14, 30, 60, 90, 180, 365];

/// A KEEP'S RING: the days every ring offers, then "never deleted" — and
/// `now` among the days, in its order, when it is a number typed of its own:
/// a turn from it goes to its neighbour, where `ring_step` would start again
/// from the first for a value the ring does not hold.
fn keep_ring(now: Option<Keep>) -> Vec<Keep> {
    let mut days = RING_DAYS.to_vec();
    if let Some(Keep::Days(n)) = now
        && !days.contains(&n)
    {
        days.push(n);
        days.sort_unstable();
    }
    days.into_iter()
        .map(Keep::Days)
        .chain(std::iter::once(Keep::Forever))
        .collect()
}

/// What a keep's question starts with: its days — nothing for "never
/// deleted", which is no number.
fn days_of(keep: Option<Keep>) -> Option<String> {
    match keep {
        Some(Keep::Days(n)) => Some(n.to_string()),
        Some(Keep::Forever) | None => None,
    }
}

/// WHAT A KEY FOR A CHANNEL DOES — a browser's, the only ones a channel
/// takes: its videos' page on YouTube, in Chrome as a kiosk or in Nyxt. Any
/// other key does nothing to a channel.
fn on_a_channel(press: Press, id: &ChannelId, r: &Readings) -> Errand {
    let Some(url) = videos_address(id, r) else {
        return Errand::Nothing;
    };
    match press {
        Press::Kiosk => Errand::Kiosk(url),
        Press::Nyxt => Errand::Nyxt(url),
        _ => Errand::Nothing,
    }
}

/// A CHANNEL'S VIDEOS ON YOUTUBE — its `/videos` tab, newest first — where
/// a browser opens it: by its id, the address the scanner lists
/// (`youtube::tab_address`); by the address it was added with only for an
/// id YouTube would not take, which the catalogue keeps none of.
fn videos_address(id: &ChannelId, r: &Readings) -> Option<String> {
    use ronoctua_ytdlp::youtube::{Tab, tab_address};
    tab_address(&id.0, Tab::Videos).ok().or_else(|| {
        r.channel(id)
            .map(|c| format!("{}/{}", c.url.trim_end_matches('/'), Tab::Videos.path()))
    })
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
            | Target::PerPage
            | Target::ChannelKeep(..)
            | Target::KeepEverywhere(_)
            | Target::FilterSetAside
            | Target::SetAside(..)
            | Target::VideoKeep(..)
            | Target::Staged(..)
            | Target::Decide(..)
            | Target::DecideAll(_)
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
        Target::KeepEverywhere(_) | Target::ChannelKeep(..) => Ask::KeepDays,
        _ => return None,
    })
}

/// The sentence a row says under the pointer: its own when it was given one
/// where it was built — a fact, a rule of a kind, a switch with more to say
/// — else its target's ([`about_of`]).
fn sentence_of(row: &Row) -> Option<&'static str> {
    row.about.or_else(|| about_of(&row.target))
}

/// WHAT A ROW IS, BY WHAT IT DOES — the sentence the pointer resting on it
/// reads, and the door's foot under its name (the owner, 2026-11-09: "hover
/// em toda opção").
///
/// EXHAUSTIVE, AND NO ARM FOR THE REST: a new [`Target`] does not compile
/// until it says what it is, so no row of tomorrow is born mute. The two
/// `None`s are rows that say theirs where they are built — `Nothing` is a
/// fact with no value, a scan's figure, a reason videos were left out; a
/// band's or words' page is opened by a row that knows its kind
/// ([`about_of_kind`]) — and the home page, which no row opens (the way
/// back does).
const fn about_of(target: &Target) -> Option<&'static str> {
    Some(match target {
        Target::Nothing => return None,
        Target::Copy(_) => key::ABOUT_VIDEO_METRICS_COPY,
        Target::Go(page) => return about_of_page(page),
        Target::Scan => key::CMD_VIDEO_METRICS_SCAN,
        Target::Consent(_) => key::ABOUT_VIDEO_METRICS_ASK_YOUTUBE,
        Target::AddChannel => key::CMD_VIDEO_METRICS_ADD_CHANNEL,
        Target::ChannelScore(..) => key::ABOUT_VIDEO_METRICS_SCORE,
        Target::ChannelShorts(..) => key::ABOUT_VIDEO_METRICS_CHANNEL_SHORTS,
        Target::ChannelKeep(..) => key::ABOUT_VIDEO_METRICS_CHANNEL_KEEP,
        Target::ChannelVideos(_) => key::ABOUT_VIDEO_METRICS_ITS_VIDEOS,
        Target::RemoveChannel(..) => key::ABOUT_VIDEO_METRICS_REMOVE_CHANNEL,
        Target::AddBand(_) => key::ABOUT_VIDEO_METRICS_ADD_BAND,
        Target::AddTerm(_) => key::ABOUT_VIDEO_METRICS_ADD_TERM,
        Target::BlockShorts(_) => key::ABOUT_VIDEO_METRICS_BLOCK_SHORTS,
        Target::KeepEverywhere(_) => key::ABOUT_VIDEO_METRICS_KEEP_EVERYWHERE,
        Target::Points(_) | Target::Weight(_) => key::ABOUT_VIDEO_METRICS_POINTS,
        Target::Days(_) => key::ABOUT_VIDEO_METRICS_RECENCY_DAYS,
        Target::Band(_) => key::ABOUT_VIDEO_METRICS_RULE_LENGTH,
        Target::Words(_) => key::ABOUT_VIDEO_METRICS_TERM_WORDS,
        Target::Match(_) => key::ABOUT_VIDEO_METRICS_TERM_MATCH,
        Target::Field(..) => key::ABOUT_VIDEO_METRICS_TERM_FIELD,
        Target::On(_) => key::ABOUT_VIDEO_METRICS_RULE_ON,
        Target::RemoveRule(_) => key::ABOUT_VIDEO_METRICS_REMOVE_RULE,
        Target::FilterChannel => key::ABOUT_VIDEO_METRICS_FILTER_CHANNEL,
        Target::FilterShorts => key::ABOUT_VIDEO_METRICS_FILTER_SHORTS,
        Target::FilterRating => key::ABOUT_VIDEO_METRICS_FILTER_RATING,
        Target::FilterHidden => key::ABOUT_VIDEO_METRICS_FILTER_HIDDEN,
        Target::FilterSetAside => key::ABOUT_VIDEO_METRICS_FILTER_SET_ASIDE,
        Target::FilterText => key::CMD_VIDEO_METRICS_SEARCH,
        Target::FilterSince => key::ABOUT_VIDEO_METRICS_FILTER_SINCE,
        // THE CHOICE, as every ring's row says its own — not the command's
        // "the next way": the row turns both ways (`←` the one before), and
        // only `L` goes on to the next. `layouts`, never `about-<id>` of the
        // command's id, which a hint line would never read beside its `cmd-`
        // (every-hint-key-explains-itself).
        Target::Layout => key::ABOUT_VIDEO_METRICS_LAYOUTS,
        Target::PageOf => key::ABOUT_VIDEO_METRICS_PAGE,
        Target::PerPage => key::ABOUT_VIDEO_METRICS_PER_PAGE,
        Target::PageTurn(false) => key::CMD_VIDEO_METRICS_PAGE_BACK,
        Target::PageTurn(true) => key::CMD_VIDEO_METRICS_PAGE_ON,
        Target::SetAsideVideos => key::ABOUT_VIDEO_METRICS_SET_ASIDE_VIDEOS,
        Target::Video(_) => key::ABOUT_VIDEO_METRICS_VIDEO_ENTRY,
        Target::Rating(..) => key::ABOUT_VIDEO_METRICS_RATING,
        // AN ARROW OF THE RATING, A BOOKMARK, AN ∞ — what the glyph says of
        // the video now; a row of the video's page says its own (`Pair::about`).
        Target::Rate(_, _, false) => key::ABOUT_VIDEO_METRICS_GLYPH_RATE,
        Target::Rate(_, _, true) => key::ABOUT_VIDEO_METRICS_GLYPH_RATED,
        Target::SetAside(_, false) => key::ABOUT_VIDEO_METRICS_GLYPH_SET_ASIDE,
        Target::SetAside(_, true) => key::ABOUT_VIDEO_METRICS_GLYPH_TAKE_OUT,
        Target::VideoKeep(_, false) => key::ABOUT_VIDEO_METRICS_GLYPH_FOREVER_NO,
        Target::VideoKeep(_, true) => key::ABOUT_VIDEO_METRICS_GLYPH_FOREVER_VIDEO,
        Target::SecondTags(..) => key::ABOUT_VIDEO_METRICS_SECOND_TAGS,
        Target::SecondDescription(..) => key::ABOUT_VIDEO_METRICS_SECOND_DESCRIPTION,
        Target::Player(_) => key::ABOUT_VIDEO_METRICS_PLAYER_ROW,
        Target::Kiosk(_) => key::ABOUT_VIDEO_METRICS_KIOSK_ROW,
        Target::Nyxt(_) => key::ABOUT_VIDEO_METRICS_NYXT_ROW,
        Target::ChannelKiosk(_) => key::ABOUT_VIDEO_METRICS_CHANNEL_IN_CHROME,
        Target::ChannelNyxt(_) => key::ABOUT_VIDEO_METRICS_CHANNEL_IN_NYXT,
        Target::Staged(..) => key::ABOUT_VIDEO_METRICS_STAGED,
        Target::Decide(..) => key::ABOUT_VIDEO_METRICS_DECISION,
        Target::DecideAll(_) => key::ABOUT_VIDEO_METRICS_DECIDE_ALL,
        Target::Takeout => key::ABOUT_VIDEO_METRICS_TAKEOUT,
        Target::PickImport => key::ABOUT_VIDEO_METRICS_CHOOSE_LIST,
        Target::PickExport => key::ABOUT_VIDEO_METRICS_SAVE_LIST,
        Target::Register => key::ABOUT_VIDEO_METRICS_REGISTER,
        // ONE SENTENCE FOR EVERY ROW OF THE PICKER: what it is over the page.
        // The row's own name says which folder, file or answer it is.
        Target::Pick(_) => key::ABOUT_VIDEO_METRICS_PICK,
        Target::Brain => key::ABOUT_VIDEO_METRICS_IN_THE_BRAIN,
        Target::InThePlayer(InThePlayer::Ranking) => key::ABOUT_VIDEO_METRICS_IN_THE_PLAYER,
        Target::InThePlayer(InThePlayer::SetAside) => {
            key::ABOUT_VIDEO_METRICS_SET_ASIDE_IN_THE_PLAYER
        }
        Target::Download(_) => key::ABOUT_VIDEO_METRICS_DOWNLOAD_ROW,
        Target::Hide(..) => key::ABOUT_VIDEO_METRICS_HIDE_ROW,
        Target::Delete(_) => key::ABOUT_VIDEO_METRICS_DELETE_ROW,
        Target::Answer(false) => key::ABOUT_VIDEO_METRICS_ANSWER_NO,
        Target::Answer(true) => key::ABOUT_VIDEO_METRICS_ANSWER_YES,
        Target::Text(_) => key::ABOUT_VIDEO_METRICS_TEXT_LINE,
    })
}

/// What a page is, said on the row that opens it ([`about_of`]).
const fn about_of_page(page: &Page) -> Option<&'static str> {
    Some(match page {
        Page::Home | Page::Rule(_, Slot::Id(_)) => return None,
        Page::Channels => key::ABOUT_VIDEO_METRICS_CHANNELS,
        Page::Channel(_) => key::ABOUT_VIDEO_METRICS_CHANNEL_ROW,
        Page::Import => key::ABOUT_VIDEO_METRICS_IMPORT_EXPORT,
        Page::Rules => key::ABOUT_VIDEO_METRICS_RULES,
        Page::Rule(_, Slot::Single(single)) => match single {
            Single::Recency => key::ABOUT_VIDEO_METRICS_RULE_RECENCY,
            Single::Short => key::ABOUT_VIDEO_METRICS_RULE_SHORT,
            Single::Preferred => key::ABOUT_VIDEO_METRICS_RULE_PREFERRED,
            Single::Live => key::ABOUT_VIDEO_METRICS_RULE_LIVE,
        },
        Page::Weights => key::ABOUT_VIDEO_METRICS_WEIGHTS,
        Page::Videos => key::ABOUT_VIDEO_METRICS_VIDEOS,
        Page::Video(_) => key::ABOUT_VIDEO_METRICS_VIDEO_ENTRY,
    })
}

/// What a rule of `kind` does — the row that opens its page says it.
const fn about_of_kind(kind: &RuleKind) -> &'static str {
    match kind {
        RuleKind::Recency { .. } => key::ABOUT_VIDEO_METRICS_RULE_RECENCY,
        RuleKind::Short => key::ABOUT_VIDEO_METRICS_RULE_SHORT,
        RuleKind::Preferred { .. } => key::ABOUT_VIDEO_METRICS_RULE_PREFERRED,
        RuleKind::Live => key::ABOUT_VIDEO_METRICS_RULE_LIVE,
        RuleKind::Band { .. } => key::ABOUT_VIDEO_METRICS_RULE_BAND,
        RuleKind::Term { .. } => key::ABOUT_VIDEO_METRICS_RULE_TERM,
    }
}

/// WHAT `RET` — OR THE CLICK THAT IS ITS TWIN — DOES on a row whose target
/// is `target`, in the words a door's gesture closes with
/// (`door::foot::gesture`): read off [`View::enter`], arm for arm, so the
/// foot promises what the key does. Exhaustive for the reason [`about_of`]
/// is. `None` for a row that does nothing but be selected — a fact with no
/// value, a line of the text being edited, where a click places the caret
/// and its sentence says so.
const fn does_of(target: &Target) -> Option<&'static str> {
    Some(match target {
        Target::Nothing | Target::Text(_) => return None,
        Target::Copy(_) => key::VIDEO_METRICS_ACT_COPIES,
        Target::Go(_)
        | Target::Video(_)
        | Target::ChannelVideos(_)
        | Target::SetAsideVideos
        | Target::Brain
        | Target::InThePlayer(_)
        | Target::Staged(..)
        | Target::Takeout
        | Target::PickImport
        | Target::PickExport => key::VIDEO_METRICS_ACT_OPENS,
        Target::Scan
        | Target::Player(_)
        | Target::Kiosk(_)
        | Target::Nyxt(_)
        | Target::ChannelKiosk(_)
        | Target::ChannelNyxt(_)
        | Target::Download(_)
        | Target::PageTurn(_) => key::VIDEO_METRICS_ACT_DOES,
        // THE PICKER'S `RET`, by what it is on: a place or a folder opens, the
        // ring turns, its question is answered, anything else is taken — a
        // file, the folder that is open, what was typed, the name to save
        // under.
        Target::Pick(pick) => {
            use crate::file_picker::Target as P;
            match pick {
                P::Nothing => return None,
                P::Place(_) | P::Folder(_) | P::Up => key::VIDEO_METRICS_ACT_OPENS,
                P::Filter => key::VIDEO_METRICS_ACT_TURNS,
                P::Answer(_) => key::VIDEO_METRICS_ACT_ANSWERS,
                P::Name | P::Typed | P::Extra(_) | P::Here | P::Take | P::File(_) => {
                    key::VIDEO_METRICS_ACT_TAKES
                }
            }
        }
        Target::AddChannel
        | Target::ChannelScore(..)
        | Target::Weight(_)
        | Target::Points(_)
        | Target::Days(_)
        | Target::Band(_)
        | Target::Words(_)
        | Target::AddBand(_)
        | Target::AddTerm(_)
        | Target::FilterText
        | Target::SecondTags(..)
        | Target::KeepEverywhere(_)
        | Target::ChannelKeep(..) => key::VIDEO_METRICS_ACT_TYPES,
        Target::SecondDescription(..) => key::VIDEO_METRICS_ACT_EDITS,
        // A SWITCH, two ways — and a hidden video is shown again at once.
        Target::Consent(_)
        | Target::BlockShorts(_)
        | Target::Field(..)
        | Target::On(_)
        | Target::SetAside(..)
        | Target::VideoKeep(..)
        | Target::Hide(_, true) => key::VIDEO_METRICS_ACT_SWITCHES,
        // A RING — the layout's four among them.
        Target::ChannelShorts(..)
        | Target::Match(_)
        | Target::Rating(..)
        | Target::FilterChannel
        | Target::FilterShorts
        | Target::FilterRating
        | Target::FilterHidden
        | Target::FilterSetAside
        | Target::FilterSince
        | Target::Layout
        | Target::PageOf
        | Target::PerPage
        | Target::Decide(..)
        | Target::DecideAll(Some(_)) => key::VIDEO_METRICS_ACT_TURNS,
        Target::Rate(_, _, false) => key::VIDEO_METRICS_ACT_PICKS,
        Target::Rate(_, _, true) => key::VIDEO_METRICS_ACT_UNPICKS,
        Target::RemoveChannel(..)
        | Target::RemoveRule(_)
        | Target::Hide(_, false)
        | Target::Delete(_)
        | Target::Register
        // ALL OF THEM WHILE THEIRS DIFFER: what each was given would go.
        | Target::DecideAll(None) => key::VIDEO_METRICS_ACT_ASKS,
        Target::Answer(_) => key::VIDEO_METRICS_ACT_ANSWERS,
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

/// WHEN A VIDEO GOES, OR WHY IT DOES NOT — decided in the catalogue's order
/// (`edit::EXPIRED`): its own mark, then its channel's "for ever"; then
/// being set aside, which keeps it only while it lasts and so lights no ∞
/// (the owner, 2026-11-09); then the days in force, its channel's own or the
/// ones everywhere, counted from when it was published — or first seen, for
/// a video with no date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lasting {
    /// Its time is up then, seconds since the epoch — `None` when that
    /// cannot be told yet: what it follows was not read, or it has no date.
    Goes(Option<i64>),
    /// It stays while it is set aside.
    SetAside,
    /// Its own mark keeps it for ever.
    ByVideo,
    /// Its channel keeps every video for ever.
    ByChannel,
    /// Everywhere keeps videos for ever, and its channel follows everywhere
    /// — a channel with days of its own still deletes, so this never says
    /// that no video goes.
    Everywhere,
}

impl Lasting {
    /// Whether the ∞ beside the video is lit: only when it is never taken
    /// out, by its own mark or its channel's — "for ever" everywhere marks
    /// every video and so none, and set aside lasts only as long as it is.
    #[must_use]
    pub const fn forever(self) -> bool {
        matches!(self, Self::ByVideo | Self::ByChannel)
    }

    /// The sentence of the ∞ beside a video in this state.
    const fn glyph_about(self) -> &'static str {
        match self {
            Self::Goes(_) => key::ABOUT_VIDEO_METRICS_GLYPH_FOREVER_NO,
            Self::SetAside => key::ABOUT_VIDEO_METRICS_GLYPH_FOREVER_ASIDE,
            Self::ByVideo => key::ABOUT_VIDEO_METRICS_GLYPH_FOREVER_VIDEO,
            Self::ByChannel => key::ABOUT_VIDEO_METRICS_GLYPH_FOREVER_CHANNEL,
            Self::Everywhere => key::ABOUT_VIDEO_METRICS_GLYPH_FOREVER_EVERYWHERE,
        }
    }

    /// Its words, as what a video follows without a mark of its own — its
    /// "never delete", its being set aside: "goes on 9 Feb 2027", "its
    /// channel keeps every video"…
    fn words(self, r: &Readings, texts: &ronoctua_i18n::Texts) -> String {
        match self {
            Self::Goes(Some(at)) => {
                let order = crate::when::prefs::current().order;
                texts.t_with(
                    key::VIDEO_METRICS_GOES_ON,
                    &[("date", &crate::chrome::date_text(at, &r.zone, order, texts))],
                )
            }
            Self::Goes(None) => texts.t(key::VIDEO_METRICS_GOES_UNKNOWN),
            Self::SetAside => texts.t(key::VIDEO_METRICS_KEPT_SET_ASIDE),
            Self::ByVideo => texts.t(key::VIDEO_METRICS_KEPT_FOR_EVER),
            Self::ByChannel => texts.t(key::VIDEO_METRICS_KEPT_BY_CHANNEL),
            Self::Everywhere => texts.t(key::VIDEO_METRICS_KEPT_EVERYWHERE),
        }
    }
}

/// [`Lasting`] for video `v`, first seen at `first_seen_s` when that is
/// known (a video of the list does not say), from what `r` read: its
/// channel, and the switches. NEVER FROM A DEFAULT NOBODY READ: a channel
/// not read, or one "as everywhere" with the switches unread, goes at a
/// moment that cannot be told.
#[must_use]
pub fn lasting(v: &RankedVideo, first_seen_s: Option<i64>, r: &Readings) -> Lasting {
    match v.forever {
        Forever::ByVideo => return Lasting::ByVideo,
        Forever::ByChannel => return Lasting::ByChannel,
        Forever::No => {}
    }
    let own = r.channel(&v.channel_id).map(|c| c.keep);
    // ITS CHANNEL'S "FOR EVER", read after the video was: the row says what
    // the channel says now.
    if own == Some(Some(Keep::Forever)) {
        return Lasting::ByChannel;
    }
    if v.set_aside_at_s.is_some() {
        return Lasting::SetAside;
    }
    let days = match own {
        Some(Some(Keep::Forever)) => return Lasting::ByChannel,
        Some(Some(Keep::Days(n))) => n,
        Some(None) => match r.settings.map(|s| s.keep) {
            Some(Keep::Forever) => return Lasting::Everywhere,
            Some(Keep::Days(n)) => n,
            None => return Lasting::Goes(None),
        },
        None => return Lasting::Goes(None),
    };
    Lasting::Goes(
        v.published_at_s
            .or(first_seen_s)
            .map(|from| from.saturating_add(i64::from(days) * 86_400)),
    )
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
    /// GLYPHS THAT ACT ON THE FIRST CLICK, not placed yet — a channel's ways
    /// out to its videos on YouTube: at the right edge of the row's first
    /// line, its words ending a cell of air before them ([`lay_out`]).
    controls: Vec<Control>,
    /// What it follows, set "as everywhere": drawn after its value, quiet
    /// whatever the value's tone ([`Follows`]).
    default: Option<Follows>,
}

/// A video of the list, or of the grid.
#[derive(Debug, Clone)]
struct Tile {
    spot: String,
    title: String,
    /// Its channel's title — data: under the title on a tile when the grid
    /// says it ([`Layout::GridWithChannel`]).
    channel: String,
    /// Its channel, its age and its score, as the list says them before the
    /// arrows of its rating — and its age and score alone, for a line too
    /// short for the channel ([`entry_line`]).
    said: [String; 2],
    /// WHERE THE CHANNEL STANDS in the first of those, `(cells before it,
    /// its cells)` — as the language put it there, read off the sentence
    /// (`Texts::t_args_marked`, the name's own range), never by finding the
    /// channel's words in it: the pointer resting there names it whole.
    channel_in_said: Option<(u16, u16)>,
    /// How long ago it was published; empty when that is not known.
    age: String,
    /// What it scores, a bare number (`words::score_number`): the arrows of
    /// its rating follow it, and "points" on every line was in the way.
    score: String,
    /// The four arrows of its rating, not placed yet.
    ratings: Vec<Control>,
    /// Its marks — set aside, hidden, deleted, never deleted — not placed
    /// yet: at the right edge of their line.
    marks: Vec<Control>,
    file: Option<PathBuf>,
    target: Target,
    hidden: bool,
}

/// A video of the list as the list and the grid say it: its words, and its
/// glyphs ([`ratings_of`], [`marks_of`]).
fn tile_of(v: &RankedVideo, r: &Readings, texts: &ronoctua_i18n::Texts) -> Tile {
    let age = age_of(v, r, texts);
    let score = words::score_number(v.score, texts);
    // WITH NO AGE, the channel and the score — and the score alone. THE
    // CHANNEL TRAVELS AS A NAME, so the sentence says where it landed.
    let channel = Arg::Name(&v.channel);
    let (with_channel, short) = if age.is_empty() {
        (
            texts.t_args_marked(
                key::VIDEO_METRICS_VIDEO_ROW_UNDATED,
                &[("channel", channel), ("score", Arg::Text(&score))],
            ),
            score.clone(),
        )
    } else {
        (
            texts.t_args_marked(
                key::VIDEO_METRICS_VIDEO_ROW,
                &[
                    ("channel", channel),
                    ("age", Arg::Text(&age)),
                    ("score", Arg::Text(&score)),
                ],
            ),
            texts.t_with(
                key::VIDEO_METRICS_VIDEO_ROW_SHORT,
                &[("age", &age), ("score", &score)],
            ),
        )
    };
    let channel_in_said = with_channel.names.first().and_then(|at| {
        let width = ronoctua_render::layout::width;
        let before = with_channel.text.get(..at.start)?;
        Some((width(before), width(with_channel.text.get(at.clone())?)))
    });
    Tile {
        spot: video_spot(&v.id),
        title: v.title.clone(),
        channel: v.channel.clone(),
        said: [with_channel.text, short],
        channel_in_said,
        age,
        score,
        ratings: ratings_of(v, texts),
        marks: marks_of(v, r, texts),
        file: v.thumb_file.clone(),
        target: Target::Video(v.id.clone()),
        hidden: v.hidden,
    }
}

/// A glyph that acts on the first click, not placed yet: as wide as its
/// glyph is, and named and explained as the foot says it.
fn glyph(glyph: char, target: Target, name: String, about: &'static str, lit: bool) -> Control {
    Control {
        from: 0,
        width: ronoctua_render::layout::width(&glyph.to_string()),
        glyph,
        target,
        name,
        about,
        follows: None,
        lit,
    }
}

/// THE FOUR ARROWS OF A VIDEO'S RATING, from very negative to very positive
/// (the owner, 2026-11-09; arrows since 2026-11-10) — the one it has lit, and a click on it takes the
/// rating away; on another, it rates the video so. The ranking's arrows
/// (`player_ranking`), in the catalogue's words.
fn ratings_of(v: &RankedVideo, texts: &ronoctua_i18n::Texts) -> Vec<Control> {
    Rating::ALL
        .into_iter()
        .map(|x| {
            let lit = v.rating == Some(x);
            let target = Target::Rate(v.id.clone(), x, lit);
            let about = about_of(&target).unwrap_or(key::ABOUT_VIDEO_METRICS_RATING);
            glyph(
                words::rating_glyph(x),
                target,
                texts.t(words::rating_key(x)),
                about,
                lit,
            )
        })
        .collect()
}

/// A VIDEO'S MARKS, each acting on the first click: the bookmark — filled
/// while it is set aside —, the eye that hides it or shows it again, the
/// bin that deletes it after asking, and the ∞, lit while it is never
/// deleted, by its own mark or its channel's. A set-aside video lights no
/// ∞: being set aside keeps it only while it lasts ([`Lasting`]). Named as
/// the rows of its own page are.
///
/// WHAT HOLDS ONCE A MARK OF ITS OWN IS OFF — the bookmark of a set-aside
/// video, the ∞ of one kept by its own mark — is read off [`lasting`]
/// WITHOUT that mark ([`Control::follows`]): taken out, a video whose
/// channel keeps every video still stays, and so does one no video is
/// deleted under, or one its own mark keeps; a sentence chosen without
/// asking would promise "it goes" of it.
fn marks_of(v: &RankedVideo, r: &Readings, texts: &ronoctua_i18n::Texts) -> Vec<Control> {
    let id = &v.id;
    let aside = v.set_aside_at_s.is_some();
    let own = v.forever == Forever::ByVideo;
    let lasting_now = lasting(v, None, r);
    let aside_target = Target::SetAside(id.clone(), aside);
    let aside_about = about_of(&aside_target).unwrap_or(key::ABOUT_VIDEO_METRICS_SET_ASIDE_ROW);
    let once_out = aside.then(|| {
        let out = RankedVideo {
            set_aside_at_s: None,
            ..v.clone()
        };
        let value = lasting(&out, None, r).words(r, texts);
        texts.t_with(key::VIDEO_METRICS_ONCE_OUT, &[("value", &value)])
    });
    let unmarked = own.then(|| {
        let unmarked = RankedVideo {
            forever: Forever::No,
            ..v.clone()
        };
        let value = lasting(&unmarked, None, r).words(r, texts);
        texts.t_with(key::VIDEO_METRICS_WITHOUT_THE_MARK, &[("value", &value)])
    });
    vec![
        Control {
            follows: once_out,
            ..glyph(
                words::set_aside_glyph(aside),
                aside_target,
                texts.t(key::VIDEO_METRICS_SET_ASIDE_ROW),
                aside_about,
                aside,
            )
        },
        glyph(
            words::hidden_glyph(v.hidden),
            Target::Hide(id.clone(), v.hidden),
            texts.t(key::CMD_VIDEO_METRICS_HIDE),
            key::ABOUT_VIDEO_METRICS_HIDE_ROW,
            v.hidden,
        ),
        glyph(
            tokens::icon::VIDEO_DELETE,
            Target::Delete(id.clone()),
            texts.t(key::CMD_VIDEO_METRICS_DELETE),
            key::ABOUT_VIDEO_METRICS_DELETE_ROW,
            false,
        ),
        Control {
            follows: unmarked,
            ..glyph(
                tokens::icon::VIDEO_FOREVER,
                Target::VideoKeep(id.clone(), own),
                texts.t(key::VIDEO_METRICS_NEVER_DELETE),
                lasting_now.glyph_about(),
                lasting_now.forever(),
            )
        },
    ]
}

/// How many cells a strip of glyphs takes: each glyph's, and a cell of air
/// ([`GLYPH_AIR`]) between two.
fn strip_cells(glyphs: &[Control]) -> u16 {
    let airs = u16::try_from(glyphs.len().saturating_sub(1)).unwrap_or(u16::MAX);
    glyphs
        .iter()
        .map(|c| c.width)
        .fold(0u16, u16::saturating_add)
        .saturating_add(airs.saturating_mul(GLYPH_AIR))
}

/// THE ROOM A ROW'S GLYPHS TAKE FROM ITS WORDS at its right edge: the strip
/// and the cell of air before it — none for a row with none.
fn glyphs_room(glyphs: &[Control]) -> u16 {
    if glyphs.is_empty() {
        0
    } else {
        strip_cells(glyphs).saturating_add(GLYPH_AIR)
    }
}

/// THE GLYPHS OF A STRIP, placed from cell `from` (a field's `at`), a cell
/// of air between two — on `row`, each its field and the box a click takes:
/// lit in the accent while it says the video's state, else in the quietest
/// ink, and never in a label's tone (`labels-say-what-they-are`).
fn put_strip(row: &mut Row, glyphs: &[Control], from: u16) {
    let mut at = from;
    for c in glyphs {
        let colour = if c.lit {
            tokens::color::STATE_ACCENT
        } else {
            tokens::color::TEXT_DIM
        };
        row.fields
            .push(piece(at, c.glyph.to_string(), colour, c.width));
        row.controls.push(Control {
            from: at,
            ..c.clone()
        });
        at = at.saturating_add(c.width).saturating_add(GLYPH_AIR);
    }
}

/// THE ARROWS OF A LIST'S PAGES at a row's right edge `end` — each in a
/// place as wide as the widest of them, a cell of air between two — in
/// `colour`: a `None` keeps its place EMPTY, so the arrow beside it never
/// slides into it (the owner, 2026-11-10: an arrow that cannot move is not
/// drawn). Each one drawn is a field and the box a click takes, as a strip's
/// glyph is ([`put_strip`]).
fn put_slots(row: &mut Row, slots: &[Option<Control>; 2], end: u16, colour: tokens::Color) {
    let place = slots.iter().flatten().map(|c| c.width).max().unwrap_or(0);
    if place == 0 {
        return;
    }
    let cells = place.saturating_mul(2).saturating_add(GLYPH_AIR);
    let mut at = end.saturating_sub(cells);
    for c in slots {
        if let Some(c) = c {
            row.fields
                .push(piece(at, c.glyph.to_string(), colour, c.width));
            row.controls.push(Control {
                from: at,
                ..c.clone()
            });
        }
        at = at.saturating_add(place).saturating_add(GLYPH_AIR);
    }
}

/// The room a row's arrows take from its words at its right edge, the cell
/// of air before them included ([`put_slots`]) — none for a row with none.
fn slots_room(slots: &[Option<Control>; 2]) -> u16 {
    let place = slots.iter().flatten().map(|c| c.width).max().unwrap_or(0);
    if place == 0 {
        return 0;
    }
    place
        .saturating_mul(2)
        .saturating_add(GLYPH_AIR)
        .saturating_add(GLYPH_AIR)
}

/// What a page says, before it is placed.
#[derive(Debug, Clone)]
enum Item {
    /// A section's title (air before it, but for the first).
    Heading(String),
    /// A row.
    Pair(Pair),
    /// A sentence, wrapped and never cut — the names it quotes in the ink a
    /// name in it wears (`chrome::quoted_ink`).
    Note(crate::chrome::Named, tokens::Color),
    /// A video of the list: its title on the whole width, and under it what
    /// it scores, its channel and its age.
    Entry(Tile),
    /// A VIDEO OF THE LIST WITH THUMBNAILS: an [`Item::Entry`] with its
    /// picture at its head ([`strip_entry`]) — the list's own entry where
    /// the door is too narrow to hold the picture beside a title.
    Strip(Tile),
    /// A shelf of the grid, and the layout its tiles are drawn in.
    Shelf(Vec<Tile>, ronoctua_design::grid::Grid, Layout),
    /// A picture on its own, `thumbLines` tall.
    Figure(PathBuf),
    /// A LIST'S TITLE THAT SAYS ITS PAGE ("By score 1 of 2", the owner,
    /// 2026-11-10): a section's title, its mark after it in the quiet ink,
    /// and the arrows `[earlier, later]` at the row's edge — a `None` keeps
    /// its place empty. Its spot is the list's (`paging::LIST_TITLE`), the
    /// row a turned page brings to the top.
    Titled {
        /// The title.
        text: String,
        /// "1 of 2" — `None` for a list of one page.
        mark: Option<String>,
        /// The arrows, `[earlier, later]`.
        controls: [Option<Control>; 2],
    },
    /// THE ARROWS AGAIN AT A LIST'S END, with its mark — a line the cursor
    /// skips, in no title's ink: no section begins there.
    Pager {
        /// "1 of 2".
        mark: String,
        /// The arrows, `[earlier, later]`.
        controls: [Option<Control>; 2],
    },
    /// THE QUESTION ON THE PAGE, a block of its own (`chrome::question_lines`):
    /// what is asked, its answers' words `[no, yes]`, and what the yes costs.
    Question {
        asked: crate::chrome::Named,
        answers: [String; 2],
        stakes: crate::chrome::Stakes,
    },
}

impl Item {
    /// Whether the row — or a cell of the shelf — it becomes has `spot`.
    fn has_spot(&self, spot: &str) -> bool {
        match self {
            Self::Pair(p) => p.spot == spot,
            Self::Entry(t) | Self::Strip(t) => t.spot == spot,
            Self::Shelf(tiles, ..) => tiles.iter().any(|t| t.spot == spot),
            _ => false,
        }
    }

    /// The same row, saying `about` under the pointer — a fact whose label
    /// alone does not say what it is, which its target cannot either.
    fn about(mut self, about: &'static str) -> Self {
        if let Self::Pair(p) = &mut self {
            p.about = Some(about);
        }
        self
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
        controls: Vec::new(),
        default: None,
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
        controls: Vec::new(),
        default: None,
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
            texts.t(words::why_key(why)).into(),
            if why == Why::NotThere {
                tokens::color::TEXT_DIM
            } else {
                tokens::color::STATE_ERROR
            },
        )),
        _ => items.push(Item::Note(
            texts.t(key::VIDEO_METRICS_READING).into(),
            tokens::color::TEXT_DIM,
        )),
    }
}

/// The question on the page, "no" first: ITS OWN BLOCK among the rows —
/// the sentence in the question's colour with what it is about in a name's,
/// the answers set in under it, on a ground of its own with air before and
/// after (owner, 2026-11-10: it had the tone and the inset of the options
/// around it, and it all ran together) — and its "yes" in the colour of what
/// it costs ([`Pending::stakes`]).
fn question(items: &mut Vec<Item>, c: &Confirm, texts: &ronoctua_i18n::Texts) {
    let [no, yes] = c.what.answers();
    items.push(Item::Question {
        asked: c.what.asked(texts),
        answers: [texts.t(no), texts.t(yes)],
        stakes: c.what.stakes(),
    });
}

impl Pending {
    /// WHAT ITS "YES" COSTS — said by the colour the yes is drawn in
    /// (`chrome::answer_tone`): what is deleted or removed for good, and
    /// every channel's decision given in one, is lost; registering what was
    /// decided weighs — it reaches every channel staged, and is not taken
    /// back by a key —; adding again and hiding lose nothing (a hidden video
    /// is shown again from its page).
    #[must_use]
    pub const fn stakes(&self) -> crate::chrome::Stakes {
        use crate::chrome::Stakes;
        match self {
            Self::Delete(..)
            | Self::RemoveChannel(..)
            | Self::RemoveRule(_)
            | Self::DecideAll(..) => Stakes::Lost,
            Self::Register(..) => Stakes::Weighty,
            Self::Again(..) | Self::Hide(..) => Stakes::Light,
        }
    }

    /// What is asked, in words — the video's title, the channel's name,
    /// the name the queue has it under each marked as a NAME the sentence
    /// quotes (`Arg::Name`), drawn in a name's ink; a count stays a number.
    #[must_use]
    pub fn asked(&self, texts: &ronoctua_i18n::Texts) -> crate::chrome::Named {
        match self {
            Self::Delete(_, title) => texts
                .t_args_marked(
                    key::ASK_VIDEO_METRICS_DELETE,
                    &[("title", Arg::Name(title))],
                )
                .into(),
            Self::Hide(_, title) => texts
                .t_args_marked(key::ASK_VIDEO_METRICS_HIDE, &[("title", Arg::Name(title))])
                .into(),
            Self::RemoveChannel(_, name) => texts
                .t_args_marked(
                    key::ASK_VIDEO_METRICS_REMOVE_CHANNEL,
                    &[("name", Arg::Name(name))],
                )
                .into(),
            Self::RemoveRule(_) => texts.t(key::ASK_VIDEO_METRICS_REMOVE_RULE).into(),
            Self::Again(_, name) => texts
                .t_args_marked(key::ASK_VIDEO_METRICS_AGAIN, &[("name", Arg::Name(name))])
                .into(),
            Self::Register(kept, discarded) => texts
                .t_args_marked(
                    key::ASK_VIDEO_METRICS_REGISTER,
                    &[
                        ("kept", Arg::Number(i64::from(*kept))),
                        ("discarded", Arg::Number(i64::from(*discarded))),
                    ],
                )
                .into(),
            Self::DecideAll(decision, changed) => texts
                .t_args_marked(
                    words::ask_decide_all_key(*decision),
                    &[("changed", Arg::Number(i64::from(*changed)))],
                )
                .into(),
        }
    }

    /// Its answers' words, `[no, yes]` — the order they are drawn in.
    const fn answers(&self) -> [&'static str; 2] {
        match self {
            Self::Delete(..) => [key::VIDEO_METRICS_KEEP_VIDEO, key::VIDEO_METRICS_DELETE_IT],
            Self::Hide(..) => [key::VIDEO_METRICS_LEAVE_SHOWN, key::VIDEO_METRICS_HIDE_IT],
            Self::RemoveChannel(..) => [
                key::VIDEO_METRICS_KEEP_CHANNEL,
                key::VIDEO_METRICS_REMOVE_CHANNEL_IT,
            ],
            Self::RemoveRule(_) => [
                key::VIDEO_METRICS_KEEP_RULE,
                key::VIDEO_METRICS_REMOVE_RULE_IT,
            ],
            Self::Again(..) => [key::VIDEO_METRICS_NOT_AGAIN, key::VIDEO_METRICS_AGAIN],
            Self::Register(..) => [key::VIDEO_METRICS_NOT_YET, key::VIDEO_METRICS_REGISTER_THEM],
            Self::DecideAll(..) => [
                key::VIDEO_METRICS_LEAVE_DECISIONS,
                key::VIDEO_METRICS_DECIDE_THEM_ALL,
            ],
        }
    }
}

/// The home page: the scan, the catalogue, what was left out.
fn home(items: &mut Vec<Item>, r: &Readings, texts: &ronoctua_i18n::Texts) {
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_SCAN)));
    if let Some(Err(why)) = r.catalogue
        && why != Why::NotThere
    {
        items.push(Item::Note(
            texts.t(words::why_key(why)).into(),
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
        about: Some(key::ABOUT_VIDEO_METRICS_LAST_SCAN),
        controls: Vec::new(),
        default: None,
    }));
    if let Some(s) = scan.filter(|s| s.ended_at_s.is_some()) {
        let all_failed = s.channels > 0 && s.channels_failed == s.channels;
        items.push(
            fact(
                "scan-added",
                texts.t(key::VIDEO_METRICS_SCAN_ADDED),
                s.added.to_string(),
            )
            .about(key::ABOUT_VIDEO_METRICS_SCAN_ADDED),
        );
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
            about: Some(key::ABOUT_VIDEO_METRICS_SCAN_FAILED),
            controls: Vec::new(),
            default: None,
        }));
    }
    if let Some(stop) = scan.and_then(|s| s.stopped) {
        items.push(Item::Note(
            texts.t(words::stop_key(stop)).into(),
            tokens::color::STATE_WARNING,
        ));
    }
    if let Some(s) = scan.filter(|s| s.pending > 0) {
        items.push(Item::Note(
            texts
                .t_args(
                    key::VIDEO_METRICS_PENDING,
                    &[("pending", Arg::Number(i64::from(s.pending)))],
                )
                .into(),
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
            controls: Vec::new(),
            default: None,
        }));
        // THE HOSTS IT LETS BE ASKED are names, each its own — as the
        // player's "Asking …" tells its hosts (`chrome::names_listed`).
        items.push(Item::Note(
            crate::chrome::names_listed(
                texts,
                key::VIDEO_METRICS_ASK_YOUTUBE_NOTE,
                "hosts",
                &ronoctua_video_metrics::HOSTS,
                ", ",
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
    // WHAT WAS SET ASIDE FOR LATER, the last first — never deleted while it
    // is (the owner, 2026-11-09).
    items.push(setting(
        "set-aside-videos",
        texts.t(key::VIDEO_METRICS_SET_ASIDE_VIDEOS),
        count(r.set_aside_count),
        Target::SetAsideVideos,
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
    // THE SAME CATALOGUE, IN OTHER APPS (the owner, 2026-11-09): the brain's
    // page on the scan — when it runs, and changing that —, and the
    // player's two lists of it, each in a window of its own.
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_ELSEWHERE)));
    items.push(setting(
        "in-the-brain",
        texts.t(key::VIDEO_METRICS_IN_THE_BRAIN),
        String::new(),
        Target::Brain,
    ));
    items.push(setting(
        "in-the-player",
        texts.t(key::VIDEO_METRICS_IN_THE_PLAYER),
        String::new(),
        Target::InThePlayer(InThePlayer::Ranking),
    ));
    items.push(setting(
        "set-aside-in-the-player",
        texts.t(key::VIDEO_METRICS_SET_ASIDE_IN_THE_PLAYER),
        String::new(),
        Target::InThePlayer(InThePlayer::SetAside),
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
                about: Some(key::ABOUT_VIDEO_METRICS_LEFT_OUT),
                controls: Vec::new(),
                default: None,
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
            texts.t(words::why_key(why)).into(),
            tokens::color::STATE_ERROR,
        ));
    }
    items.push(setting(
        "add-channel",
        texts.t(key::VIDEO_METRICS_ADD_A_CHANNEL),
        String::new(),
        Target::AddChannel,
    ));
    // A LIST OF SUBSCRIPTIONS, BROUGHT IN OR SAVED — and, while one waits,
    // how many of its channels wait with it, decided or not: none is a
    // channel here until the list is registered. WORDED AS WAITING TO BE
    // REGISTERED, never "to decide": every one decided still waits.
    let waiting = r.staged.len();
    items.push(setting(
        "import-export",
        texts.t(key::VIDEO_METRICS_IMPORT_EXPORT),
        if r.staged.is_empty() {
            String::new()
        } else {
            texts.t_args(
                key::VIDEO_METRICS_UNREGISTERED,
                &[(
                    "count",
                    Arg::Number(i64::try_from(waiting).unwrap_or(i64::MAX)),
                )],
            )
        },
        Target::Go(Page::Import),
    ));
    if r.channels.is_empty() {
        items.push(Item::Note(
            texts.t(key::VIDEO_METRICS_NO_CHANNELS).into(),
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
            controls: ways_out(&c.id, texts),
            default: None,
        }));
    }
}

/// A CHANNEL'S WAYS OUT, glyphs at its row's right edge on the channels'
/// list: its videos' page on YouTube in Chrome, and in Nyxt — each acting on
/// the first click, as the rows of its own page do on the second.
///
/// EACH GLYPH CARRIES ITS OWN SENTENCE, named beside it: a sentence looked
/// up with another to fall back on would, on a miss, explain the glyph as
/// the channel's row — a plausible wrong sentence, said without a word.
fn ways_out(id: &ChannelId, texts: &ronoctua_i18n::Texts) -> Vec<Control> {
    [
        (
            tokens::icon::OPEN_IN_CHROME,
            Target::ChannelKiosk(id.clone()),
            key::VIDEO_METRICS_CHANNEL_IN_CHROME,
            key::ABOUT_VIDEO_METRICS_CHANNEL_IN_CHROME,
        ),
        (
            tokens::icon::OPEN_IN_NYXT,
            Target::ChannelNyxt(id.clone()),
            key::VIDEO_METRICS_CHANNEL_IN_NYXT,
            key::ABOUT_VIDEO_METRICS_CHANNEL_IN_NYXT,
        ),
    ]
    .into_iter()
    .map(|(drawn, target, name, about)| glyph(drawn, target, texts.t(name), about, false))
    .collect()
}

/// One channel: its facts, its score, its Shorts, every rule in its channel
/// version, and removing it.
///
/// A CHANNEL OF A LIST IMPORTED, not registered yet, has this page too — and
/// what registering does with it, first; everything a channel is given is
/// given to it here before it joins. When it was added (registering sets
/// it), and what only a scan writes — when it was last tried, where the
/// scans stand on it, its videos — it has none of yet, and is not said.
fn channel(items: &mut Vec<Item>, id: &ChannelId, r: &Readings, texts: &ronoctua_i18n::Texts) {
    let Some(c) = r.channel(id) else {
        items.push(Item::Heading(id.0.clone()));
        refused_or_reading(items, r, texts);
        return;
    };
    let staged = r.staged_as(id).map(|s| s.decision);
    items.push(Item::Heading(c.title.clone()));
    if let Some(decision) = staged {
        items.push(Item::Note(
            texts.t(key::VIDEO_METRICS_STAGED_NOTE).into(),
            tokens::color::TEXT_DIM,
        ));
        items.push(setting(
            "decision",
            texts.t(key::VIDEO_METRICS_DECISION),
            texts.t(words::decision_key(decision)),
            Target::Decide(id.clone(), decision),
        ));
    }
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
    if staged.is_none() {
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
        items.push(
            fact(
                "reference",
                texts.t(key::VIDEO_METRICS_REFERENCE),
                c.reference_at_s.map_or_else(
                    || texts.t(key::VIDEO_METRICS_REFERENCE_NONE),
                    |s| crate::chrome::when_text(s, &r.zone, false, texts),
                ),
            )
            .about(key::ABOUT_VIDEO_METRICS_REFERENCE),
        );
    }
    if let Some(e) = &c.last_error {
        items.push(Item::Pair(Pair {
            spot: "last-error".to_owned(),
            label: texts.t(key::VIDEO_METRICS_LAST_ERROR),
            value: e.clone(),
            kind: Kind::Fact,
            target: Target::Copy(e.clone()),
            tone: Some(tokens::color::STATE_ERROR),
            about: None,
            controls: Vec::new(),
            default: None,
        }));
    }
    if staged.is_none() {
        items.push(setting(
            "its-videos",
            texts.t(key::VIDEO_METRICS_ITS_VIDEOS),
            c.videos.to_string(),
            Target::ChannelVideos(id.clone()),
        ));
    }
    // AND ITS VIDEOS ON YOUTUBE, its `/videos` page, in either browser —
    // beside the list of them here.
    items.push(setting(
        "channel-in-chrome",
        texts.t(key::VIDEO_METRICS_CHANNEL_IN_CHROME),
        String::new(),
        Target::ChannelKiosk(id.clone()),
    ));
    items.push(setting(
        "channel-in-nyxt",
        texts.t(key::VIDEO_METRICS_CHANNEL_IN_NYXT),
        String::new(),
        Target::ChannelNyxt(id.clone()),
    ));
    items.push(Item::Pair(Pair {
        spot: "score".to_owned(),
        label: texts.t(key::VIDEO_METRICS_SCORE),
        value: c.score.to_string(),
        kind: Kind::Setting,
        target: Target::ChannelScore(id.clone(), c.score),
        tone: None,
        about: Some(key::ABOUT_VIDEO_METRICS_SCORE),
        controls: Vec::new(),
        default: None,
    }));
    // "AS EVERYWHERE" SAYS WHAT EVERYWHERE IS NOW, quietly beside it — from
    // the switches READ, never a default nobody read (`nothing_unread_…`).
    let shorts_everywhere = r.settings.map(|s| {
        texts.t(if s.block_shorts {
            key::VIDEO_METRICS_SHORTS_BLOCKED
        } else {
            key::VIDEO_METRICS_SHORTS_ALLOWED
        })
    });
    items.push(Item::Pair(Pair {
        spot: "shorts".to_owned(),
        label: texts.t(key::VIDEO_METRICS_ITS_SHORTS),
        value: texts.t(match c.block_shorts {
            None => key::VIDEO_METRICS_AS_EVERYWHERE,
            Some(true) => key::VIDEO_METRICS_SHORTS_BLOCKED,
            Some(false) => key::VIDEO_METRICS_SHORTS_ALLOWED,
        }),
        kind: Kind::Setting,
        target: Target::ChannelShorts(id.clone(), c.block_shorts),
        tone: None,
        about: None,
        controls: Vec::new(),
        default: c
            .block_shorts
            .is_none()
            .then(|| shorts_everywhere.map(|now| Follows::now(&now, texts)))
            .flatten(),
    }));
    // HOW LONG ITS VIDEOS STAY: as everywhere — what that is now beside it,
    // read and never a default — days of its own, or for ever.
    let keep_everywhere = r.settings.map(|s| words::keep(s.keep, texts));
    items.push(Item::Pair(Pair {
        spot: "keep".to_owned(),
        label: texts.t(key::VIDEO_METRICS_CHANNEL_KEEP),
        value: c.keep.map_or_else(
            || texts.t(key::VIDEO_METRICS_AS_EVERYWHERE),
            |k| words::keep(k, texts),
        ),
        kind: Kind::Setting,
        target: Target::ChannelKeep(id.clone(), c.keep),
        tone: None,
        about: None,
        controls: Vec::new(),
        default: c
            .keep
            .is_none()
            .then(|| keep_everywhere.map(|now| Follows::now(&now, texts)))
            .flatten(),
    }));
    expiring(items, r, texts);
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

/// THE IMPORT PAGE: Google Takeout's list of the channels an account
/// subscribes to — where it is exported, choosing it, the lines the last
/// import left out and the channels it found here already —; the channels
/// here saved as such a list; and the channels it staged — how many are
/// decided, all of them at once, registering what was decided, then each,
/// waiting to be kept or discarded.
///
/// THE SHORT ROWS BEFORE THE LONG LIST: an account follows hundreds of
/// channels, and a row after them — what the import found here already,
/// registering, exporting — would be hundreds of rows away, where nobody
/// scrolls.
fn import(
    items: &mut Vec<Item>,
    imported: Option<&Imported>,
    r: &Readings,
    texts: &ronoctua_i18n::Texts,
) {
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_IMPORT)));
    if let Some(Err(why)) = r.catalogue
        && why != Why::NotThere
    {
        items.push(Item::Note(
            texts.t(words::why_key(why)).into(),
            tokens::color::STATE_ERROR,
        ));
    }
    items.push(Item::Note(
        texts.t(key::VIDEO_METRICS_IMPORT_NOTE).into(),
        tokens::color::TEXT_DIM,
    ));
    items.push(setting(
        "takeout",
        texts.t(key::VIDEO_METRICS_TAKEOUT),
        takeout_shown(),
        Target::Takeout,
    ));
    items.push(setting(
        "choose-list",
        texts.t(key::VIDEO_METRICS_CHOOSE_LIST),
        String::new(),
        Target::PickImport,
    ));
    // WHAT THE LAST IMPORT LEFT OUT — the view's memory: a list chosen
    // again says it again.
    if let Some(i) = imported {
        items.push(
            fact(
                "left-out",
                texts.t(key::VIDEO_METRICS_IMPORT_LEFT_OUT),
                i.left_out.to_string(),
            )
            .about(key::ABOUT_VIDEO_METRICS_IMPORT_LEFT_OUT),
        );
    }
    // THE CHANNELS OF THE LAST IMPORT THAT WERE HERE ALREADY, each a way to
    // its page — never staged, and left as they are: beside what else that
    // import found, and before the channels it staged, which run to
    // hundreds — after them, nobody would scroll down to these.
    if let Some(i) = imported.filter(|i| !i.already.is_empty()) {
        items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_ALREADY)));
        for (id, title) in &i.already {
            items.push(Item::Pair(Pair {
                spot: format!("already:{}", id.0),
                label: title.clone(),
                value: String::new(),
                kind: Kind::ListedFact,
                target: Target::Go(Page::Channel(id.clone())),
                tone: None,
                about: None,
                controls: Vec::new(),
                default: None,
            }));
        }
    }
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_EXPORT)));
    items.push(setting(
        "save-list",
        texts.t(key::VIDEO_METRICS_SAVE_LIST),
        String::new(),
        Target::PickExport,
    ));
    // THE CHANNELS STAGED, by title — never grouped by decision, so a row
    // the person turns stays under the cursor (`query::staged`).
    if !r.staged.is_empty() {
        items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_TO_DECIDE)));
        let count = |d: Decision| {
            let n = r.staged.iter().filter(|s| s.decision == d).count();
            Arg::Number(i64::try_from(n).unwrap_or(i64::MAX))
        };
        items.push(
            fact(
                "decisions",
                texts.t(key::VIDEO_METRICS_DECISIONS),
                texts.t_args(
                    key::VIDEO_METRICS_DECISIONS_VALUE,
                    &[
                        ("keep", count(Decision::Keep)),
                        ("discard", count(Decision::Discard)),
                        ("undecided", count(Decision::Undecided)),
                    ],
                ),
            )
            .about(key::ABOUT_VIDEO_METRICS_DECISIONS),
        );
        let alike = r
            .staged
            .first()
            .map(|s| s.decision)
            .filter(|d| r.staged.iter().all(|s| s.decision == *d));
        items.push(setting(
            "decide-all",
            texts.t(key::VIDEO_METRICS_DECIDE_ALL),
            texts.t(alike.map_or(key::VIDEO_METRICS_DECISION_MIXED, words::decision_key)),
            Target::DecideAll(alike),
        ));
        // REGISTERING, once something is decided — the undecided stay.
        if r.staged.iter().any(|s| s.decision != Decision::Undecided) {
            items.push(setting(
                "register",
                texts.t(key::VIDEO_METRICS_REGISTER),
                String::new(),
                Target::Register,
            ));
        }
        for s in &r.staged {
            let id = &s.channel.id;
            items.push(Item::Pair(Pair {
                spot: format!("staged:{}", id.0),
                label: s.channel.title.clone(),
                value: texts.t(words::decision_key(s.decision)),
                kind: Kind::ListedSetting,
                target: Target::Staged(id.clone(), s.decision),
                tone: None,
                about: None,
                // ITS VIDEOS ON YOUTUBE, in either browser, before it is
                // decided — as a channel's row of the list has them.
                controls: ways_out(id, texts),
                default: None,
            }));
        }
    }
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
                texts.t(key::VIDEO_METRICS_BLOCK_SHORTS_NOTE).into(),
                tokens::color::TEXT_DIM,
            ));
            // HOW LONG A VIDEO STAYS, everywhere a channel keeps no time of
            // its own — a ring, and any number of days typed.
            items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_KEEP)));
            items.push(setting(
                "keep",
                texts.t(key::VIDEO_METRICS_KEEP_AFTER),
                words::keep(settings.keep, texts),
                Target::KeepEverywhere(settings.keep),
            ));
            expiring(items, r, texts);
        }
        None => refused_or_reading(items, r, texts),
    }
    items.push(Item::Heading(texts.t(key::VIDEO_METRICS_HEADING_SCORE)));
    rule_rows(items, scope, r, texts);
}

/// WHAT THE NEXT SCAN TAKES OUT FOR ITS TIME, under the keeps in force —
/// quietly, under a keep's row, so a small number typed by mistake surprises
/// nobody before the scan (Reconciliation 10). The catalogue's whole count,
/// worded so; said only when there is one.
fn expiring(items: &mut Vec<Item>, r: &Readings, texts: &ronoctua_i18n::Texts) {
    if let Some(n) = r.expiring.filter(|n| *n > 0) {
        items.push(Item::Note(
            texts
                .t_args(
                    key::VIDEO_METRICS_EXPIRING,
                    &[("count", Arg::Number(i64::try_from(n).unwrap_or(i64::MAX)))],
                )
                .into(),
            tokens::color::TEXT_DIM,
        ));
    }
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
/// scope, that it is the channel's own. The one everywhere says its points
/// alone, and that it is everywhere's apart, quietly ([`everywhere_rule`]):
/// "2 000 points (everywhere)" reads the value first and the scope as an
/// aside, where "2 000 points · everywhere" read two values.
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
    if scope.is_none() || rule.channel.is_none() {
        return points;
    }
    texts.t_with(
        key::VIDEO_METRICS_RULE_IN,
        &[
            ("value", &points),
            ("scope", &texts.t(key::VIDEO_METRICS_SCOPE_OWN)),
        ],
    )
}

/// A RULE FROM EVERYWHERE, on a channel's page: what it follows — the rule
/// as it is everywhere now, its value the row's —, said apart from the
/// value, quietly ([`Follows`]). Under the pointer it says what changing it
/// does: a single rule's page makes the channel its own version
/// (`rule_page`), and a band's or words' page is the one everywhere, which
/// changes it on every channel (`rule_page_of`). `None` off a channel's
/// page, and for the channel's own.
fn everywhere_rule(
    rule: &Rule,
    scope: &Scope,
    r: &Readings,
    texts: &ronoctua_i18n::Texts,
) -> Option<Follows> {
    let id = scope.as_ref().filter(|_| rule.channel.is_none())?;
    let said = if rule.kind.is_single() {
        let name = r
            .channel(id)
            .map_or_else(|| id.0.clone(), |c| c.title.clone());
        // THE CHANNEL IS A NAME (`Arg::Name`), told as every name is; what
        // explains a row (`door::Explained`) is said in its own inks, which
        // quote no name, so only the words go.
        texts
            .t_args_marked(
                key::VIDEO_METRICS_RULE_INHERITED_NOTE,
                &[("name", ronoctua_i18n::Arg::Name(&name))],
            )
            .text
    } else {
        texts.t(key::VIDEO_METRICS_FOLLOWS_RULE_EVERYWHERE)
    };
    Some(Follows {
        beside: texts.t_with(
            key::VIDEO_METRICS_DEFAULT_NOW,
            &[("value", &texts.t(key::VIDEO_METRICS_SCOPE_EVERYWHERE))],
        ),
        said,
    })
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
            controls: Vec::new(),
            default: found.and_then(|x| everywhere_rule(x, scope, r, texts)),
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
            about: Some(about_of_kind(&b.kind)),
            controls: Vec::new(),
            default: everywhere_rule(b, scope, r, texts),
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
            texts.t(key::VIDEO_METRICS_BANDS_NOTE).into(),
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
            about: Some(about_of_kind(&t.kind)),
            controls: Vec::new(),
            default: everywhere_rule(t, scope, r, texts),
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
                texts.t(words::why_key(Why::NotFound)).into(),
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
        // THE CHANNEL BY ITS NAME, in a name's ink near the note's.
        items.push(Item::Note(
            texts
                .t_args_marked(
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
                    &[("name", Arg::Name(&channel.title))],
                )
                .into(),
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
            controls: Vec::new(),
            default: None,
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
        texts.t(key::VIDEO_METRICS_WEIGHTS_NOTE).into(),
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
            controls: Vec::new(),
            default: None,
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
                texts.t(words::why_key(Why::NotFound)).into(),
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
    items.push(
        fact(
            "published",
            texts.t(key::VIDEO_METRICS_PUBLISHED),
            row.published_at_s
                .map(|s| crate::chrome::when_text(s, &r.zone, false, texts))
                .unwrap_or_default(),
        )
        .about(key::ABOUT_VIDEO_METRICS_PUBLISHED),
    );
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
    items.push(
        fact(
            "static",
            texts.t(key::VIDEO_METRICS_ITS_POINTS),
            words::points(v.static_points, texts),
        )
        .about(key::ABOUT_VIDEO_METRICS_ITS_POINTS),
    );
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
        items.push(Item::Note(
            v.description.clone().into(),
            Kind::Fact.tones(false).1,
        ));
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
    // SET ASIDE FOR LATER, AND NEVER DELETED: each a switch of its own, the
    // second saying, quietly, what keeps the video — or when it goes —
    // without its mark.
    let aside = row.set_aside_at_s.is_some();
    items.push(Item::Pair(Pair {
        spot: "set-aside".to_owned(),
        label: texts.t(key::VIDEO_METRICS_SET_ASIDE_ROW),
        value: on_off(aside, texts),
        kind: Kind::Setting,
        target: Target::SetAside(id.clone(), aside),
        tone: None,
        about: Some(key::ABOUT_VIDEO_METRICS_SET_ASIDE_ROW),
        controls: Vec::new(),
        default: None,
    }));
    let own = row.forever == Forever::ByVideo;
    let follows = (!own).then(|| {
        let value = lasting(row, Some(v.first_seen_at_s), r).words(r, texts);
        Follows {
            beside: texts.t_with(key::VIDEO_METRICS_DEFAULT_NOW, &[("value", &value)]),
            said: texts.t_with(key::VIDEO_METRICS_WITHOUT_THE_MARK, &[("value", &value)]),
        }
    });
    items.push(Item::Pair(Pair {
        spot: "never-delete".to_owned(),
        label: texts.t(key::VIDEO_METRICS_NEVER_DELETE),
        value: on_off(own, texts),
        kind: Kind::Setting,
        target: Target::VideoKeep(id.clone(), own),
        tone: None,
        about: Some(key::ABOUT_VIDEO_METRICS_NEVER_DELETE),
        controls: Vec::new(),
        default: follows,
    }));
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
        controls: Vec::new(),
        default: None,
    }));
    if !v.second_description.trim().is_empty() {
        items.push(Item::Note(
            v.second_description.clone().into(),
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
        "nyxt",
        texts.t(key::CMD_VIDEO_METRICS_NYXT),
        String::new(),
        Target::Nyxt(id.clone()),
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
        controls: Vec::new(),
        thumbs: Vec::new(),
        caret: None,
        about: None,
        name: String::new(),
        default: None,
        ground: None,
        words_at: None,
        wholes: Vec::new(),
    }
}

/// THE LINES OF A PAIR WITH WHAT IT FOLLOWS "AS EVERYWHERE" ([`Follows`]):
/// `beside`, in the quietest ink, a cell of air after the LAST piece of the
/// value — on its line when it fits before `end`, else under it in the
/// value's column, wrapped there, never cut: on the lines a wrapped label
/// still takes when it has them (their value column is free), and on lines
/// of its own after them. Laid AFTER the value's tint (`chrome::tinted`
/// recolours every field after a line's first, and a state's colour would
/// have reached it), so it stays quiet whatever the value says and wherever
/// the cursor is.
fn with_beside(mut lines: Vec<Vec<Field>>, beside: &str, column: u16, end: u16) -> Vec<Vec<Field>> {
    use ronoctua_render::layout::width;
    let quiet = tokens::color::TEXT_DIM;
    let wide = width(beside);
    let last = lines.iter().enumerate().rev().find_map(|(k, fields)| {
        fields
            .iter()
            .rfind(|f| f.at >= column)
            .map(|f| (k, f.at + width(&f.text) + 1))
    });
    if let Some((k, at)) = last
        && at + wide <= end
    {
        lines[k].push(piece(at, beside.to_owned(), quiet, wide));
        return lines;
    }
    // UNDER THE VALUE: the line after its last piece holds a label's rest at
    // most, which never reaches the column. With no value at all the label
    // took the whole row, and only lines after it are free.
    let first = last.map_or(lines.len(), |(k, _)| k + 1);
    let parts = crate::chrome::note_lines(beside, end.saturating_sub(column).max(1));
    for (k, part) in (first..).zip(parts) {
        let field = piece(column, part, quiet, end.saturating_sub(column));
        match lines.get_mut(k) {
            Some(line) => line.push(field),
            None => lines.push(vec![field]),
        }
    }
    lines
}

/// The items, placed: the values of the whole page in one column after its
/// widest label (`chrome::section_column`), labels wrapped and never cut,
/// notes wrapped, air before every title but the first — and the cursor's
/// row in the cursor's colour.
fn lay_out(items: Vec<Item>, left: u16, room: u16, at: &str) -> Vec<Row> {
    use ronoctua_render::layout::width;
    let end = left + room;
    // THE COLUMN IS MEASURED ON THE ROWS THAT HAVE A VALUE: a row that is
    // only a label — "Delete it for good: it never comes back" — has nothing
    // to align, and would push every value on the page to the right. A
    // value that follows everywhere is measured WITH what it follows beside
    // it, so the column leaves the room for both on one line where it can.
    let pairs: Vec<(u16, u16, u16)> = items
        .iter()
        .filter_map(|i| match i {
            // AND WITH ITS GLYPHS, at the row's right edge: the column leaves
            // the room for the value before them too.
            Item::Pair(p) if !p.value.is_empty() => Some((
                left,
                width(&p.label),
                width(&p.value)
                    + p.default.as_ref().map_or(0, |d| 1 + width(&d.beside))
                    + glyphs_room(&p.controls),
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
                // AIR BEFORE A TITLE — but the first, and one already there:
                // a question's block ends in a line of its own.
                if !out.is_empty() && !out.last().is_some_and(is_air) {
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
            Item::Titled {
                text,
                mark,
                controls,
            } => {
                if !out.is_empty() && !out.last().is_some_and(is_air) {
                    out.push(plain(crate::chrome::AIR, true, spot("air")));
                }
                // THE TITLE AND ITS MARK in what the arrows leave, a cell of
                // air between the two: on the title's line while the whole
                // mark fits after it, else wrapped under it, whole — never
                // cut, in either ink.
                let words = room.saturating_sub(slots_room(&controls)).max(1);
                let mut lines: Vec<Vec<Field>> = crate::chrome::note_lines(&text, words)
                    .into_iter()
                    .map(|line| vec![piece(left, line, tokens::color::TEXT_HEADING, words)])
                    .collect();
                if let Some(mark) = mark {
                    let quiet = tokens::color::TEXT_DIM;
                    let after = lines
                        .last()
                        .and_then(|l| l.last())
                        .map_or(left, |f| f.at + width(&f.text) + 1);
                    match lines.last_mut() {
                        Some(last) if after + width(&mark) <= left + words => {
                            last.push(piece(after, mark, quiet, words));
                        }
                        _ => {
                            for line in crate::chrome::note_lines(&mark, words) {
                                lines.push(vec![piece(left, line, quiet, words)]);
                            }
                        }
                    }
                }
                for (k, fields) in lines.into_iter().enumerate() {
                    let mut row = plain(
                        fields,
                        true,
                        if k == 0 {
                            crate::paging::LIST_TITLE.to_owned()
                        } else {
                            spot("title")
                        },
                    );
                    if k == 0 {
                        put_slots(&mut row, &controls, end, tokens::color::TEXT_HEADING);
                    }
                    out.push(row);
                }
            }
            Item::Pager { mark, controls } => {
                if !out.is_empty() && !out.last().is_some_and(is_air) {
                    out.push(plain(crate::chrome::AIR, true, spot("air")));
                }
                let quiet = tokens::color::TEXT_DIM;
                let words = room.saturating_sub(slots_room(&controls)).max(1);
                let mut row = plain(
                    vec![piece(left, mark, quiet, words)],
                    true,
                    crate::paging::LIST_PAGER.to_owned(),
                );
                put_slots(&mut row, &controls, end, quiet);
                out.push(row);
            }
            Item::Note(said, color) => {
                // THE LINES ONE INK WOULD TAKE, its names in theirs
                // (`chrome::note_fields`): each paragraph a line of its own or
                // more, an empty one a row of nothing — and a sentence that
                // ENDS a paragraph (a description's last newline) one more
                // empty row, as splitting it at every newline always gave.
                for fields in crate::chrome::note_fields(left, &said, color, room) {
                    out.push(plain(fields, true, spot("note")));
                }
                if said.ends_with('\n') {
                    out.push(plain(Vec::new(), true, spot("note")));
                }
            }
            Item::Pair(p) => {
                let focused = p.spot == at;
                // A ROW WITH GLYPHS ends its words a cell of air before them,
                // and they stand at the row's right edge on its first line.
                let words_end = end.saturating_sub(glyphs_room(&p.controls));
                let lines: Vec<Vec<Field>> = crate::chrome::aligned_pair(
                    left, column, words_end, &p.label, &p.value, p.kind, focused,
                )
                .into_iter()
                .enumerate()
                .map(|(k, fields)| {
                    if k == 0 {
                        crate::chrome::tinted(fields, p.tone, focused)
                    } else {
                        fields
                    }
                })
                .collect();
                let lines = match &p.default {
                    Some(d) => with_beside(lines, &d.beside, column, words_end),
                    None => lines,
                };
                for (k, fields) in lines.into_iter().enumerate() {
                    let mut row = Row {
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
                        controls: Vec::new(),
                        thumbs: Vec::new(),
                        caret: None,
                        about: p.about,
                        name: p.label.clone(),
                        default: p.default.clone(),
                        ground: None,
                        words_at: None,
                        wholes: Vec::new(),
                    };
                    if k == 0 && !p.controls.is_empty() {
                        put_strip(
                            &mut row,
                            &p.controls,
                            end.saturating_sub(strip_cells(&p.controls)),
                        );
                    }
                    out.push(row);
                }
            }
            Item::Question {
                asked,
                answers,
                stakes,
            } => {
                let lines = crate::chrome::question_lines(
                    &asked,
                    [answers[0].as_str(), answers[1].as_str()],
                    left,
                    room,
                );
                laid_question(
                    &mut out,
                    &lines,
                    &answers,
                    stakes,
                    &mut spot,
                    |yes| (answer_spot(yes).to_owned(), Target::Answer(yes)),
                    |yes| answer_spot(yes) == at,
                );
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
            Item::Strip(t) if strip_holds_a_picture(room) => {
                strip_entry(&mut out, &t, left, room, at, &mut spot);
            }
            Item::Entry(t) | Item::Strip(t) => {
                let focused = t.spot == at;
                let (label, value) = entry_tones(&t, focused);
                for (k, line) in crate::chrome::note_lines(&t.title, room)
                    .into_iter()
                    .enumerate()
                {
                    let mut row = plain(
                        vec![piece(left, line, label, room)],
                        false,
                        if k == 0 { t.spot.clone() } else { spot("rest") },
                    );
                    row.wholes = title_whole(&row, &t.title);
                    if k == 0 {
                        row.target = t.target.clone();
                    } else {
                        row.continues = true;
                    }
                    row.name.clone_from(&t.title);
                    out.push(row);
                }
                let mut row = plain(Vec::new(), false, spot("rest"));
                row.continues = true;
                row.name.clone_from(&t.title);
                entry_line(&mut row, &t, value, left, end);
                out.push(row);
            }
            Item::Shelf(tiles, grid, layout) => {
                shelf(&mut out, &tiles, grid, layout, left, at, &mut spot);
            }
        }
    }
    out
}

/// Whether `row` is a LINE OF AIR between two blocks — the one a title
/// puts before itself, the ones around a question, and the one a video of
/// the list with thumbnails ends with ([`strip_entry`]).
fn is_air(row: &Row) -> bool {
    row.spot.starts_with("air#")
}

/// A QUESTION'S LINES ([`crate::chrome::question_lines`]) as rows of the
/// page, after `out`'s: its air off the ground, and every other line on it;
/// the sentence a note the cursor skips; each answer a row the cursor
/// stops on, its first line `first_of(yes)` — its spot and what `RET` does
/// there — and the lines that wrap it CONTINUING it ([`Row::owner`]), so a
/// click anywhere on it is a click on it. `here(yes)`: the cursor is on that
/// answer, whichever line of it, and every line of it wears the cursor's
/// colour. No air where the line before is air already.
fn laid_question(
    out: &mut Vec<Row>,
    lines: &[crate::chrome::QuestionLine],
    answers: &[String; 2],
    stakes: crate::chrome::Stakes,
    spot: &mut impl FnMut(&str) -> String,
    first_of: impl Fn(bool) -> (String, Target),
    here: impl Fn(bool) -> bool,
) {
    use crate::chrome::QuestionLine;
    for line in lines {
        let mut row = match line {
            QuestionLine::Air => {
                if out.is_empty() || out.last().is_some_and(is_air) {
                    continue;
                }
                plain(crate::chrome::AIR, true, spot("air"))
            }
            QuestionLine::Asked(_) => plain(line.fields(false, stakes), true, spot("asked")),
            QuestionLine::Answer { yes, first, .. } => {
                let mut row = plain(line.fields(here(*yes), stakes), false, spot("rest"));
                if *first {
                    (row.spot, row.target) = first_of(*yes);
                } else {
                    row.continues = true;
                }
                row.name.clone_from(&answers[usize::from(*yes)]);
                row
            }
        };
        row.ground = line.ground();
        out.push(row);
    }
}

/// THE LINE UNDER A LIST'S TITLE, from `left` to `end`: its channel, its
/// age and its score — a bare number — and the four arrows of its rating
/// right after it, and its marks at the line's right edge. The words before
/// the arrows are the row's (`Row::words_box`): a click there takes the
/// video, a click on a glyph does what it says. Where the line is too short
/// the channel goes first, then the age; the score is never cut, and the
/// glyphs go before it would be.
fn entry_line(row: &mut Row, t: &Tile, colour: tokens::Color, left: u16, end: u16) {
    use ronoctua_render::layout::width;
    let room = end.saturating_sub(left);
    let arrows = strip_cells(&t.ratings);
    let marks = strip_cells(&t.marks);
    // WHAT THE WORDS BEFORE THE ARROWS MAY TAKE: the line less the arrows, the
    // marks and the air before each.
    let before = room.saturating_sub(
        GLYPH_AIR
            .saturating_add(arrows)
            .saturating_add(GLYPH_AIR)
            .saturating_add(marks),
    );
    let chosen = t.said.iter().position(|w| width(w) <= before);
    let words = chosen.map_or(t.score.as_str(), |k| t.said[k].as_str());
    row.fields
        .push(piece(left, words.to_owned(), colour, width(words)));
    // ITS CHANNEL, where the words say it, names itself whole under the
    // pointer — the thing explained there is the channel, not the video.
    if chosen == Some(0)
        && let Some((from, cells)) = t.channel_in_said.filter(|(_, cells)| *cells > 0)
    {
        row.wholes.push(Whole {
            from: left + from,
            width: cells,
            text: t.channel.clone(),
        });
    }
    let at = left.saturating_add(width(words)).saturating_add(GLYPH_AIR);
    // THE GLYPHS ONLY WHERE THEY FIT WHOLE — the marks against the edge,
    // the arrows after the score with a cell of air before the marks.
    let marks_from = end.saturating_sub(marks);
    if at.saturating_add(arrows).saturating_add(GLYPH_AIR) <= marks_from {
        put_strip(row, &t.ratings, at);
    }
    if at <= marks_from {
        put_strip(row, &t.marks, marks_from);
    }
}

/// The tones of a video of the list, its title's and its words': a hidden
/// one's are a disabled row's.
fn entry_tones(t: &Tile, focused: bool) -> (tokens::Color, tokens::Color) {
    if t.hidden {
        Kind::Disabled.tones(focused)
    } else {
        Kind::ListedSetting.tones(focused)
    }
}

/// WHETHER A DOOR `room` CELLS WIDE HOLDS A VIDEO'S PICTURE beside a title as
/// wide again, a cell of air between — the player's strip's rule
/// (`player_posters::strip`). Narrower, the list with thumbnails is the list:
/// a title squeezed into the cells a picture leaves would be a word a line.
const fn strip_holds_a_picture(room: u16) -> bool {
    room >= tokens::space::VIDEOMETRICS_THUMBCELLS.saturating_mul(2) + ronoctua_design::GAP
}

/// A VIDEO OF THE LIST WITH THUMBNAILS (the owner, 2026-11-10: "uma lista com
/// as imagens, como a lista do player") — the player's strip in this view's
/// rows, which are not the player's (`player_posters::strip` writes
/// `player_rows::Row`s): the picture at the head, `thumbCells` by
/// `thumbLines` — the glyph of a video in its place until the scan fetches
/// one —, and a cell of air after it, the title beside it WRAPPED WHOLE,
/// never cut (the list's own rule: a title is the long half of a video), and
/// under the title what it scores, with its glyphs ([`entry_line`], from the
/// words' column). As tall as the title and that line, or as the picture
/// when it is taller — and a line of air after it, the entry's own: two
/// pictures one over the other read as one strip.
///
/// EVERY LINE CONTINUES THE ENTRY and says where its words begin
/// ([`Row::words_at`]): the pointer lights the words, on every line of the
/// video and never over its picture, and a click from there on takes the
/// video — but on the line of glyphs, where the row's words are their box
/// and each glyph acts on its own. The picture takes nothing and lights
/// nothing, on any of its lines ([`View::on_the_words`]): not the player's
/// strip's rule (`Lit::From`, a click anywhere on the line), the plan's.
fn strip_entry(
    out: &mut Vec<Row>,
    t: &Tile,
    left: u16,
    room: u16,
    at: &str,
    spot: &mut impl FnMut(&str) -> String,
) {
    let cells = tokens::space::VIDEOMETRICS_THUMBCELLS;
    let lines = tokens::space::VIDEOMETRICS_THUMBLINES;
    let end = left + room;
    let text_at = left + cells + ronoctua_design::GAP;
    let words = end.saturating_sub(text_at).max(1);
    let (label, value) = entry_tones(t, t.spot == at);
    let title = crate::chrome::note_lines(&t.title, words);
    let tall = (title.len() + 1).max(usize::from(lines));
    for (k, line) in (0..tall).zip(0u16..) {
        let mut row = plain(
            Vec::new(),
            false,
            if k == 0 { t.spot.clone() } else { spot("rest") },
        );
        if k == 0 {
            row.target = t.target.clone();
        } else {
            row.continues = true;
        }
        row.name.clone_from(&t.title);
        row.words_at = Some(text_at);
        if let Some(text) = title.get(k) {
            row.fields.push(piece(text_at, text.clone(), label, words));
            row.wholes = title_whole(&row, &t.title);
        } else if k == title.len() {
            entry_line(&mut row, t, value, text_at, end);
        }
        // THE PICTURE ON THE LINES OF ITS BOX, and on no other.
        if line < lines {
            match &t.file {
                Some(file) => row.thumbs.push(Thumb {
                    line,
                    lines,
                    left,
                    cells,
                    file: file.clone(),
                }),
                // NO PICTURE YET: the glyph of a video in its place, centred,
                // as a tile of the grid says it.
                None if line == lines / 2 => row.fields.push(piece(
                    left + cells / 2,
                    tokens::icon::VIDEO_NO_THUMB.to_string(),
                    tokens::color::TEXT_DIM,
                    1,
                )),
                None => {}
            }
        }
        out.push(row);
    }
    // ITS LINE OF AIR, the video's — a title after the list takes no second
    // one ([`is_air`]).
    let mut row = plain(Vec::new(), false, spot("air"));
    row.continues = true;
    row.name.clone_from(&t.title);
    row.words_at = Some(text_at);
    out.push(row);
}

/// The name whose words are at cell `column` of line `n` ([`Row::wholes`]),
/// whatever is under the pointer there.
fn whole_on(rows: &[Row], n: usize, column: Option<u16>) -> Option<&str> {
    let col = column?;
    rows.get(n)?
        .wholes
        .iter()
        .find(|w| crate::chrome::in_cell(col, w.from, w.width))
        .map(|w| w.text.as_str())
}

/// THE TITLE'S NAME on a line of it, `row` holding the line's one field:
/// the cells of its words, said whole under the pointer.
fn title_whole(row: &Row, title: &str) -> Vec<Whole> {
    row.fields
        .first()
        .map(|f| Whole {
            from: f.at,
            width: ronoctua_render::layout::width(&f.text),
            text: title.to_owned(),
        })
        .into_iter()
        .collect()
}

/// The field just put on a shelf's line is a NAME — a tile's title or its
/// channel, cut to the tile: its cells say `whole` under the pointer.
fn named(row: &mut Row, whole: &str) {
    if let Some(f) = row.fields.last() {
        row.wholes.push(Whole {
            from: f.at,
            width: ronoctua_render::layout::width(&f.text),
            text: whole.to_owned(),
        });
    }
}

/// One shelf of the grid: the pictures' lines, the titles' two, the
/// channels' when the layout says so, the line that says what each scores
/// with the arrows of its rating, the one that says when with its marks, and
/// a line of air. A WIDE TILE ([`Layout::Wide`]) says its score and its age
/// beside its picture, on the picture's first two lines and a gap after it,
/// as wide as a thumbnail — and its title and its channel under the
/// picture, the whole tile wide. Each tile's cell keeps [`TILE_AIR`] on both
/// sides of it, so the hover that lights the cell has the same air on both
/// sides of the tile. The FIRST line carries the cells a click takes — the
/// box the hover lights — and every line after it continues it: a click on
/// a glyph takes the glyph under it ([`View::click_control`]), and anywhere
/// else in a cell its video, on any line of the shelf, for the shelf's words
/// are its videos' ([`Row::has_words`]).
fn shelf(
    out: &mut Vec<Row>,
    tiles: &[Tile],
    grid: ronoctua_design::grid::Grid,
    layout: Layout,
    left: u16,
    at: &str,
    spot: &mut impl FnMut(&str) -> String,
) {
    let lines = tokens::space::VIDEOMETRICS_THUMBLINES;
    let title_lines = tokens::space::VIDEOMETRICS_TITLELINES;
    let thumb = tokens::space::VIDEOMETRICS_THUMBCELLS;
    let width = tile_cells(layout);
    let first = out.len();
    // WHERE EACH TILE STARTS.
    let placed: Vec<(u16, &Tile)> = tiles
        .iter()
        .enumerate()
        .filter_map(|(i, t)| grid.at(i).map(|(_, column)| (left + column, t)))
        .collect();
    let total = shelf_lines(layout);
    // THE LINES OF ITS WORDS: the title's under the picture, and its channel
    // when the layout says it; the score with the arrows of its rating and
    // the age with its marks — under them on a tile of the grid, the tile's
    // width; beside the picture on a wide one, from `facts` cells on.
    let channel_line = layout.channel().then_some(lines + title_lines);
    let (score_line, facts) = match layout {
        Layout::Wide => (0, thumb + ronoctua_design::GAP),
        Layout::List | Layout::Strip | Layout::Grid | Layout::GridWithChannel => {
            (lines + title_lines + u16::from(layout.channel()), 0)
        }
    };
    let facts_cells = width - facts;
    let age_line = score_line + 1;
    for line in 0..total {
        let mut row = plain(Vec::new(), false, spot("shelf"));
        row.continues = line > 0;
        for (from, t) in &placed {
            let from = *from;
            let focused = t.spot == at;
            if line < lines {
                if let Some(file) = &t.file {
                    row.thumbs.push(Thumb {
                        line,
                        lines,
                        left: from,
                        cells: thumb,
                        file: file.clone(),
                    });
                } else if line == lines / 2 {
                    // NO PICTURE YET: the glyph of a video in its place,
                    // centred, until the scan fetches one.
                    let glyph = tokens::icon::VIDEO_NO_THUMB.to_string();
                    row.fields
                        .push(piece(from + thumb / 2, glyph, tokens::color::TEXT_DIM, 1));
                }
            }
            if (lines..lines + title_lines).contains(&line) {
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
                    row.fields.push(piece(from, text, label, width));
                    named(&mut row, &t.title);
                }
            }
            if channel_line == Some(line) {
                row.fields.push(piece(
                    from,
                    t.channel.clone(),
                    Kind::ListedFact.tones(focused).1,
                    width,
                ));
                named(&mut row, &t.channel);
            }
            let facts_at = from + facts;
            if line == score_line {
                // THE SCORE, never cut for a glyph — and the arrows of its
                // rating a cell after it, where they fit whole.
                let number = ronoctua_render::layout::width(&t.score);
                row.fields.push(piece(
                    facts_at,
                    t.score.clone(),
                    Kind::ListedFact.tones(focused).1,
                    facts_cells,
                ));
                if number + GLYPH_AIR + strip_cells(&t.ratings) <= facts_cells {
                    put_strip(&mut row, &t.ratings, facts_at + number + GLYPH_AIR);
                }
            } else if line == age_line {
                // ITS MARKS against the tile's right edge, and its age in
                // the room they leave, cut there.
                let marks = strip_cells(&t.marks).min(facts_cells);
                let room = facts_cells.saturating_sub(marks + GLYPH_AIR);
                if room > 0 && !t.age.is_empty() {
                    row.fields.push(piece(
                        facts_at,
                        t.age.clone(),
                        Kind::ListedFact.tones(focused).1,
                        room,
                    ));
                }
                put_strip(&mut row, &t.marks, facts_at + facts_cells - marks);
            }
        }
        if line == 0 {
            row.cells = placed
                .iter()
                .map(|(from, t)| Cell {
                    from: from.saturating_sub(TILE_AIR),
                    width: width + 2 * TILE_AIR,
                    target: t.target.clone(),
                    spot: t.spot.clone(),
                    name: t.title.clone(),
                    about: about_of(&t.target).unwrap_or(key::ABOUT_VIDEO_METRICS_VIDEO_ENTRY),
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
    let name = texts.t(key::VIDEO_METRICS_YOUR_DESCRIPTION);
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
        row.name.clone_from(&name);
        out.push(row);
    }
    // ITS QUESTION IS A QUESTION'S BLOCK, the page's own drawing — on its
    // ground, with its air, "no" first — and its yes LOSES the text. Its
    // answers are a question's answers (`Target::Answer`) for what explains
    // them; a click on any line of one is read back as that answer
    // (`text_hit`, by the row it continues), and never reaches the page's
    // own question.
    if let Some(yes) = area.asking() {
        let answers = [texts.t(key::TEXT_AREA_NO), texts.t(key::TEXT_AREA_YES)];
        let lines = crate::chrome::question_lines(
            &texts.t(key::ASK_TEXT_AREA_DISCARD).into(),
            [answers[0].as_str(), answers[1].as_str()],
            left,
            room,
        );
        let mut n = 0usize;
        let mut spot = |kind: &str| {
            n += 1;
            format!("text-{kind}#{n}")
        };
        laid_question(
            &mut out,
            &lines,
            &answers,
            crate::chrome::Stakes::Lost,
            &mut spot,
            |answer| (text_answer_spot(answer), Target::Answer(answer)),
            |answer| answer == yes,
        );
    }
    out
}

/// The spot of an answer of the description's question.
fn text_answer_spot(yes: bool) -> String {
    format!("text-answer-{yes}")
}

/// The row the cursor of the description being edited is on — the caret's,
/// or, while its question is up (`asking`: the answer its cursor is on), the
/// first line of that answer — which the door keeps in sight. Found by the
/// answer's SPOT, never by its colour: the cursor's colour is the drawing's
/// to choose.
#[must_use]
pub fn text_cursor(rows: &[Row], asking: Option<bool>) -> Option<usize> {
    match asking {
        Some(yes) => {
            let spot = text_answer_spot(yes);
            rows.iter().position(|r| r.spot == spot)
        }
        None => rows.iter().position(|r| r.caret.is_some()),
    }
}

/// What a click on the text being edited landed on, at `column` (a field's
/// `at`): a line, or an answer of its question — on any line of it, a long
/// answer wrapped being one ([`Row::owner`]).
#[must_use]
pub fn text_hit(rows: &[Row], n: usize, column: u16) -> Option<crate::text_area::Hit> {
    let row = rows.get(n)?;
    if let Target::Text(at) = &row.target {
        return Some(crate::text_area::Hit::Text {
            at: *at,
            cell: column.saturating_sub(row.fields.first().map_or(0, |f| f.at)),
        });
    }
    match &rows.get(Row::owner(rows, n))?.target {
        Target::Answer(yes) => Some(crate::text_area::Hit::Answer(*yes)),
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
pub(crate) mod tests {
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
            keep: None,
            videos: 10,
        }
    }

    /// A CHANNEL OF A LIST IMPORTED, as staging leaves it: its address by
    /// its id, never scanned, no video — and `decision`.
    fn staged(n: u8, decision: Decision) -> Staged {
        let id = ChannelId(format!("UC{n:022}"));
        Staged {
            channel: Channel {
                handle: None,
                url: format!("https://www.youtube.com/channel/{}", id.0),
                reference_at_s: None,
                last_try_at_s: None,
                videos: 0,
                id,
                ..channel(n)
            },
            decision,
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
            rating: None,
            set_aside_at_s: None,
            forever: ronoctua_video_metrics::read::Forever::No,
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
    pub(crate) fn readings() -> Readings {
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
            staged: vec![staged(3, Decision::Undecided), staged(4, Decision::Keep)],
            rules: Some((None, global_rules())),
            settings: Some(Settings::default()),
            weights: Some(Weights::default()),
            listed: Some(Listed {
                query: Query {
                    filter: Filter::default(),
                    page: 0,
                    per_page: crate::paging::PerPage::default().get(),
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

    /// The sentence of the question drawn between the row it was asked
    /// from and its no — each line's runs joined as drawn (a name is a run
    /// of its own, glued to its words), the lines by a space; the air reads
    /// as nothing.
    fn asked_between(rows: &[Row], from: usize, no: usize) -> String {
        rows[from + 1..no]
            .iter()
            .map(|x| x.fields.iter().map(|f| f.text.as_str()).collect::<String>())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
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
            Page::Import,
            // A STAGED CHANNEL'S PAGE, and its rules' page.
            Page::Channel(staged(3, Decision::Undecided).channel.id),
            Page::Rule(
                Some(staged(4, Decision::Keep).channel.id),
                Slot::Single(Single::Recency),
            ),
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
    /// wide door — in either language, in every layout of the videos — or the
    /// cursor is drawn on no row.
    #[test]
    fn every_page_draws_its_words_inside_the_door() {
        let mut r = readings();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for page in every_page() {
                for layout in Layout::ALL {
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
        // UNDER IT: a line of air, the sentence — at most three lines — and
        // the no.
        assert!(
            asked_from < no && no <= asked_from + 5,
            "the question is not under the row it was asked from: {:?}",
            spots(&rows)
        );
        assert!(no < row_of(&rows, "answer-yes"), "the no is not first");
        assert_eq!(v.cursor(&rows), no, "the cursor was not born on the no");
        assert_eq!(
            rows[no].fields[0].color,
            tokens::color::TEXT_HERE_ACTS,
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
        // A CLICK ON THE SECOND CELL, on a lower line of the shelf — on its
        // tile: the air before it is the first cell's ([`Cell::width`]).
        let second = shelves[0].cells[1].clone();
        assert!(
            !v.clicked(n + 2, second.from + TILE_AIR, &rows),
            "a first click acted"
        );
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

    /// FAILS IF: the first click on the "no" a question was born on gives
    /// it — that cursor is the screen's, not the person's (invariant 19,
    /// `aim::Aim`) — or the first click on its yes deletes; a click on the
    /// answer a key moved the cursor to waits for a second; or a click on
    /// the second line of an answer wrapped in a narrow door is not a click
    /// on it.
    #[test]
    fn the_first_click_on_a_born_no_only_picks_it() {
        let r = readings();
        let t = texts("en-GB");
        let id = VideoId("video000000".to_owned());
        let asked = || {
            let (mut v, _) = View::fresh(Page::Video(id.clone()));
            onto(&mut v, &r, "delete");
            assert_eq!(press(&mut v, &r, Press::Delete), Errand::Nothing);
            v
        };
        // THE BORN NO: picked by the first click, given by the second.
        let mut v = asked();
        let rows = v.rows(&r, COLUMNS, &t);
        let no = row_of(&rows, "answer-no");
        assert_eq!(v.click_answers(no, &rows), Some(Errand::Nothing));
        assert!(v.confirm.is_some(), "one click on the born no answered");
        assert_eq!(v.click_answers(no, &rows), Some(Errand::Nothing));
        assert!(v.confirm.is_none(), "the picked no did not answer");
        let rows = v.rows(&r, COLUMNS, &t);
        assert_eq!(rows[v.cursor(&rows)].spot, "delete");
        // THE YES: the first click only puts the cursor there.
        let mut v = asked();
        let rows = v.rows(&r, COLUMNS, &t);
        let yes = row_of(&rows, "answer-yes");
        assert_eq!(v.click_answers(yes, &rows), Some(Errand::Nothing));
        assert!(v.confirm.is_some(), "one click on yes deleted");
        let rows = v.rows(&r, COLUMNS, &t);
        assert_eq!(rows[v.cursor(&rows)].spot, "answer-yes");
        assert_eq!(
            v.click_answers(yes, &rows),
            Some(Errand::Edit(Edit::Delete(id.clone())))
        );
        // A KEY THAT MOVED THE CURSOR is the person's: one click answers.
        let mut v = asked();
        press(&mut v, &r, Press::Down);
        press(&mut v, &r, Press::Up);
        let rows = v.rows(&r, COLUMNS, &t);
        assert_eq!(
            v.click_answers(row_of(&rows, "answer-no"), &rows),
            Some(Errand::Nothing)
        );
        assert!(v.confirm.is_none(), "a keyed no waited for a second click");
        // A WRAPPED ANSWER is one answer on every line of it.
        const NARROW: u16 = 24;
        let mut v = asked();
        let rows = v.rows(&r, NARROW, &t);
        let yes = row_of(&rows, "answer-yes");
        assert!(
            rows[yes + 1].continues && !rows[yes + 1].fields.is_empty(),
            "the yes did not wrap at {NARROW} columns: {:?}",
            spots(&rows)
        );
        assert!(
            rows[yes..=yes + 1]
                .iter()
                .all(|x| x.fields.iter().all(|f| !f.text.contains('…'))),
            "an answer was cut"
        );
        assert_eq!(v.click_answers(yes + 1, &rows), Some(Errand::Nothing));
        assert_eq!(
            v.click_answers(yes + 1, &rows),
            Some(Errand::Edit(Edit::Delete(id.clone())))
        );
    }

    /// FAILS IF: a question on the page stops standing apart from the rows
    /// around it — a line of air off the ground before and after it, every
    /// line of the sentence and the answers on the question's ground, the
    /// sentence in the question's colour and the video's title in a name's,
    /// the answers set in by `space.view.nestCells` —, a title after it
    /// takes a second line of air, or its yes stops wearing what it costs:
    /// an error's colour to delete, a warning's to register, a setting's to
    /// hide; or the cursor is not the cursor's colour on it.
    #[test]
    fn a_question_stands_on_its_ground_with_air_around_it() {
        use crate::chrome::QUESTION_GROUND;
        let r = readings();
        let t = texts("en-GB");
        let id = VideoId("video000000".to_owned());
        let (mut v, _) = View::fresh(Page::Video(id));
        onto(&mut v, &r, "delete");
        press(&mut v, &r, Press::Delete);
        let rows = v.rows(&r, COLUMNS, &t);
        let (no, yes) = (row_of(&rows, "answer-no"), row_of(&rows, "answer-yes"));
        let first = rows
            .iter()
            .position(|x| x.ground.is_some())
            .expect("a ground");
        let last = (yes..rows.len())
            .take_while(|&k| rows[k].ground.is_some())
            .last()
            .expect("the yes on the ground");
        let before = &rows[first - 1];
        assert!(
            is_air(before) && before.fields.is_empty() && before.ground.is_none(),
            "no air before the question: {:?}",
            spots(&rows)
        );
        assert!(
            rows[first..=last]
                .iter()
                .all(|x| x.ground == Some(QUESTION_GROUND)),
            "a line of the block is off its ground"
        );
        let after = rows.get(last + 1).expect("air after the question");
        assert!(is_air(after) && after.ground.is_none(), "no air after");
        assert!(
            rows.get(last + 2).is_none_or(|x| !is_air(x)),
            "two lines of air after the question: {:?}",
            spots(&rows)
        );
        // THE SENTENCE, in the question's colour; the title in a name's.
        let fields: Vec<&Field> = rows[first..no].iter().flat_map(|x| &x.fields).collect();
        assert!(
            fields
                .iter()
                .any(|f| f.text == "A video, number 0"
                    && f.color == tokens::color::QUESTION_QUOTED),
            "the title is not in a name's ink: {fields:?}"
        );
        assert!(
            fields
                .iter()
                .all(|f| f.color == tokens::color::QUESTION_ASKED
                    || f.color == tokens::color::QUESTION_QUOTED),
            "the sentence is not in the question's colour"
        );
        assert_eq!(
            rows[no].fields[0].at,
            rows[first].fields[0].at + tokens::space::VIEW_NESTCELLS,
            "the answers are not set in"
        );
        assert_eq!(rows[no].fields[0].color, tokens::color::TEXT_HERE_ACTS);
        assert_eq!(rows[yes].fields[0].color, tokens::color::STATE_ERROR);
        // HIDING loses nothing.
        let mut v = View::fresh(Page::Video(VideoId("video000000".to_owned()))).0;
        onto(&mut v, &r, "hide");
        press(&mut v, &r, Press::Enter);
        let rows = v.rows(&r, COLUMNS, &t);
        assert!(matches!(
            v.confirm.as_ref().map(|c| &c.what),
            Some(Pending::Hide(..))
        ));
        assert_eq!(
            rows[row_of(&rows, "answer-yes")].fields[0].color,
            Kind::Setting.tones(false).0
        );
        // REGISTERING weighs.
        let (mut v, _) = View::fresh(Page::Import);
        onto(&mut v, &r, "register");
        press(&mut v, &r, Press::Enter);
        let rows = v.rows(&r, COLUMNS, &t);
        assert_eq!(
            rows[row_of(&rows, "answer-yes")].fields[0].color,
            tokens::color::STATE_WARNING
        );
    }

    /// FAILS IF: the description's question answers the first click on the
    /// "no" it was born on, or on its yes; stops being a block on the
    /// question's ground with its yes in an error's colour (it loses the
    /// text); or the door's cursor stops being found on the answer the
    /// question's cursor is on, whatever colour it is drawn in.
    #[test]
    fn the_description_s_discard_is_picked_before_it_answers() {
        use crate::chrome::QUESTION_GROUND;
        let r = readings();
        let t = texts("en-GB");
        let asking = || {
            let (mut v, _) = View::fresh(Page::Video(VideoId("video000000".to_owned())));
            onto(&mut v, &r, "second-description");
            press(&mut v, &r, Press::Enter);
            v.text_key(Key::simple(Sym::Char('x')), 60);
            v.text_key(Key::simple(Sym::Esc), 60);
            v
        };
        let mut v = asking();
        let rows = v.rows(&r, COLUMNS, &t);
        let no = row_of(&rows, "text-answer-false");
        let yes = row_of(&rows, "text-answer-true");
        assert_eq!(v.drawn_cursor(&rows), no, "the door does not follow the no");
        assert_eq!(rows[no].ground, Some(QUESTION_GROUND));
        assert_eq!(rows[yes].fields[0].color, tokens::color::STATE_ERROR);
        assert!(
            rows[no - 1].ground.is_some(),
            "the sentence is off the ground"
        );
        assert!(rows[yes + 1].ground.is_none() && rows[yes + 1].fields.is_empty());
        let on_no = text_hit(&rows, no, 0).expect("the no takes a click");
        assert!(!v.text_clicked(on_no));
        let rows = v.rows(&r, COLUMNS, &t);
        assert!(
            rows.iter().any(|x| x.spot == "text-answer-false"),
            "one click on the born no answered"
        );
        assert!(!v.text_clicked(on_no));
        assert!(v.editing(), "the no threw the text away");
        let rows = v.rows(&r, COLUMNS, &t);
        assert!(
            !rows.iter().any(|x| x.spot == "text-answer-false"),
            "the picked no did not answer"
        );
        // THE YES: picked, then given.
        let mut v = asking();
        let rows = v.rows(&r, COLUMNS, &t);
        let on_yes = text_hit(&rows, row_of(&rows, "text-answer-true"), 0).expect("yes");
        assert!(
            !v.text_clicked(on_yes),
            "one click on yes threw the text away"
        );
        let rows = v.rows(&r, COLUMNS, &t);
        assert_eq!(v.drawn_cursor(&rows), row_of(&rows, "text-answer-true"));
        assert!(v.text_clicked(on_yes), "the picked yes did not answer");
        assert!(!v.editing());
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
            l.total = u64::from(crate::paging::PerPage::default().get()) * 2 + 1;
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

    /// The fixture's readings, its list read as page `page` of `total`
    /// videos at `per` a page — its ten rows standing for the page.
    fn paged(page: u32, per: u32, total: u64) -> Readings {
        let mut r = readings();
        if let Some(l) = r.listed.as_mut() {
            l.query.page = page;
            l.query.per_page = per;
            l.total = total;
        }
        r
    }

    /// The Videos page open on page `page` of the list, its reading in.
    fn on_page(page: u32, total: u64) -> (View, Readings) {
        let (mut v, _) = View::fresh(Page::Videos);
        v.page = page;
        (
            v,
            paged(page, crate::paging::PerPage::default().get(), total),
        )
    }

    /// The arrows of a row: what each turns, and where it is drawn.
    fn turns_of(row: &Row) -> Vec<(Target, u16)> {
        row.controls
            .iter()
            .map(|c| (c.target.clone(), c.from))
            .collect()
    }

    /// FAILS IF: a page's size does not turn round 12, 16, 18, 20, 32, 40
    /// and 64 from 16, a new size is not what the feed is asked for, or the
    /// page it lands on does not hold the first video of the one that was on
    /// screen — or its title is not owed the top of the door.
    #[test]
    fn a_page_size_turns_in_a_ring_and_keeps_the_first_video() {
        let (mut v, r) = on_page(3, 230);
        assert_eq!(v.per_page.get(), 16);
        onto(&mut v, &r, "per-page");
        let Errand::Look(look) = press(&mut v, &r, Press::More) else {
            panic!("a new size asked nothing");
        };
        // THE 49TH VIDEO, the first of the fourth page of sixteen, is on the
        // third of eighteen.
        assert_eq!(look.query.map(|q| (q.page, q.per_page)), Some((2, 18)));
        assert!(v.top_owed, "a new size owed its title nothing");
        let rows = v.rows(&r, COLUMNS, &texts("en-GB"));
        let size = row_of(&rows, "per-page");
        assert!(
            rows[size].fields.iter().any(|f| f.text == "18"),
            "the row does not say the size"
        );
        press(&mut v, &r, Press::More);
        assert_eq!((v.per_page.get(), v.page), (20, 1));
        for size in [32, 40, 64, 12] {
            press(&mut v, &r, Press::More);
            assert_eq!((v.per_page.get(), v.page), (size, 0));
        }
        press(&mut v, &r, Press::More);
        assert_eq!(
            (v.per_page.get(), v.page),
            (16, 0),
            "the ring did not go round"
        );
        press(&mut v, &r, Press::Less);
        assert_eq!(v.per_page.get(), 12, "back, round the ring");
    }

    /// FAILS IF: the list's title stops saying its page beside it in the
    /// quiet ink, draws an arrow that cannot move — the first page's `‹`,
    /// the last's `›` — or lets the other slide into its place, or a list
    /// of one page draws a mark or an arrow, here or at its end.
    #[test]
    fn the_list_s_page_and_its_arrows_stop_at_the_ends() {
        let t = texts("en-GB");
        let title = |page: u32, total: u64| {
            let (v, r) = on_page(page, total);
            let rows = v.rows(&r, COLUMNS, &t);
            rows[row_of(&rows, crate::paging::LIST_TITLE)].clone()
        };
        let middle = title(1, 40);
        assert_eq!(middle.fields[0].text, t.t(key::VIDEO_METRICS_BY_SCORE));
        assert_eq!(middle.fields[0].color, tokens::color::TEXT_HEADING);
        let mark = middle
            .fields
            .iter()
            .find(|f| f.text == "2 of 3")
            .expect("the title's mark");
        assert_eq!(mark.color, tokens::color::TEXT_DIM);
        let both = turns_of(&middle);
        assert_eq!(
            both.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>(),
            [Target::PageTurn(false), Target::PageTurn(true)]
        );
        assert!(middle.takes(), "the arrows take no click");
        // THE FIRST PAGE: no `‹`, and the `›` where it was.
        assert_eq!(turns_of(&title(0, 40)), [both[1].clone()]);
        // THE LAST: no `›`, and the `‹` where it was.
        assert_eq!(turns_of(&title(2, 40)), [both[0].clone()]);
        // ONE PAGE: the title alone, and no arrows at the end.
        let (v, r) = on_page(0, 10);
        let rows = v.rows(&r, COLUMNS, &t);
        let alone = &rows[row_of(&rows, crate::paging::LIST_TITLE)];
        assert_eq!(alone.fields.len(), 1, "one page says a mark");
        assert!(alone.controls.is_empty(), "one page draws arrows");
        assert!(!spots(&rows).contains(&crate::paging::LIST_PAGER));
    }

    /// FAILS IF: the list does not end with its arrows and its mark — or
    /// they wear a title's ink, which would begin a section there
    /// (`section_ring`), or the cursor stops on them, or an arrow there does
    /// not turn the page at once and owe its title the top.
    #[test]
    fn the_list_ends_with_its_arrows_in_no_title_ink() {
        let t = texts("en-GB");
        let (mut v, r) = on_page(1, 40);
        let rows = v.rows(&r, COLUMNS, &t);
        let end = row_of(&rows, crate::paging::LIST_PAGER);
        let last_video = rows
            .iter()
            .rposition(|x| x.spot.starts_with("video:"))
            .expect("a video");
        assert!(end > last_video, "the arrows are not at the list's end");
        let pager = &rows[end];
        assert!(
            pager.heading && pager.takes(),
            "the cursor stops on the arrows"
        );
        assert!(
            pager
                .fields
                .iter()
                .all(|f| f.color != tokens::color::TEXT_HEADING),
            "the list's end wears a title's ink"
        );
        assert_eq!(pager.fields[0].text, "2 of 3");
        assert_eq!(turns_of(pager).len(), 2);
        let later = pager.controls[1].from;
        let errand = v.click_control(end, later, &rows, &r);
        assert!(
            matches!(
                errand,
                Some(Errand::Look(Look {
                    query: Some(Query { page: 2, .. }),
                    ..
                }))
            ),
            "the end's `›` did not turn: {errand:?}"
        );
        assert!(v.top_owed);
    }

    /// FAILS IF: a turned page does not put its title first, with the
    /// cursor on its first video — the list's row, with its picture or
    /// without, or the grid's first cell — or does so before the page asked
    /// is read (the page still drawn moving under the person), or more than
    /// once. In every layout.
    #[test]
    fn a_turned_page_puts_its_title_first_and_the_cursor_on_its_first_video() {
        let t = texts("en-GB");
        for layout in Layout::ALL {
            let (mut v, mut r) = on_page(0, 40);
            v.layout = layout;
            let entries = row_of(&v.rows(&r, COLUMNS, &t), crate::paging::LIST_TITLE);
            onto(&mut v, &r, "filter-shorts");
            press(&mut v, &r, Press::PageOn);
            assert_eq!(v.page, 1);
            // THE PAGE BEFORE IS STILL DRAWN, saying the page asked.
            let rows = v.rows(&r, COLUMNS, &t);
            assert_eq!(row_of(&rows, crate::paging::LIST_TITLE), entries);
            assert!(
                rows.iter()
                    .any(|x| x.fields.iter().any(|f| f.text == "2 of 3"))
            );
            assert_eq!(
                v.settle_on_list(&rows, &r),
                None,
                "placed before its reading"
            );
            assert_eq!(
                v.at, "filter-shorts",
                "the cursor left before the page came"
            );
            // ITS READING CAME.
            if let Some(l) = r.listed.as_mut() {
                l.query.page = 1;
            }
            let rows = v.rows(&r, COLUMNS, &t);
            let title = row_of(&rows, crate::paging::LIST_TITLE);
            assert_eq!(v.settle_on_list(&rows, &r), Some(title), "{layout:?}");
            let first = video_spot(&r.listed.as_ref().expect("a list").rows[0].id);
            assert_eq!(
                v.at, first,
                "{layout:?}: the cursor is not on the first video"
            );
            assert_eq!(v.cursor(&rows), title + 1, "{layout:?}");
            assert_eq!(v.settle_on_list(&rows, &r), None, "placed twice");
        }
    }

    /// FAILS IF: a turned page whose title is put first does not keep, under
    /// it, the room a page of the door's lines takes — so a short last
    /// page's title could not be first in either door (the owner's "2 of 2",
    /// 2026-11-11) — or keeps it from the lines told before and not now; or
    /// that room is anything but the air between sections: a line a click
    /// takes or the pointer lights, a stop of the cursor, a section's title;
    /// or the title is not kept first while the cursor's row is under it on
    /// the page, or is kept when it was not first; or a page never turned to
    /// is given room, or the room outlives the Videos page. In every layout.
    #[test]
    fn a_title_put_first_keeps_its_room_and_the_room_is_air() {
        let t = texts("en-GB");
        // A DOOR TALLER THAN THE SHORT PAGE in every layout: a strip's
        // pictures take lines.
        let lines = 80;
        for layout in Layout::ALL {
            // A SHORT PAGE NEVER TURNED TO is drawn as it was.
            let (mut v, r) = on_page(1, 27);
            v.layout = layout;
            v.page_is(lines);
            let own = v.rows(&r, COLUMNS, &t);
            let title = row_of(&own, crate::paging::LIST_TITLE);
            assert!(own.len() < title + lines, "{layout:?}: a page too long");
            assert_eq!(
                own.last().map(|x| x.spot.as_str()),
                Some(crate::paging::LIST_PAGER),
                "{layout:?}: a page never turned to was given room"
            );
            // TURNED TO, its title is put first, and keeps its room.
            let (mut v, mut r) = on_page(0, 27);
            v.layout = layout;
            v.page_is(lines);
            press(&mut v, &r, Press::PageOn);
            if let Some(l) = r.listed.as_mut() {
                l.query.page = 1;
            }
            let rows = v.rows(&r, COLUMNS, &t);
            assert_eq!(v.settle_on_list(&rows, &r), Some(title), "{layout:?}");
            let rows = v.rows(&r, COLUMNS, &t);
            assert_eq!(rows.len(), title + lines, "{layout:?}: no room");
            assert_eq!(rows[own.len() - 1].spot, crate::paging::LIST_PAGER);
            for (n, x) in rows.iter().enumerate().skip(own.len()) {
                assert!(x.fields.is_empty(), "{layout:?}: line {n} says something");
                assert!(!x.takes(), "{layout:?}: line {n} takes a click");
                assert!(!x.stops(), "{layout:?}: the cursor stops on line {n}");
                assert!(
                    !ronoctua_shell_kit::section_ring::is_title(&x.fields, x.takes()),
                    "{layout:?}: line {n} begins a section"
                );
            }
            // KEPT FIRST while the cursor's row is under it on the page.
            assert_eq!(v.title_kept(&rows, title, lines), Some(title));
            assert_eq!(v.title_kept(&rows, title - 1, lines), None, "not first");
            assert_eq!(v.title_kept(&rows, title, 1), None, "past the page");
            // A DOOR TALLER NOW keeps the page it has now.
            v.page_is(lines + 5);
            assert_eq!(v.rows(&r, COLUMNS, &t).len(), title + lines + 5);
            // AND THE ROOM IS THE PAGE'S: opened again, none.
            let _ = v.open(Page::Home);
            let _ = v.open(Page::Videos);
            assert_eq!(v.rows(&r, COLUMNS, &t).len(), own.len(), "{layout:?}");
        }
    }

    /// FAILS IF: `Home` and `End` stop going to the first and the last page
    /// of the list, `[` and `]` stop turning it — or any of them turns a
    /// page anywhere but on the videos.
    #[test]
    fn home_and_end_go_to_the_first_and_last_page() {
        let (mut v, r) = on_page(1, 40);
        assert_eq!(press_of_key(Key::simple(Sym::Home)), Some(Press::PageFirst));
        assert_eq!(press_of_key(Key::simple(Sym::End)), Some(Press::PageLast));
        assert_eq!(
            press_of_key(Key::simple(Sym::Char('['))),
            Some(Press::PageBack)
        );
        assert_eq!(
            press_of_key(Key::simple(Sym::Char(']'))),
            Some(Press::PageOn)
        );
        press(&mut v, &r, Press::PageLast);
        assert_eq!(v.page, 2);
        assert_eq!(
            press(&mut v, &r, Press::PageLast),
            Errand::Nothing,
            "past the last"
        );
        press(&mut v, &r, Press::PageFirst);
        assert_eq!(v.page, 0);
        let (mut home, r) = (View::fresh(Page::Home).0, readings());
        assert_eq!(press(&mut home, &r, Press::PageLast), Errand::Nothing);
    }

    /// FAILS IF: the page row's sentence stops teaching the keys that turn
    /// the page — each one as its door reaches it: the bare `[` in the
    /// footer, `LEADER Alt` and `[` in a window, the page keys alike in both
    /// — or a language leaves one of them unsaid.
    #[test]
    fn the_page_row_teaches_its_keys_in_each_door() {
        let (mut v, r) = on_page(0, 40);
        let footer = v.page_keys();
        assert_eq!(footer[0], ("earlier", "[".to_owned()));
        assert_eq!(footer[1], ("prior", "<prior>".to_owned()));
        assert_eq!(footer[2], ("later", "]".to_owned()));
        assert_eq!(footer[3], ("next", "<next>".to_owned()));
        assert_eq!(footer[4], ("first", "<home>".to_owned()));
        assert_eq!(footer[5], ("last", "<end>".to_owned()));
        // THE KEYS LEAD, IN FEW WORDS: the footer says the sentence on ONE
        // line, after the click's gesture, and cuts it at its end. A 1280-px
        // footer's line holds some 112 cells, and the gesture takes 65 of
        // them in English and 55 in Portuguese; keys said after a clause of
        // their own were cut there, `]` and `<end>` never shown.
        for (language, room) in [("en-GB", 47), ("pt-BR", 57)] {
            let t = texts(language);
            let rows = v.rows(&r, COLUMNS, &t);
            let page = row_of(&rows, "page");
            let said = v
                .explained_at(&rows, page, None, &t)
                .expect("the page row explained")
                .said[0]
                .0
                .clone();
            assert!(said.starts_with(footer[0].1.as_str()), "{language}: {said}");
            let last = footer[5].1.as_str();
            let end = said.find(last).expect("the last key said") + last.len();
            assert!(
                said[..end].chars().count() <= room,
                "{language}: the keys run past the footer's line: {said}"
            );
        }
        v.in_a_window();
        let window = v.page_keys();
        assert_eq!(window[0].1, crate::chrome::leader_alt_text("[", false));
        assert_eq!(window[2].1, crate::chrome::leader_alt_text("]", false));
        assert_eq!(
            window[1].1, "<prior>",
            "a page key is direct in a window too"
        );
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let rows = v.rows(&r, COLUMNS, &t);
            let page = row_of(&rows, "page");
            let said = v
                .explained_at(&rows, page, None, &t)
                .expect("the page row explained")
                .said[0]
                .0
                .clone();
            for (_, k) in &window {
                assert!(
                    said.contains(k.as_str()),
                    "{language}: `{k}` untaught: {said}"
                );
            }
            assert!(!unworded(&said), "{language}: {said}");
        }
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

    /// FAILS IF: the window's corner says its way back on the home page —
    /// where it says the window's name — or not on a page below it, opened
    /// as a person opens it (`RET` on the home page's row) or from outside;
    /// or a step back home leaves it saying the way back.
    #[test]
    fn the_way_back_is_said_only_below_the_home_page() {
        let r = readings();
        let (mut v, _) = View::fresh(Page::Home);
        assert!(!v.below_the_first(), "the home page says its way back");
        onto(&mut v, &r, "channels");
        assert!(matches!(press(&mut v, &r, Press::Enter), Errand::Look(_)));
        assert_eq!(v.page(), &Page::Channels);
        assert!(v.below_the_first(), "the channels say no way back");
        assert!(matches!(press(&mut v, &r, Press::Escape), Errand::Look(_)));
        assert!(
            !v.below_the_first(),
            "back home, it still says the way back"
        );
        let (v, _) = View::fresh(Page::Channels);
        assert!(v.below_the_first(), "opened on the channels, no way back");
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
    /// picture flush with its left edge reads as cut — or the tiles stand
    /// further apart than the player's (a picture and a gap, from the page's
    /// left edge on: the owner's window held three where the player's held
    /// four), or the air two cells share is not the first's, or a click past
    /// the last cell takes one.
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
        for line in &rows[shelf..shelf + usize::from(shelf_lines(Layout::Grid))] {
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
        let (left, _) = extent(100);
        assert_eq!(
            cells[0].from + TILE_AIR,
            left,
            "the first tile is not at the page's edge"
        );
        assert_eq!(
            cells[1].from - cells[0].from,
            tokens::space::VIDEOMETRICS_THUMBCELLS + ronoctua_design::GAP,
            "the tiles stand further apart than the player's"
        );
        let shared = cells[1].from;
        assert_eq!(shared + 1, cells[0].from + cells[0].width, "no air shared");
        assert!(!v.clicked(shelf, shared, &rows));
        assert_eq!(
            v.at, cells[0].spot,
            "the shared air is not the first cell's"
        );
        let last = cells.last().expect("a cell");
        let past = last.from + last.width;
        let at = v.at.clone();
        assert!(
            !v.clicked(shelf, past, &rows),
            "the air past the last cell acted"
        );
        assert_eq!(v.at, at, "the air past the last cell took the cursor");
    }

    /// The names a said sentence quotes, as they read in it.
    fn names_in(said: Option<&Said>) -> Vec<&str> {
        let (Some(Said::Normal(s)) | Some(Said::Long(s))) = said else {
            panic!("nothing said");
        };
        s.names().iter().map(|r| &s[r.clone()]).collect()
    }

    /// FAILS IF: what an errand came to stops telling the NAME it quotes —
    /// the channel added, the video queued or downloaded, its title when a
    /// mark changed, the file the channels were written to — as the stretch
    /// a door draws in a name's ink (`Arg::Name`), in either language; or a
    /// count beside a name stops picking its plural (it travels as a
    /// number), or the sentence stops being the one the key says.
    #[test]
    fn what_is_said_tells_the_names_it_quotes() {
        let r = readings();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let mut v = View::default();
            let channel = "Ancient Recipes of the Sea";
            let (said, _) = v.took(Done::Added(Ok(channel.to_owned())), &r, &t);
            assert_eq!(names_in(said.as_ref()), [channel], "{language}");
            let (said, _) = v.took(Done::Downloading("A title".to_owned(), true), &r, &t);
            assert_eq!(names_in(said.as_ref()), ["A title"], "{language}");
            let id = VideoId("video000000".to_owned());
            let (said, _) = v.took(Done::Edited(Edit::KeepVideo(id, true), Ok(())), &r, &t);
            assert_eq!(names_in(said.as_ref()), ["A video, number 0"], "{language}");
            let file = PathBuf::from("/home/someone/subscriptions.csv");
            let (said, _) = v.took(Done::Exported(Ok((3, file.clone()))), &r, &t);
            assert_eq!(
                names_in(said.as_ref()),
                ["/home/someone/subscriptions.csv"],
                "{language}"
            );
            let Some(Said::Normal(exported)) = said else {
                panic!("{language}: the export was not said");
            };
            assert_eq!(
                exported.as_str(),
                t.t_args(
                    key::VIDEO_METRICS_EXPORTED,
                    &[
                        ("count", Arg::Number(3)),
                        ("file", Arg::Text("/home/someone/subscriptions.csv")),
                    ],
                ),
                "{language}"
            );
        }
    }

    /// FAILS IF: the note under the consent to YouTube stops telling each
    /// host it lets be asked as a NAME of its own — the list told as one, the
    /// joiner inside a name, a host lost — in either language; or its text
    /// stops being the key's sentence with the hosts joined as before.
    #[test]
    fn the_youtube_note_tells_each_host_as_a_name() {
        let r = readings();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let mut items = Vec::new();
            home(&mut items, &r, &t);
            let note = items
                .iter()
                .find_map(|i| match i {
                    Item::Note(said, _) if said.contains(ronoctua_video_metrics::HOSTS[0]) => {
                        Some(said)
                    }
                    _ => None,
                })
                .expect("the consent's note");
            assert_eq!(
                note.as_str(),
                t.t_with(
                    key::VIDEO_METRICS_ASK_YOUTUBE_NOTE,
                    &[("hosts", &ronoctua_video_metrics::HOSTS.join(", "))],
                ),
                "{language}"
            );
            let names: Vec<&str> = note.names().iter().map(|r| &note[r.clone()]).collect();
            assert_eq!(names, ronoctua_video_metrics::HOSTS, "{language}");
        }
    }

    /// What a page is open with, for [`every_taking_row_is_explained`]: as
    /// it opens, with a question on it, with the description being edited,
    /// and with the edit's own question up.
    #[derive(Debug, Clone, Copy)]
    enum Open {
        Plain,
        Question,
        Editing,
        EditingAsked,
    }

    /// Every piece of what a door says of a thing, for a test to read:
    /// its name, its gesture and its sentences.
    fn pieces(e: &crate::door::Explained) -> Vec<&str> {
        let mut out = vec![e.name.as_str()];
        out.extend(e.gesture.as_deref());
        out.extend(e.said.iter().map(|(s, _)| s.as_str()));
        out
    }

    /// Whether `said` holds a KEY WITH NO WORDS — `[video-metrics-x]`, what
    /// a key the texts lack is drawn as — or an argument left unfilled
    /// (`{ $x }`). A `[` alone is the page's quick key, which the page row's
    /// sentence teaches ([`View::page_keys`]).
    fn unworded(said: &str) -> bool {
        said.contains('{')
            || said.match_indices('[').any(|(i, _)| {
                said[i + 1..].split(']').next().is_some_and(|k| {
                    k.len() > 1 && k.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                })
            })
    }

    /// FAILS IF: a row a click takes — on any page, in every layout of the
    /// videos, at a narrow, a middling and a wide door, in either language,
    /// with a question on the page or the description being edited — has
    /// nothing to say where it is explained (the window's foot, the footer's
    /// line): no name, no sentence, a key with no words (`[key]`); or a video
    /// of a shelf, or a glyph, is not explained at the cell under the
    /// pointer; or a row no click takes says something; or, under a
    /// question, a row that is not one of its answers promises what a second
    /// click does there — it would give the question its answer instead.
    ///
    /// IN EVERY STATE A VIDEO, A CHANNEL OR THE CATALOGUE IS SAID IN: as the
    /// fixture reads; then marked — a rating, set aside, kept by its own
    /// mark or by its channel's, a channel keeping days of its own and one
    /// keeping every video, a video with no date and one whose channel was
    /// not read, the set-aside videos alone and newest first —; then with
    /// no video deleted everywhere. So every arrow
    /// lit or not, every bookmark, every ∞ ([`Lasting`]) and what holds once
    /// it is off ([`Control::follows`]), and every quiet default of a video's
    /// or a channel's page are said whole.
    #[test]
    fn every_taking_row_is_explained() {
        let id = VideoId("video000000".to_owned());
        let (a, b) = (channel(1), channel(2));
        let mut pages = every_page();
        pages.push(Page::Rule(Some(a.id), Slot::Id(RuleId(3))));
        pages.push(Page::Channel(b.id.clone()));
        pages.extend([2, 3, 5, 6, 7, 9].map(|n| Page::Video(VideoId(format!("video{n:06}")))));
        let marked = {
            let mut r = readings();
            if let Some(l) = r.listed.as_mut() {
                let rows = &mut l.rows;
                rows[1].rating = Some(Rating::Positive);
                rows[2].set_aside_at_s = Some(NOW - 60);
                rows[3].forever = Forever::ByVideo;
                rows[4].forever = Forever::ByChannel;
                for x in &mut rows[5..8] {
                    x.channel_id = b.id.clone();
                    x.channel = b.title.clone();
                }
                rows[6].set_aside_at_s = Some(NOW - 120);
                rows[7].forever = Forever::ByVideo;
                rows[7].set_aside_at_s = Some(NOW - 180);
                rows[8].published_at_s = None;
                rows[9].channel_id = ChannelId("UCnot-read".to_owned());
            }
            r.channels[0].keep = Some(Keep::Days(7));
            r.channels[1].keep = Some(Keep::Forever);
            r.set_aside_count = Some(3);
            r.expiring = Some(5);
            r
        };
        let never = {
            let mut r = marked.clone();
            r.channels[0].keep = None;
            r.settings = Some(Settings {
                keep: Keep::Forever,
                ..Settings::default()
            });
            r
        };
        let mut explained = 0usize;
        let mut followed = 0usize;
        for (fixture, mut r, varied) in [
            ("as read", readings(), false),
            ("marked", marked, true),
            ("never deleted everywhere", never, true),
        ] {
            for language in ["en-GB", "pt-BR"] {
                let t = texts(language);
                for page in &pages {
                    // A CHANNEL'S PAGE reads its own scope's rules — the ones
                    // everywhere, which it follows.
                    r.rules = Some(match page {
                        Page::Channel(c) | Page::Rule(Some(c), _) => {
                            (Some(c.clone()), global_rules())
                        }
                        _ => (None, global_rules()),
                    });
                    // A VIDEO'S PAGE reads that video.
                    if let Page::Video(vid) = page {
                        let row = r
                            .listed
                            .as_ref()
                            .and_then(|l| l.rows.iter().find(|x| &x.id == vid))
                            .cloned();
                        r.video = row.map(|x| (vid.clone(), Some(whole(x))));
                    }
                    // EVERY LAYOUT — the channel under a thumbnail among them.
                    for layout in Layout::ALL {
                        for open in [
                            Open::Plain,
                            Open::Question,
                            Open::Editing,
                            Open::EditingAsked,
                        ] {
                            let (mut v, _) = View::fresh(page.clone());
                            v.layout = layout;
                            // MARKED: the list is the set-aside videos alone,
                            // newest first.
                            if varied {
                                if fixture == "marked" {
                                    v.filter.set_aside = SetAside::Only;
                                    v.filter.order = Order::NewestSetAside;
                                }
                                // AND A LIST IMPORTED, what it left out and
                                // found here already said on its page.
                                v.imported = Some(imported());
                            }
                            match open {
                                Open::Plain => {}
                                Open::Question => {
                                    v.asks(Pending::Delete(id.clone(), "A video".to_owned()));
                                }
                                Open::Editing | Open::EditingAsked => {
                                    v.editing = Some((
                                        id.clone(),
                                        crate::text_area::TextArea::new(
                                            DESCRIPTION,
                                            "Mine\nand more",
                                        ),
                                    ));
                                    if matches!(open, Open::EditingAsked) {
                                        v.text_key(Key::simple(Sym::Char('x')), 60);
                                        v.text_key(Key::simple(Sym::Esc), 60);
                                    }
                                }
                            }
                            for columns in [44, 80, 200] {
                                let rows = v.rows(&r, columns, &t);
                                let takes = v.takes(&rows);
                                let what = format!(
                                    "{fixture}: {language} {page:?} {layout:?} {open:?} at {columns}"
                                );
                                for (n, row) in rows.iter().enumerate() {
                                    for f in &row.fields {
                                        assert!(!f.text.contains('['), "{what}: `{}`", f.text);
                                    }
                                    if !takes[n] {
                                        assert_eq!(
                                            v.explained_at(&rows, n, None, &t),
                                            None,
                                            "{what}: row {n} ({}) takes no click and says something",
                                            row.spot
                                        );
                                        continue;
                                    }
                                    let owner = &rows[Row::owner(&rows, n)];
                                    // WHERE THE POINTER IS ACROSS THE LINE says
                                    // which thing: every glyph of a line — a
                                    // shelf's lines of glyphs among them —, and
                                    // every cell of a shelf's other lines.
                                    let at: Vec<Option<u16>> = if !row.controls.is_empty() {
                                        row.controls.iter().map(|c| Some(c.from)).collect()
                                    } else if !owner.cells.is_empty() {
                                        owner.cells.iter().map(|c| Some(c.from + 1)).collect()
                                    } else {
                                        assert!(
                                            v.about(&rows, n, &t).is_some(),
                                            "{what}: row {n} ({}) has no sentence",
                                            row.spot
                                        );
                                        vec![None]
                                    };
                                    for column in at {
                                        let e = v.explained_at(&rows, n, column, &t).unwrap_or_else(|| {
                                        panic!("{what}: row {n} ({}) at {column:?} is not explained", row.spot)
                                    });
                                        assert!(
                                            !e.name.is_empty(),
                                            "{what}: row {n} ({}) has no name",
                                            row.spot
                                        );
                                        assert!(
                                            e.said.iter().any(|(s, _)| !s.is_empty()),
                                            "{what}: row {n} ({}) says nothing",
                                            row.spot
                                        );
                                        for p in pieces(&e) {
                                            assert!(!unworded(p), "{what}: row {n}: `{p}`");
                                        }
                                        // A GLYPH says its own sentence, and what
                                        // holds once it is off where it says it.
                                        if let Some(c) = column
                                            .and_then(|x| row.controls.iter().find(|c| c.from == x))
                                        {
                                            let said: Vec<&str> =
                                                e.said.iter().map(|(s, _)| s.as_str()).collect();
                                            assert_eq!(said[0], t.t(c.about), "{what}: row {n}");
                                            assert_eq!(
                                                said.get(1).copied(),
                                                c.follows.as_deref(),
                                                "{what}: row {n}"
                                            );
                                            followed += usize::from(c.follows.is_some());
                                        }
                                        // UNDER A QUESTION only its answers
                                        // promise a click — and a glyph, which
                                        // lets the question go and acts.
                                        if matches!(open, Open::Question) {
                                            assert_eq!(
                                                e.gesture.is_some(),
                                                matches!(owner.target, Target::Answer(_))
                                                    || !row.controls.is_empty(),
                                                "{what}: row {n} ({}) under a question",
                                                row.spot
                                            );
                                        }
                                        explained += 1;
                                    }
                                }
                                // AND THE CURSOR'S THING, whatever the pointer does.
                                if let Some(e) = v.explained_cursor(&rows, &t) {
                                    assert!(
                                        !e.name.is_empty(),
                                        "{what}: the cursor's thing has no name"
                                    );
                                    for p in pieces(&e) {
                                        assert!(!unworded(p), "{what}: the cursor's: `{p}`");
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(explained > 1000, "only {explained} things explained");
        assert!(followed > 0, "no glyph said what holds once it is off");
    }

    /// FAILS IF: on a line with glyphs, the cells the pointer lights, the
    /// ones the foot explains and the ones a click takes differ anywhere —
    /// the cell of air before the first glyph, the margin before the words,
    /// the words of a line that continues a row (a list entry's meta line),
    /// a line of glyphs alone; or a click on a glyph or the air selects the
    /// row. ONE ANSWER FOR THE THREE, before any row carries glyphs.
    #[test]
    fn the_words_beside_glyphs_are_lit_explained_and_taken_alike() {
        let r = readings();
        let t = texts("en-GB");
        let (v, _) = View::fresh(Page::Videos);
        let mut rows = v.rows(&r, COLUMNS, &t);
        let (left, _) = extent(COLUMNS);
        let entry = rows
            .iter()
            .enumerate()
            .position(|(n, row)| row.stops() && rows.get(n + 1).is_some_and(|r| r.continues))
            .expect("a row of several lines");
        let title = rows
            .iter()
            .position(|row| row.heading && !row.continues && row.cells.is_empty())
            .expect("a title");
        let glyph = |from: u16| Control {
            from,
            width: 1,
            glyph: 'x',
            target: Target::Nothing,
            name: "a glyph".to_owned(),
            about: key::ABOUT_VIDEO_METRICS_CHANNELS,
            follows: None,
            lit: false,
        };
        for n in [entry, entry + 1, title] {
            rows[n].controls = vec![glyph(40), glyph(42)];
        }
        let mut words = 0;
        for n in [entry, entry + 1, title] {
            let owner_stops = rows[Row::owner(&rows, n)].stops();
            let lit = rows[n].lit_cells(left, owner_stops);
            for column in 0..COLUMNS {
                let what = format!("line {n} ({}) at {column}", rows[n].spot);
                let lights = lit
                    .iter()
                    .any(|&(from, width)| crate::chrome::in_cell(column, from, width));
                let on_a_glyph = column == 40 || column == 42;
                assert_eq!(
                    v.explained_at(&rows, n, Some(column), &t).is_some(),
                    lights,
                    "{what}: lit and explained differ"
                );
                let mut clicked = v.clone();
                clicked.at = "elsewhere".to_owned();
                clicked.clicked(n, column, &rows);
                assert_eq!(
                    clicked.at != "elsewhere",
                    lights && !on_a_glyph,
                    "{what}: lit and taken differ"
                );
                if lights && !on_a_glyph {
                    words += 1;
                    assert!(column >= left && column < 39, "{what}: past the words");
                }
            }
        }
        // THE WORDS OF THE ROW AND OF THE LINE THAT CONTINUES IT, and none of
        // the title's.
        assert_eq!(words, 2 * (39 - left), "the words' cells");
    }

    /// FAILS IF: a row set "as everywhere" does not say, quietly after its
    /// value, what everywhere is now — the channel's Shorts, a rule of a
    /// channel's page that is the one everywhere —, says it in any ink but
    /// the quietest (a state's tone, the cursor's), says it before the
    /// value, reaches past a narrow door, or says one nobody read; or what
    /// it follows is not said where the row is explained.
    #[test]
    fn as_everywhere_shows_what_it_is_now() {
        let mut r = readings();
        let a = channel(1).id;
        r.rules = Some((Some(a.clone()), global_rules()));
        r.settings = Some(Settings {
            block_shorts: true,
            ..Settings::default()
        });
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let blocked = t.t_with(
                key::VIDEO_METRICS_DEFAULT_NOW,
                &[("value", &t.t(key::VIDEO_METRICS_SHORTS_BLOCKED))],
            );
            let everywhere = t.t_with(
                key::VIDEO_METRICS_DEFAULT_NOW,
                &[("value", &t.t(key::VIDEO_METRICS_SCOPE_EVERYWHERE))],
            );
            for focused in [false, true] {
                let (mut v, _) = View::fresh(Page::Channel(a.clone()));
                if focused {
                    v.at = "shorts".to_owned();
                }
                for columns in [44, 80, 200] {
                    let rows = v.rows(&r, columns, &t);
                    let what = format!("{language} at {columns}, focused {focused}");
                    let n = row_of(&rows, "shorts");
                    let lines = &rows[n..n + Row::lines_of(&rows, n)];
                    let fields: Vec<&Field> = lines.iter().flat_map(|l| &l.fields).collect();
                    let value = fields
                        .iter()
                        .position(|f| f.text == t.t(key::VIDEO_METRICS_AS_EVERYWHERE))
                        .unwrap_or_else(|| panic!("{what}: no “as everywhere”: {fields:?}"));
                    let beside = fields
                        .iter()
                        .position(|f| f.text == blocked)
                        .unwrap_or_else(|| {
                            panic!("{what}: what everywhere is, unsaid: {fields:?}")
                        });
                    assert!(beside > value, "{what}: said before the value");
                    assert_eq!(fields[beside].color, tokens::color::TEXT_DIM, "{what}");
                    for f in &fields {
                        let end = f.at + ronoctua_render::layout::width(&f.text);
                        assert!(end <= columns, "{what}: `{}` ends at {end}", f.text);
                    }
                    // AND WHERE THE ROW IS EXPLAINED, quietly, under its sentence.
                    let e = v
                        .explained_at(&rows, n, None, &t)
                        .expect("the row explained");
                    let (said, ink) = e.said.last().expect("a sentence");
                    assert!(
                        said.contains(&t.t(key::VIDEO_METRICS_SHORTS_BLOCKED)),
                        "{what}: {said}"
                    );
                    assert_eq!(*ink, tokens::color::TEXT_DIM, "{what}");
                    // A RULE OF THE PAGE EVERYWHERE: its points, and apart
                    // from them, quietly, that it is everywhere's.
                    let n = row_of(&rows, "single:recency");
                    let fields: Vec<&Field> = rows[n..n + Row::lines_of(&rows, n)]
                        .iter()
                        .flat_map(|l| &l.fields)
                        .collect();
                    let scope = fields
                        .iter()
                        .find(|f| f.text == everywhere)
                        .unwrap_or_else(|| panic!("{what}: the rule's scope, unsaid: {fields:?}"));
                    assert_eq!(scope.color, tokens::color::TEXT_DIM, "{what}");
                    // AND RIGHT AFTER THE VALUE, nor two pieces on one cell:
                    // under a wrapped label ("Penalidade de / Short" at 44)
                    // it takes the label's free value column, and not a line
                    // of its own below the label's rest.
                    for spot in ["shorts", "single:recency", "single:short"] {
                        let n = row_of(&rows, spot);
                        let lines = &rows[n..n + Row::lines_of(&rows, n)];
                        let column = lines[0].fields.get(1).map_or(0, |f| f.at);
                        let value = lines.iter().rposition(|l| {
                            l.fields
                                .iter()
                                .any(|f| f.at >= column && f.color != tokens::color::TEXT_DIM)
                        });
                        let quiet = lines.iter().position(|l| {
                            l.fields.iter().any(|f| f.color == tokens::color::TEXT_DIM)
                        });
                        assert!(
                            matches!((value, quiet), (Some(v), Some(q)) if q <= v + 1),
                            "{what}: `{spot}` says what it follows away from its value: {lines:?}"
                        );
                        for line in lines {
                            let mut cells: Vec<(u16, u16)> = line
                                .fields
                                .iter()
                                .map(|f| (f.at, f.at + ronoctua_render::layout::width(&f.text)))
                                .collect();
                            cells.sort_unstable();
                            assert!(
                                cells.windows(2).all(|w| w[0].1 <= w[1].0),
                                "{what}: `{spot}` draws two pieces on one cell: {cells:?}"
                            );
                        }
                    }
                }
            }
            // WHAT NOBODY READ IS NOT SAID: the switches unread, the row
            // says "as everywhere" alone — and a channel with its own says
            // its own, and follows nothing.
            let mut unread = r.clone();
            unread.settings = None;
            let (v, _) = View::fresh(Page::Channel(a.clone()));
            let rows = v.rows(&unread, COLUMNS, &t);
            assert_eq!(rows[row_of(&rows, "shorts")].default, None, "{language}");
            let mut own = r.clone();
            own.channels[0].block_shorts = Some(false);
            let rows = v.rows(&own, COLUMNS, &t);
            assert_eq!(rows[row_of(&rows, "shorts")].default, None, "{language}");
        }
        // A STATE'S TONE NEVER REACHES IT: the value says the state, what it
        // follows stays quiet — with the cursor on the row and without.
        for focused in [false, true] {
            let item = Item::Pair(Pair {
                spot: "toned".to_owned(),
                label: "Label".to_owned(),
                value: "value".to_owned(),
                kind: Kind::Setting,
                target: Target::Nothing,
                tone: Some(tokens::color::STATE_ERROR),
                about: None,
                controls: Vec::new(),
                default: Some(Follows {
                    beside: "(beside)".to_owned(),
                    said: "said".to_owned(),
                }),
            });
            let rows = lay_out(vec![item], 1, 42, if focused { "toned" } else { "" });
            let fields: Vec<&Field> = rows.iter().flat_map(|l| &l.fields).collect();
            let value = fields
                .iter()
                .find(|f| f.text == "value")
                .expect("the value");
            let beside = fields
                .iter()
                .find(|f| f.text == "(beside)")
                .expect("beside");
            assert_eq!(
                value.color == tokens::color::STATE_ERROR,
                !focused,
                "the state's tone"
            );
            assert_eq!(beside.color, tokens::color::TEXT_DIM, "focused {focused}");
            assert_eq!(beside.at, value.at + 6, "not a cell of air after the value");
        }
    }

    /// Video `id` of the list's reading, changed by `change`.
    fn with_video(r: &mut Readings, id: &VideoId, change: impl FnOnce(&mut RankedVideo)) {
        let v = r
            .listed
            .as_mut()
            .and_then(|l| l.rows.iter_mut().find(|v| &v.id == id))
            .expect("the video in the list");
        change(v);
    }

    /// The line, and the glyph on it, whose target `is` says.
    fn glyph_at(rows: &[Row], is: &dyn Fn(&Target) -> bool) -> (usize, Control) {
        rows.iter()
            .enumerate()
            .find_map(|(n, r)| {
                r.controls
                    .iter()
                    .find(|c| is(&c.target))
                    .map(|c| (n, c.clone()))
            })
            .unwrap_or_else(|| panic!("no such glyph among {:?}", spots(rows)))
    }

    /// The glyph `is` says clicked once, on the rows as drawn.
    fn click_glyph(v: &mut View, r: &Readings, is: &dyn Fn(&Target) -> bool) -> Errand {
        let rows = v.rows(r, COLUMNS, &texts("en-GB"));
        let (n, c) = glyph_at(&rows, is);
        v.click_control(n, c.from, &rows, r)
            .expect("a click on a glyph does what it says")
    }

    /// FAILS IF: a glyph beside a video — an arrow of its rating, its
    /// bookmark, its ∞, its eye, its bin — waits for a second click, does
    /// anything but what its key does, leaves the cursor away from the video
    /// it acted on, or takes the air beside it; the arrow a video has does
    /// not take the rating away; the ∞ switches anything but the video's own
    /// mark (Reconciliation 4: also where its channel keeps it); or the bin
    /// and the eye do not ask first, "no" first, under the video — in every
    /// layout: the list, with its thumbnails or without, and the grid.
    #[test]
    fn a_glyph_acts_on_the_first_click() {
        let t = texts("en-GB");
        let id = VideoId("video000002".to_owned());
        for layout in Layout::ALL {
            let fresh = || {
                let (mut v, _) = View::fresh(Page::Videos);
                v.layout = layout;
                v
            };
            let mine = |x: &VideoId| *x == id;
            let mut r = readings();
            // AN ARROW GIVES ITS RATING at once — and the cursor is on the
            // video it rated.
            let mut v = fresh();
            assert_eq!(
                click_glyph(
                    &mut v,
                    &r,
                    &|x| matches!(x, Target::Rate(v, Rating::Positive, _) if mine(v))
                ),
                Errand::Edit(Edit::Rating(id.clone(), Some(Rating::Positive))),
                "{layout:?}"
            );
            assert_eq!(v.at, video_spot(&id), "{layout:?}: the cursor stayed away");
            // THE ONE IT HAS takes it away.
            with_video(&mut r, &id, |x| x.rating = Some(Rating::Positive));
            assert_eq!(
                click_glyph(
                    &mut fresh(),
                    &r,
                    &|x| matches!(x, Target::Rate(v, Rating::Positive, true) if mine(v))
                ),
                Errand::Edit(Edit::Rating(id.clone(), None)),
                "{layout:?}"
            );
            // THE BOOKMARK AND THE ∞ switch, each its own.
            for (aside, then) in [(None, true), (Some(NOW), false)] {
                with_video(&mut r, &id, |x| x.set_aside_at_s = aside);
                assert_eq!(
                    click_glyph(
                        &mut fresh(),
                        &r,
                        &|x| matches!(x, Target::SetAside(v, _) if mine(v))
                    ),
                    Errand::Edit(Edit::SetAside(id.clone(), then)),
                    "{layout:?}"
                );
            }
            for (forever, then) in [
                (Forever::No, true),
                (Forever::ByChannel, true),
                (Forever::ByVideo, false),
            ] {
                with_video(&mut r, &id, |x| x.forever = forever);
                assert_eq!(
                    click_glyph(
                        &mut fresh(),
                        &r,
                        &|x| matches!(x, Target::VideoKeep(v, _) if mine(v))
                    ),
                    Errand::Edit(Edit::KeepVideo(id.clone(), then)),
                    "{layout:?} {forever:?}"
                );
            }
            // THE BIN AND THE EYE ASK, "no" first, under the video.
            for is_bin in [true, false] {
                let mut v = fresh();
                let asked = click_glyph(&mut v, &r, &|x| match x {
                    Target::Delete(v) => is_bin && mine(v),
                    Target::Hide(v, _) => !is_bin && mine(v),
                    _ => false,
                });
                assert_eq!(asked, Errand::Nothing, "{layout:?}");
                let c = v.confirm.clone().expect("a question");
                assert_eq!(
                    c.under,
                    video_spot(&id),
                    "{layout:?}: asked away from the video"
                );
                assert!(
                    matches!(c.what, Pending::Delete(..) | Pending::Hide(..)),
                    "{:?}",
                    c.what
                );
                let rows = v.rows(&r, COLUMNS, &t);
                let no = row_of(&rows, "answer-no");
                assert_eq!(
                    v.cursor(&rows),
                    no,
                    "{layout:?}: the cursor is not on the no"
                );
                assert!(no < row_of(&rows, "answer-yes"));
                // A GLYPH UNDER THE QUESTION lets it go, and acts.
                let rated = click_glyph(
                    &mut v,
                    &r,
                    &|x| matches!(x, Target::Rate(v, Rating::Negative, _) if mine(v)),
                );
                assert_eq!(
                    rated,
                    Errand::Edit(Edit::Rating(id.clone(), Some(Rating::Negative)))
                );
                assert!(v.confirm.is_none(), "{layout:?}: the question stayed");
            }
            // THE AIR BESIDE A GLYPH is nobody's.
            let mut v = fresh();
            let rows = v.rows(&r, COLUMNS, &t);
            let (n, c) = glyph_at(&rows, &|x| matches!(x, Target::Delete(v) if mine(v)));
            assert_eq!(v.click_control(n, c.from + c.width, &rows, &r), None);
            assert_eq!(v.click_control(n, c.from - 1, &rows, &r), None);
        }
    }

    /// FAILS IF: a tile's two lines of glyphs leave the tile's box, draw two
    /// things on one cell, cut the score, lose a glyph, or do not end at the
    /// box's right edge — for a score of any length and an age of any — or
    /// a list's line under its title reaches past the door, cuts its score,
    /// or does not end its marks at the door's right edge.
    #[test]
    fn a_tile_holds_its_glyphs() {
        use ronoctua_render::layout::width;
        let box_cells = tokens::space::VIDEOMETRICS_THUMBCELLS;
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for score in [-99_999, 10_000, 123_456] {
                for days in [0, 1, 365, 1000] {
                    let mut r = readings();
                    if let Some(l) = r.listed.as_mut() {
                        for v in &mut l.rows {
                            v.score = score;
                            v.published_at_s = Some(NOW - days * 86_400);
                        }
                    }
                    let number = words::score_number(score, &t);
                    let what = format!("{language}, {score}, {days} days");
                    // THE GRID.
                    let (mut v, _) = View::fresh(Page::Videos);
                    v.layout = Layout::Grid;
                    let rows = v.rows(&r, 100, &t);
                    let shelf = rows
                        .iter()
                        .position(|x| !x.cells.is_empty())
                        .expect("a shelf");
                    let lines = &rows[shelf..shelf + usize::from(shelf_lines(Layout::Grid))];
                    let glyphs: Vec<&Row> =
                        lines.iter().filter(|l| !l.controls.is_empty()).collect();
                    assert_eq!(glyphs.len(), 2, "{what}: the score's line and the age's");
                    for (k, line) in glyphs.iter().enumerate() {
                        for cell in &rows[shelf].cells {
                            let inside = cell.from + TILE_AIR;
                            let end = inside + box_cells;
                            let mut drawn: Vec<(u16, u16)> = line
                                .fields
                                .iter()
                                .filter(|f| crate::chrome::in_cell(f.at, cell.from, cell.width))
                                .map(|f| (f.at, f.at + width(&f.text)))
                                .collect();
                            drawn.sort_unstable();
                            for &(a, b) in &drawn {
                                assert!(
                                    a >= inside && b <= end,
                                    "{what}: {a}..{b} out of {inside}..{end}"
                                );
                            }
                            assert!(
                                drawn.windows(2).all(|w| w[0].1 <= w[1].0),
                                "{what}: two things on one cell: {drawn:?}"
                            );
                            let mine: Vec<&Control> = line
                                .controls
                                .iter()
                                .filter(|c| crate::chrome::in_cell(c.from, cell.from, cell.width))
                                .collect();
                            assert_eq!(mine.len(), 4, "{what}: a glyph lost on line {k}");
                            if k == 0 {
                                assert!(
                                    line.fields
                                        .iter()
                                        .any(|f| f.at == inside && f.text == number),
                                    "{what}: the score cut"
                                );
                            } else {
                                let last = mine.iter().map(|c| c.from + c.width).max();
                                assert_eq!(last, Some(end), "{what}: the marks end inside the box");
                            }
                        }
                    }
                    // THE LIST, at a narrow door and a wide one.
                    for columns in [44, COLUMNS] {
                        let (v, _) = View::fresh(Page::Videos);
                        let rows = v.rows(&r, columns, &t);
                        let (left, room) = extent(columns);
                        let entry = row_of(&rows, "video:video000001");
                        let line = &rows[entry + Row::lines_of(&rows, entry) - 1];
                        assert_eq!(line.controls.len(), 8, "{what} at {columns}: glyphs lost");
                        assert!(
                            line.fields
                                .iter()
                                .any(|f| f.at == left && f.text.ends_with(&number)),
                            "{what}: the score cut"
                        );
                        for f in &line.fields {
                            assert!(
                                f.at >= left && f.at + width(&f.text) <= left + room,
                                "{what} at {columns}: `{}`",
                                f.text
                            );
                        }
                        let last = line.controls.iter().map(|c| c.from + c.width).max();
                        assert_eq!(
                            last,
                            Some(left + room),
                            "{what} at {columns}: the marks are not at the edge"
                        );
                    }
                }
            }
        }
    }

    /// FAILS IF: what keeps a video, or when it goes, is decided in another
    /// order than the catalogue's — its own mark, its channel's "for ever",
    /// being set aside, the days of its channel and then everywhere's,
    /// counted from when it was published, or first seen — or from what
    /// nobody read; the ∞ is lit for anything but the two "for ever"s (a
    /// set-aside video, everywhere's "never"); or the video's page does not
    /// say, quietly beside "Never delete it", what keeps it or when it goes
    /// without the mark.
    #[test]
    fn lasting_follows_the_video_then_the_channel_then_everywhere() {
        let t = texts("en-GB");
        let day = 86_400;
        let mut r = readings();
        let a = r.channels[0].clone();
        let mut v = ranked(3, &a);
        let published = NOW - 3 * day;
        // EVERYWHERE'S DAYS, from when it was published.
        assert_eq!(
            lasting(&v, None, &r),
            Lasting::Goes(Some(published + 90 * day))
        );
        // ITS CHANNEL'S OWN DAYS before them, and the first sighting for a
        // video with no date.
        r.channels[0].keep = Some(Keep::Days(7));
        assert_eq!(
            lasting(&v, None, &r),
            Lasting::Goes(Some(published + 7 * day))
        );
        v.published_at_s = None;
        assert_eq!(
            lasting(&v, Some(NOW), &r),
            Lasting::Goes(Some(NOW + 7 * day))
        );
        assert_eq!(lasting(&v, None, &r), Lasting::Goes(None));
        v.published_at_s = Some(published);
        // SET ASIDE keeps it while it is — before any days.
        v.set_aside_at_s = Some(NOW);
        assert_eq!(lasting(&v, None, &r), Lasting::SetAside);
        // ITS CHANNEL'S "FOR EVER", then its own mark, before being set aside.
        r.channels[0].keep = Some(Keep::Forever);
        assert_eq!(lasting(&v, None, &r), Lasting::ByChannel);
        v.forever = Forever::ByChannel;
        assert_eq!(lasting(&v, None, &r), Lasting::ByChannel);
        v.forever = Forever::ByVideo;
        assert_eq!(lasting(&v, None, &r), Lasting::ByVideo);
        // EVERYWHERE'S "NEVER", for a channel as everywhere.
        let mut w = ranked(4, &a);
        r.channels[0].keep = None;
        r.settings = Some(Settings {
            keep: Keep::Forever,
            ..Settings::default()
        });
        assert_eq!(lasting(&w, None, &r), Lasting::Everywhere);
        // NOTHING UNREAD IS TAKEN FOR A DEFAULT: the switches, the channel.
        r.settings = None;
        assert_eq!(lasting(&w, None, &r), Lasting::Goes(None));
        r.settings = Some(Settings::default());
        r.channels.clear();
        assert_eq!(lasting(&w, None, &r), Lasting::Goes(None));
        // THE ∞ IS LIT for the two "for ever"s only.
        let mut r = readings();
        for (forever, aside, keep, lit) in [
            (Forever::No, None, Keep::DEFAULT, false),
            (Forever::No, Some(NOW), Keep::DEFAULT, false),
            (Forever::No, None, Keep::Forever, false),
            (Forever::ByVideo, None, Keep::DEFAULT, true),
            (Forever::ByChannel, Some(NOW), Keep::DEFAULT, true),
        ] {
            w.forever = forever;
            w.set_aside_at_s = aside;
            r.settings = Some(Settings {
                keep,
                ..Settings::default()
            });
            let marks = marks_of(&w, &r, &t);
            let infinity = marks
                .iter()
                .find(|c| matches!(c.target, Target::VideoKeep(..)))
                .expect("the ∞");
            assert_eq!(
                infinity.lit, lit,
                "{forever:?}, set aside {aside:?}, {keep:?}"
            );
        }
        // THE VIDEO'S PAGE: off, and quietly beside it when it goes.
        let r = readings();
        let id = VideoId("video000000".to_owned());
        let (v, _) = View::fresh(Page::Video(id));
        let rows = v.rows(&r, COLUMNS, &t);
        let n = row_of(&rows, "never-delete");
        let goes = Lasting::Goes(Some(NOW + 90 * day)).words(&r, &t);
        let beside = t.t_with(key::VIDEO_METRICS_DEFAULT_NOW, &[("value", &goes)]);
        let fields: Vec<&Field> = rows[n..n + Row::lines_of(&rows, n)]
            .iter()
            .flat_map(|l| &l.fields)
            .collect();
        let quiet = fields
            .iter()
            .find(|f| f.text == beside)
            .unwrap_or_else(|| panic!("when it goes, unsaid: {fields:?}"));
        assert_eq!(quiet.color, tokens::color::TEXT_DIM);
        assert_eq!(
            rows[n].target,
            Target::VideoKeep(VideoId("video000000".to_owned()), false)
        );
        row_of(&rows, "set-aside");
    }

    /// FAILS IF: the bookmark of a set-aside video, or the ∞ of one kept by
    /// its own mark, says what holds once it is off without asking
    /// [`lasting`] WITHOUT that mark — "it goes" of a video its channel, the
    /// catalogue, its being set aside or its other mark still keeps (the
    /// set-aside video whose channel keeps every video, photographed); or a
    /// glyph that is no mark of the video's own says anything more.
    #[test]
    fn what_holds_once_a_mark_is_off_is_read_without_it() {
        fn follows(
            v: &RankedVideo,
            r: &Readings,
            t: &ronoctua_i18n::Texts,
            aside: bool,
        ) -> Option<String> {
            marks_of(v, r, t)
                .into_iter()
                .find(|c| {
                    if aside {
                        matches!(c.target, Target::SetAside(..))
                    } else {
                        matches!(c.target, Target::VideoKeep(..))
                    }
                })
                .expect("the glyph")
                .follows
        }
        let t = texts("en-GB");
        let day = 86_400;
        let mut r = readings();
        let a = r.channels[0].clone();
        let mut v = ranked(3, &a);
        let goes = Lasting::Goes(Some(NOW - 3 * day + 90 * day));
        let out = |l: Lasting, r: &Readings| {
            Some(t.t_with(key::VIDEO_METRICS_ONCE_OUT, &[("value", &l.words(r, &t))]))
        };
        let unmarked = |l: Lasting, r: &Readings| {
            Some(t.t_with(
                key::VIDEO_METRICS_WITHOUT_THE_MARK,
                &[("value", &l.words(r, &t))],
            ))
        };
        // NO MARK OF ITS OWN: nothing more.
        assert_eq!(follows(&v, &r, &t, true), None);
        assert_eq!(follows(&v, &r, &t, false), None);
        // SET ASIDE: taken out, it goes when its time is up — unless its
        // channel keeps every video, or its own mark keeps it.
        v.set_aside_at_s = Some(NOW);
        assert_eq!(follows(&v, &r, &t, true), out(goes, &r));
        assert_eq!(follows(&v, &r, &t, false), None);
        r.channels[0].keep = Some(Keep::Forever);
        assert_eq!(follows(&v, &r, &t, true), out(Lasting::ByChannel, &r));
        r.channels[0].keep = None;
        v.forever = Forever::ByVideo;
        assert_eq!(follows(&v, &r, &t, true), out(Lasting::ByVideo, &r));
        // KEPT BY ITS OWN MARK: without it, set aside keeps it while it is…
        assert_eq!(follows(&v, &r, &t, false), unmarked(Lasting::SetAside, &r));
        // …its time, its channel's "for ever", everywhere's "never".
        v.set_aside_at_s = None;
        assert_eq!(follows(&v, &r, &t, false), unmarked(goes, &r));
        r.channels[0].keep = Some(Keep::Forever);
        assert_eq!(follows(&v, &r, &t, false), unmarked(Lasting::ByChannel, &r));
        r.channels[0].keep = None;
        r.settings = Some(Settings {
            keep: Keep::Forever,
            ..Settings::default()
        });
        assert_eq!(
            follows(&v, &r, &t, false),
            unmarked(Lasting::Everywhere, &r)
        );
    }

    /// FAILS IF: how long videos stay does not turn round its ring — the
    /// days, then "never deleted", a channel's "as everywhere" first and what
    /// everywhere is now beside it, quietly —, a number typed of its own is
    /// left out of the ring (a turn would jump back to its start), a keep is
    /// written from switches nobody read, a number typed out of the days the
    /// catalogue keeps is clamped instead of refused, or what the next scan
    /// takes out is not said under it when it takes any.
    #[test]
    fn lasting_follows_the_ring_typed_or_turned() {
        let t = texts("en-GB");
        let mut r = readings();
        let keep_of = |e: Errand| match e {
            Errand::Edit(Edit::Settings(s)) => s.keep,
            other => panic!("not the switches: {other:?}"),
        };
        // EVERYWHERE: a ring of days, then "never deleted", and round.
        let (mut v, _) = View::fresh(Page::Rules);
        let rows = onto(&mut v, &r, "keep");
        assert!(
            rows[row_of(&rows, "keep")]
                .fields
                .iter()
                .any(|f| f.text == words::days(90, &t)),
            "the keep everywhere unsaid"
        );
        assert_eq!(keep_of(press(&mut v, &r, Press::More)), Keep::Days(180));
        assert_eq!(keep_of(press(&mut v, &r, Press::Less)), Keep::Days(60));
        r.settings = Some(Settings {
            keep: Keep::Forever,
            ..Settings::default()
        });
        assert_eq!(keep_of(press(&mut v, &r, Press::More)), Keep::Days(7));
        r.settings = Some(Settings {
            keep: Keep::Days(45),
            ..Settings::default()
        });
        assert_eq!(keep_of(press(&mut v, &r, Press::More)), Keep::Days(60));
        assert_eq!(keep_of(press(&mut v, &r, Press::Less)), Keep::Days(30));
        // TYPED: any number of days, and nothing else.
        let asked = press(&mut v, &r, Press::Enter);
        assert!(
            matches!(&asked, Errand::Ask { ask: Ask::KeepDays, initial: Some(n) } if n == "45"),
            "{asked:?}"
        );
        assert_eq!(
            keep_of(v.typed(Ask::KeepDays.command(), "120", &r, &t)),
            Keep::Days(120)
        );
        for refused in ["0", "lots", "40000"] {
            press(&mut v, &r, Press::Enter);
            assert!(
                matches!(
                    v.typed(Ask::KeepDays.command(), refused, &r, &t),
                    Errand::Say(_)
                ),
                "`{refused}` was taken"
            );
        }
        // FROM THE SWITCHES READ, or not at all.
        let mut unread = r.clone();
        unread.settings = None;
        assert_eq!(
            v.turn(&Target::KeepEverywhere(Keep::DEFAULT), true, &unread),
            Errand::Nothing
        );
        // WHAT THE NEXT SCAN TAKES OUT, quietly, only when it takes any.
        let said = |r: &Readings, page: Page| -> bool {
            let (v, _) = View::fresh(page);
            let expiring = t.t_args(key::VIDEO_METRICS_EXPIRING, &[("count", Arg::Number(3))]);
            v.rows(r, COLUMNS, &t)
                .iter()
                .flat_map(|x| &x.fields)
                .any(|f| f.text == expiring && f.color == tokens::color::TEXT_DIM)
        };
        let a = channel(1).id;
        let mut counted = readings();
        counted.rules = Some((Some(a.clone()), global_rules()));
        counted.expiring = Some(3);
        assert!(said(&counted, Page::Rules), "the rules' page");
        assert!(said(&counted, Page::Channel(a.clone())), "a channel's page");
        counted.expiring = Some(0);
        assert!(!said(&counted, Page::Rules));
        // A CHANNEL: as everywhere first — what that is now beside it —, then
        // the ring, then "never deleted", and round.
        let mut r = readings();
        r.rules = Some((Some(a.clone()), global_rules()));
        let (mut v, _) = View::fresh(Page::Channel(a.clone()));
        let rows = onto(&mut v, &r, "keep");
        let n = row_of(&rows, "keep");
        let everywhere = t.t_with(
            key::VIDEO_METRICS_DEFAULT_NOW,
            &[("value", &words::days(90, &t))],
        );
        assert!(
            rows[n..n + Row::lines_of(&rows, n)]
                .iter()
                .flat_map(|l| &l.fields)
                .any(|f| f.text == everywhere && f.color == tokens::color::TEXT_DIM),
            "what everywhere keeps, unsaid beside “as everywhere”"
        );
        let channel_keep = |e: Errand| match e {
            Errand::Edit(Edit::ChannelKeep(id, keep)) => {
                assert_eq!(id, a);
                keep
            }
            other => panic!("not the channel's keep: {other:?}"),
        };
        assert_eq!(
            channel_keep(press(&mut v, &r, Press::More)),
            Some(Keep::Days(7))
        );
        assert_eq!(
            channel_keep(press(&mut v, &r, Press::Less)),
            Some(Keep::Forever)
        );
        r.channels[0].keep = Some(Keep::Forever);
        assert_eq!(channel_keep(press(&mut v, &r, Press::More)), None);
        let asked = press(&mut v, &r, Press::Enter);
        assert!(
            matches!(
                asked,
                Errand::Ask {
                    ask: Ask::KeepDays,
                    initial: None
                }
            ),
            "{asked:?}"
        );
        assert_eq!(
            channel_keep(v.typed(Ask::KeepDays.command(), "45", &r, &t)),
            Some(Keep::Days(45))
        );
    }

    /// FAILS IF: the set-aside videos are not a filter of the list — any,
    /// only them with the last set aside first, none of them — the list by
    /// score when they are not only them, the home page's way to them does
    /// not open only them, hidden ones included, the last first; or the keys
    /// on a video do not set it aside or mark it never to be deleted.
    #[test]
    fn the_set_aside_videos_are_a_filter_and_a_way_from_home() {
        let t = texts("en-GB");
        let r = readings();
        let (mut v, _) = View::fresh(Page::Videos);
        onto(&mut v, &r, "filter-set-aside");
        v.page = 1;
        press(&mut v, &r, Press::More);
        assert_eq!(
            (v.filter.set_aside, v.filter.order, v.page),
            (SetAside::Only, Order::NewestSetAside, 0)
        );
        // THE LIST READ FOR IT says by what it is ordered.
        let mut read = r.clone();
        if let Some(l) = read.listed.as_mut() {
            l.query.filter = v.filter.clone();
        }
        let rows = v.rows(&read, COLUMNS, &t);
        assert!(
            rows.iter()
                .flat_map(|x| &x.fields)
                .any(|f| f.text == t.t(key::VIDEO_METRICS_BY_SET_ASIDE)),
            "the list's title says it is by score"
        );
        press(&mut v, &r, Press::More);
        assert_eq!(
            (v.filter.set_aside, v.filter.order),
            (SetAside::Without, Order::Score)
        );
        // FROM HOME.
        let (mut home, _) = View::fresh(Page::Home);
        let mut counted = readings();
        counted.set_aside_count = Some(4);
        let rows = onto(&mut home, &counted, "set-aside-videos");
        assert!(
            rows[row_of(&rows, "set-aside-videos")]
                .fields
                .iter()
                .any(|f| f.text == "4"),
            "the home page does not say how many"
        );
        assert!(matches!(
            press(&mut home, &counted, Press::Enter),
            Errand::Look(Look {
                page: Page::Videos,
                ..
            })
        ));
        assert_eq!(
            home.filter,
            Filter {
                set_aside: SetAside::Only,
                hidden: Hidden::Any,
                order: Order::NewestSetAside,
                ..Filter::default()
            }
        );
        // THE KEYS ON A VIDEO, from the state read.
        let id = VideoId("video000000".to_owned());
        let (mut v, _) = View::fresh(Page::Video(id.clone()));
        assert_eq!(
            press(&mut v, &r, Press::SetAside),
            Errand::Edit(Edit::SetAside(id.clone(), true))
        );
        assert_eq!(
            press(&mut v, &r, Press::Keep),
            Errand::Edit(Edit::KeepVideo(id.clone(), true))
        );
        assert_eq!(
            press_of_key(Key::parse("s").expect("a key")),
            Some(Press::SetAside)
        );
        assert_eq!(
            press_of_key(Key::parse("f").expect("a key")),
            Some(Press::Keep)
        );
        // SAID, by its title.
        let (said, _) = v.took(Done::Edited(Edit::SetAside(id, true), Ok(())), &r, &t);
        assert!(
            matches!(&said, Some(Said::Normal(s)) if s.contains("A video, number 0")),
            "{said:?}"
        );
    }

    /// FAILS IF: the list with thumbnails stops putting each video's picture
    /// at its head — `thumbCells` by `thumbLines`, on the first lines of its
    /// entry and no other, the glyph of a video in its place while there is
    /// none — with the title beside it wrapped WHOLE (never cut with "…") and
    /// the line of what it scores under the title, glyphs and all, acting on
    /// the first click; the entry is not as tall as its words or its
    /// picture, a line of air after it; a line of it does not say where its
    /// words begin, or a field of it is drawn over the picture; a first click
    /// on its words does anything but select the video, or one on its
    /// picture — on any line of it, the line of glyphs as the title's — takes
    /// or explains anything (the plan: the click from the text, never over
    /// the figure); or, too narrow for a picture, it is not the list's own
    /// entry. At a narrow, a middling and a wide door, in either language.
    #[test]
    fn the_list_with_thumbnails_puts_each_picture_beside_its_words() {
        use ronoctua_render::layout::width;
        let cells = tokens::space::VIDEOMETRICS_THUMBCELLS;
        let lines = usize::from(tokens::space::VIDEOMETRICS_THUMBLINES);
        let file = PathBuf::from("/nowhere/video000000.jpg");
        let long = "A video whose title says a great deal more than a line holds, \
                    and then a little more again, so it wraps over several lines";
        let mut r = readings();
        with_video(&mut r, &VideoId("video000000".to_owned()), |x| {
            x.thumb_file = Some(file.clone());
            x.title = long.to_owned();
        });
        let mut narrow = 0;
        let mut pictured_at = 0;
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            // AND A DOOR TOO NARROW for a picture beside a title.
            for columns in [30, 44, 80, 200] {
                let what = format!("{language} at {columns}");
                let (left, room) = extent(columns);
                let (mut v, _) = View::fresh(Page::Videos);
                v.laid_out(Layout::Strip);
                let rows = v.rows(&r, columns, &t);
                if !strip_holds_a_picture(room) {
                    // TOO NARROW: the list's own entries, line for line —
                    // all but the spots of the lines that continue a row,
                    // counted from the page's top, where the layout's own
                    // words may wrap otherwise.
                    let drawn = |rows: &[Row]| -> Vec<Row> {
                        rows[row_of(rows, crate::paging::LIST_TITLE)..]
                            .iter()
                            .map(|x| Row {
                                spot: if x.continues || x.heading {
                                    String::new()
                                } else {
                                    x.spot.clone()
                                },
                                ..x.clone()
                            })
                            .collect()
                    };
                    let (mut list, _) = View::fresh(Page::Videos);
                    list.laid_out(Layout::List);
                    assert_eq!(
                        drawn(&rows),
                        drawn(&list.rows(&r, columns, &t)),
                        "{what}: not the list"
                    );
                    narrow += 1;
                    continue;
                }
                pictured_at += 1;
                let text_at = left + cells + ronoctua_design::GAP;
                for (k, spot, pictured) in [
                    (0, "video:video000000", true),
                    (1, "video:video000001", false),
                ] {
                    let n = row_of(&rows, spot);
                    let tall = Row::lines_of(&rows, n);
                    let entry = &rows[n..n + tall];
                    let title = crate::chrome::note_lines(
                        &r.listed.as_ref().expect("a list").rows[k].title,
                        left + room - text_at,
                    );
                    assert_eq!(
                        tall,
                        (title.len() + 1).max(lines) + 1,
                        "{what}: video {k} is {tall} lines"
                    );
                    assert!(
                        entry.iter().all(|x| x.words_at == Some(text_at)),
                        "{what}: a line of video {k} says no words' column"
                    );
                    let drawn: Vec<&str> = entry[..title.len()]
                        .iter()
                        .flat_map(|x| x.fields.iter().filter(|f| f.at == text_at))
                        .map(|f| f.text.as_str())
                        .collect();
                    assert_eq!(drawn, title, "{what}: video {k}'s title");
                    assert!(
                        drawn.iter().all(|x| !x.contains('…')),
                        "{what}: a title cut"
                    );
                    if k == 0 {
                        assert_eq!(drawn.join(" "), long, "{what}: the title is not whole");
                    }
                    // THE PICTURE, on the lines of its box and on no other.
                    for (l, x) in entry.iter().enumerate() {
                        let want: Vec<Thumb> = if pictured && l < lines {
                            vec![Thumb {
                                line: u16::try_from(l).unwrap_or(u16::MAX),
                                lines: tokens::space::VIDEOMETRICS_THUMBLINES,
                                left,
                                cells,
                                file: file.clone(),
                            }]
                        } else {
                            Vec::new()
                        };
                        assert_eq!(x.thumbs, want, "{what}: video {k}, line {l}");
                        for f in &x.fields {
                            let glyph = f.text == tokens::icon::VIDEO_NO_THUMB.to_string();
                            assert!(
                                f.at >= text_at || (glyph && !pictured && l == lines / 2),
                                "{what}: `{}` over video {k}'s picture",
                                f.text
                            );
                            assert!(f.at + width(&f.text) <= left + room, "{what}: `{}`", f.text);
                        }
                    }
                    assert_eq!(
                        !pictured,
                        entry[lines / 2]
                            .fields
                            .iter()
                            .any(|f| f.text == tokens::icon::VIDEO_NO_THUMB.to_string()),
                        "{what}: video {k}'s place for a picture"
                    );
                    // WHAT IT SCORES under the title, with its glyphs.
                    let meta = &entry[title.len()];
                    assert!(
                        !meta.controls.is_empty(),
                        "{what}: no glyphs under the title"
                    );
                    assert_eq!(
                        meta.words_box(left, true).map(|b| b.0),
                        Some(text_at),
                        "{what}: the words' box starts on the picture"
                    );
                    // AND ITS LINE OF AIR.
                    let air = &entry[tall - 1];
                    assert!(is_air(air) && air.fields.is_empty() && air.continues);
                }
                // A FIRST CLICK selects — on the title's words.
                let n = row_of(&rows, "video:video000001");
                for column in [text_at, text_at + 1] {
                    let (mut v, _) = View::fresh(Page::Videos);
                    v.laid_out(Layout::Strip);
                    let rows = v.rows(&r, columns, &t);
                    assert!(!v.clicked(n, column, &rows), "{what}: a first click acted");
                    assert_eq!(v.at, "video:video000001", "{what}: at {column}");
                }
                // AND THE PICTURE IS NOBODY'S, on every line of its video —
                // the title's, the line of glyphs, the air: a click there
                // takes nothing and the foot says nothing of it.
                let tall = Row::lines_of(&rows, n);
                for line in n..n + tall {
                    for column in [left, left + 1, text_at - 1] {
                        let (mut v, _) = View::fresh(Page::Videos);
                        v.laid_out(Layout::Strip);
                        let rows = v.rows(&r, columns, &t);
                        let was = v.at.clone();
                        assert!(
                            !v.clicked(line, column, &rows),
                            "{what}: line {line} acted at {column}"
                        );
                        assert_eq!(v.at, was, "{what}: line {line} taken at {column}");
                        assert!(
                            v.explained_at(&rows, line, Some(column), &t).is_none(),
                            "{what}: line {line} explained at {column}"
                        );
                    }
                }
                // A GLYPH under the title acts at once.
                let (mut v, _) = View::fresh(Page::Videos);
                v.laid_out(Layout::Strip);
                let rows = v.rows(&r, columns, &t);
                let (line, c) = glyph_at(&rows, &|x| matches!(x, Target::Rate(..)));
                assert!(
                    v.click_control(line, c.from, &rows, &r).is_some(),
                    "{what}: a glyph waits"
                );
            }
        }
        assert!(
            narrow > 0 && pictured_at > 0,
            "{narrow} narrow, {pictured_at} pictured"
        );
    }

    /// FAILS IF: the layouts stop turning as a ring of five — `→`, a click
    /// and `RET` the next, `←` the one before, `L` the next — or say one
    /// layout in another's words, or two in the same; the list reads the page
    /// again for anything but the thumbnails coming or going; a switch of the
    /// channel comes back beside the ring (the owner, 2026-11-10: one way to
    /// one choice); or the grid with the channel, or the wide thumbnails, do
    /// not put it under each title, while the grid without says none.
    #[test]
    fn the_layouts_turn_in_a_ring_and_one_says_the_channel() {
        let r = readings();
        let (mut v, _) = View::fresh(Page::Videos);
        let pictures = |e: &Errand| match e {
            Errand::Look(Look { query: Some(q), .. }) => Some(q.pictures),
            _ => None,
        };
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let said: Vec<String> = Layout::ALL.iter().map(|l| t.t(l.key())).collect();
            let mut distinct = said.clone();
            distinct.sort();
            distinct.dedup();
            assert_eq!(
                distinct.len(),
                Layout::ALL.len(),
                "{language}: two layouts alike: {said:?}"
            );
        }
        let t = texts("en-GB");
        onto(&mut v, &r, "layout");
        // →: the next, read again only where the thumbnails come or go.
        let mut asked = Vec::new();
        for _ in Layout::ALL {
            asked.push((pictures(&press(&mut v, &r, Press::More)), v.layout));
            assert!(
                !spots(&v.rows(&r, 100, &t)).contains(&"grid-channel"),
                "{:?}: a switch of the channel beside the ring",
                v.layout
            );
            let rows = v.rows(&r, 100, &t);
            let layout = &rows[row_of(&rows, "layout")];
            assert!(
                layout.fields.iter().any(|f| f.text == t.t(v.layout.key())),
                "{:?}: the row does not say it",
                v.layout
            );
        }
        assert_eq!(
            asked,
            vec![
                (Some(true), Layout::Strip),
                (None, Layout::Grid),
                (None, Layout::GridWithChannel),
                (None, Layout::Wide),
                (Some(false), Layout::List),
            ]
        );
        // ←: the one before; `L` and `RET`: the next.
        press(&mut v, &r, Press::Less);
        assert_eq!(v.layout, Layout::Wide);
        press(&mut v, &r, Press::Less);
        assert_eq!(v.layout, Layout::GridWithChannel);
        press(&mut v, &r, Press::Layout);
        assert_eq!(v.layout, Layout::Wide);
        press(&mut v, &r, Press::Layout);
        assert_eq!(v.layout, Layout::List);
        press(&mut v, &r, Press::Enter);
        assert_eq!(v.layout, Layout::Strip);
        // THE CHANNEL under every title, a line taller — only in its layout.
        let said = |layout: Layout| -> (usize, usize) {
            let (mut v, _) = View::fresh(Page::Videos);
            v.layout = layout;
            let rows = v.rows(&r, 100, &t);
            let shelf = rows
                .iter()
                .position(|x| !x.cells.is_empty())
                .expect("a shelf");
            let channel = rows
                .iter()
                .flat_map(|x| &x.fields)
                .filter(|f| f.text == "Channel 1")
                .count();
            (Row::lines_of(&rows, shelf), channel)
        };
        assert_eq!(
            said(Layout::Grid),
            (usize::from(shelf_lines(Layout::Grid)), 0)
        );
        for layout in [Layout::GridWithChannel, Layout::Wide] {
            let (lines, channel) = said(layout);
            assert_eq!(lines, usize::from(shelf_lines(layout)), "{layout:?}");
            assert_eq!(channel, 10, "{layout:?}: not every tile says its channel");
        }
    }

    /// FAILS IF: wide thumbnails draw past a door too narrow for one, or are
    /// not the grid with the channel there — or a shelf of them holds
    /// anything but half the thumbnails a shelf of the grid does, the room of
    /// a wide tile being two of the grid's.
    #[test]
    fn wide_thumbnails_are_two_of_the_grid_and_the_grid_where_one_does_not_fit() {
        let r = readings();
        let t = texts("en-GB");
        let shelf_of = |layout: Layout, columns: u16| {
            let (mut v, _) = View::fresh(Page::Videos);
            v.layout = layout;
            let rows = v.rows(&r, columns, &t);
            for row in &rows {
                for f in &row.fields {
                    let end = f.at + ronoctua_render::layout::width(&f.text);
                    assert!(
                        end <= columns,
                        "{layout:?} at {columns}: `{}` ends at {end}",
                        f.text
                    );
                }
            }
            let shelf = rows
                .iter()
                .position(|x| !x.cells.is_empty())
                .expect("a shelf");
            (rows[shelf].cells.clone(), Row::lines_of(&rows, shelf))
        };
        let wide = tile_cells(Layout::Wide);
        let narrow = (wide..)
            .find(|&c| extent(c).1 >= wide)
            .expect("a door that holds a wide tile")
            - 1;
        let (cells, lines) = shelf_of(Layout::Wide, narrow);
        assert_eq!(cells[0].width, tile_cells(Layout::Grid) + 2 * TILE_AIR);
        assert_eq!(lines, usize::from(shelf_lines(Layout::GridWithChannel)));
        for columns in [narrow + 1, 120, 200] {
            let (two, _) = shelf_of(Layout::Wide, columns);
            let (four, _) = shelf_of(Layout::Grid, columns);
            assert_eq!(two[0].width, wide + 2 * TILE_AIR, "at {columns}");
            assert_eq!(four.len() / 2, two.len(), "at {columns}");
        }
    }

    /// A video's title longer than a tile's two lines, for the tests of a
    /// name said whole.
    pub(crate) const LONG_TITLE: &str =
        "A video whose title runs on far longer than any tile's two lines can ever hold";

    /// And a channel's name longer than a tile is wide.
    pub(crate) const LONG_CHANNEL: &str = "A channel whose name is far longer than a tile";

    /// `r` with every video of its page titled [`LONG_TITLE`], of a channel
    /// named [`LONG_CHANNEL`].
    pub(crate) fn long_names(r: &mut Readings) {
        for x in &mut r.listed.as_mut().expect("a page of videos").rows {
            LONG_TITLE.clone_into(&mut x.title);
            LONG_CHANNEL.clone_into(&mut x.channel);
        }
    }

    /// FAILS IF: what explains a video of the list — the window's foot, the
    /// footer's line — does not NAME it by its title WHOLE where the pointer
    /// rests on the title's words, and by its channel's name whole on the
    /// channel's, which a tile cuts (the owner, 2026-11-11); or the score,
    /// the age, the picture or a glyph is named by the channel instead of
    /// what it was named by. In every layout, in both languages.
    #[test]
    fn a_name_under_the_pointer_names_the_thing_whole() {
        let mut r = readings();
        long_names(&mut r);
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for layout in Layout::ALL {
                let (mut v, _) = View::fresh(Page::Videos);
                v.laid_out(layout);
                let rows = v.rows(&r, COLUMNS, &t);
                let what = format!("{language}, {layout:?}");
                let name = |n: usize, column: u16| {
                    v.explained_at(&rows, n, Some(column), &t).map(|e| e.name)
                };
                let (mut titles, mut channels, mut cut) = (0, 0, false);
                // THE VIDEOS' LINES — a row's, a shelf's — and not the
                // filters over them, whose words are no name.
                let of_a_video = |n: usize| {
                    let o = &rows[Row::owner(&rows, n)];
                    matches!(o.target, Target::Video(_)) || !o.cells.is_empty()
                };
                for (n, row) in rows.iter().enumerate() {
                    if !of_a_video(n) {
                        continue;
                    }
                    for f in &row.fields {
                        let text = f.text.trim_end_matches('…').trim_end();
                        let last = f.at + ronoctua_render::layout::width(&f.text) - 1;
                        if text.chars().count() > 1
                            && LONG_TITLE.contains(text)
                            && !LONG_CHANNEL.contains(text)
                        {
                            for column in [f.at, last] {
                                assert_eq!(
                                    name(n, column).as_deref(),
                                    Some(LONG_TITLE),
                                    "{what}: {:?} on line {n}",
                                    f.text
                                );
                            }
                            cut |= f.text.ends_with('…');
                            titles += 1;
                        } else if f.text.starts_with("A channel") {
                            assert_eq!(
                                name(n, f.at).as_deref(),
                                Some(LONG_CHANNEL),
                                "{what}: {:?} on line {n}",
                                f.text
                            );
                            channels += 1;
                            // THE WORDS AFTER IT on its line — the age, the
                            // score — are the video's.
                            if !f.text.ends_with('…') {
                                assert_eq!(
                                    name(n, last).as_deref(),
                                    Some(LONG_TITLE),
                                    "{what}: the score on line {n}"
                                );
                            }
                        } else {
                            // THE SCORE, THE AGE, THE PICTURE, A GLYPH.
                            assert_ne!(
                                name(n, f.at).as_deref(),
                                Some(LONG_CHANNEL),
                                "{what}: {:?} on line {n}",
                                f.text
                            );
                        }
                    }
                    for c in &row.controls {
                        assert_eq!(
                            name(n, c.from).as_deref(),
                            Some(c.name.as_str()),
                            "{what}: a glyph on line {n}"
                        );
                    }
                }
                assert!(titles >= 10, "{what}: {titles} titles");
                // THE LIST SAYS EVERY CHANNEL, the grid with the channel
                // under each title too — and the grid's tile cuts both; a
                // wide one, whose two lines are twice a tile's, holds this
                // title whole.
                if layout != Layout::Grid {
                    assert!(channels >= 10, "{what}: {channels} channels");
                }
                assert_eq!(cut, layout.grid() && layout != Layout::Wide, "{what}: cut");
            }
        }
    }

    /// FAILS IF: what every door fills the download's sentence with teaches
    /// the queue's keys without the leader's label — `t t` where the other
    /// doors say `SUPER t t` (`keys-are-never-text`) — or the sentence does
    /// not say it.
    #[test]
    fn a_download_teaches_the_queue_with_the_leader() {
        let keys = crate::bindings::with_stub(&[("t t", "transfers-open")], transfers_keys);
        assert_eq!(keys, format!("{} t t", crate::bindings::label()));
        let said = texts("en-GB").t_with(
            key::VIDEO_METRICS_DOWNLOADING,
            &[("title", "A film"), ("keys", &keys)],
        );
        assert!(said.contains(&keys), "{said}");
    }

    /// FAILS IF: the home page stops linking out — the brain's page on the
    /// scan, the player's two lists in a window of its own — under its own
    /// heading, after the catalogue; `RET` on one of them does anything but
    /// hand its door the errand that leaves the view (and leaves the view
    /// where it was), or one of them is not explained as what it opens.
    #[test]
    fn the_home_page_links_out() {
        use crate::player_seam::ranking::{OPEN_RANKING, OPEN_SET_ASIDE};
        let r = readings();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let (v, _) = View::fresh(Page::Home);
            let rows = v.rows(&r, COLUMNS, &t);
            let heading = rows
                .iter()
                .position(|x| {
                    x.heading
                        && x.fields.first().map(|f| f.text.as_str())
                            == Some(t.t(key::VIDEO_METRICS_HEADING_ELSEWHERE).as_str())
                })
                .expect("the heading of the other apps");
            assert!(
                row_of(&rows, "weights") < heading,
                "{language}: the other apps before the catalogue"
            );
            let links = [
                ("in-the-brain", Errand::Brain),
                ("in-the-player", Errand::Player(OPEN_RANKING)),
                ("set-aside-in-the-player", Errand::Player(OPEN_SET_ASIDE)),
            ];
            let mut last = heading;
            for (spot, errand) in links {
                let n = row_of(&rows, spot);
                assert!(n > last, "{language}: `{spot}` out of its place");
                last = n;
                let (mut v, _) = View::fresh(Page::Home);
                onto(&mut v, &r, spot);
                let rows = v.rows(&r, COLUMNS, &t);
                let e = v.explained_cursor(&rows, &t).expect("explained");
                assert_eq!(e.name, rows[n].name, "{language}: `{spot}`");
                assert!(
                    e.gesture
                        .as_deref()
                        .is_some_and(|g| g.contains(&t.t(key::VIDEO_METRICS_ACT_OPENS))),
                    "{language}: `{spot}` does not say it opens: {e:?}"
                );
                assert_eq!(press(&mut v, &r, Press::Enter), errand, "`{spot}`");
                assert_eq!(v.page(), &Page::Home, "`{spot}` moved the view");
            }
        }
    }

    /// FAILS IF: a video, or a channel's videos, do not open in either
    /// browser from every place they are offered — a video's rows and keys
    /// (`K`, `n`), a channel's page (its rows, and the keys on any row of
    /// it), a channel's row on the list (its keys and its two glyphs, which
    /// act on the first click, put the cursor on the channel and each say
    /// its own sentence under the pointer) —, or a channel opens on anything
    /// but its `/videos` page; or the browsers' keys are taught, or act,
    /// where nothing has an address.
    #[test]
    fn a_video_or_a_channel_opens_in_either_browser() {
        let r = readings();
        let t = texts("en-GB");
        let video = r.listed.as_ref().expect("listed").rows[0].clone();
        let a = channel(1);
        let videos = format!("https://www.youtube.com/channel/{}/videos", a.id.0);
        let taught = |v: &View| -> Vec<&'static str> {
            let rows = v.rows(&r, COLUMNS, &t);
            v.hint(&rows, &t).iter().filter_map(|p| p.id).collect()
        };
        // A VIDEO'S PAGE: its rows, and its keys.
        let (mut v, _) = View::fresh(Page::Video(video.id.clone()));
        onto(&mut v, &r, "nyxt");
        assert_eq!(
            press(&mut v, &r, Press::Enter),
            Errand::Nyxt(video.url.clone())
        );
        assert_eq!(press(&mut v, &r, Press::Kiosk), Errand::Kiosk(video.url));
        assert!(taught(&v).contains(&"video-metrics-nyxt"));
        // A CHANNEL'S PAGE: its two rows, and the keys on any row of it.
        let (mut v, _) = View::fresh(Page::Channel(a.id.clone()));
        onto(&mut v, &r, "channel-in-chrome");
        assert_eq!(
            press(&mut v, &r, Press::Enter),
            Errand::Kiosk(videos.clone())
        );
        onto(&mut v, &r, "channel-in-nyxt");
        assert_eq!(
            press(&mut v, &r, Press::Enter),
            Errand::Nyxt(videos.clone())
        );
        onto(&mut v, &r, "score");
        assert_eq!(press(&mut v, &r, Press::Nyxt), Errand::Nyxt(videos.clone()));
        let here = taught(&v);
        assert!(here.contains(&"video-metrics-kiosk") && here.contains(&"video-metrics-nyxt"));
        assert!(
            !here.contains(&"video-metrics-player"),
            "a video's key on a channel"
        );
        // A CHANNEL'S ROW ON THE LIST: its keys, and its glyphs at its edge.
        let spot = format!("channel:{}", a.id.0);
        let (left, room) = extent(COLUMNS);
        let (mut v, _) = View::fresh(Page::Channels);
        let rows = v.rows(&r, COLUMNS, &t);
        let n = row_of(&rows, &spot);
        v.clicked(n, left, &rows);
        assert_eq!(
            press(&mut v, &r, Press::Kiosk),
            Errand::Kiosk(videos.clone())
        );
        assert_eq!(press(&mut v, &r, Press::Nyxt), Errand::Nyxt(videos.clone()));
        assert_eq!(press(&mut v, &r, Press::Player), Errand::Nothing);
        let glyphs = rows[n].controls.clone();
        assert_eq!(glyphs.len(), 2, "a channel's row has no ways out");
        let ends = glyphs.last().map(|c| c.from + c.width);
        assert_eq!(ends, Some(left + room), "the glyphs are not at the edge");
        for ((c, errand), about) in glyphs
            .iter()
            .zip([Errand::Kiosk(videos.clone()), Errand::Nyxt(videos)])
            .zip([
                key::ABOUT_VIDEO_METRICS_CHANNEL_IN_CHROME,
                key::ABOUT_VIDEO_METRICS_CHANNEL_IN_NYXT,
            ])
        {
            assert_eq!(c.about, about, "a glyph explained as another thing");
            let (mut v, _) = View::fresh(Page::Channels);
            let rows = v.rows(&r, COLUMNS, &t);
            assert_eq!(v.click_control(n, c.from, &rows, &r), Some(errand));
            assert_eq!(v.cursor(&rows), n, "the cursor is not on the channel");
        }
        // NOTHING WITH AN ADDRESS: the home page, the list's first row.
        let (mut v, _) = View::fresh(Page::Home);
        assert_eq!(press(&mut v, &r, Press::Nyxt), Errand::Nothing);
        assert!(!taught(&v).contains(&"video-metrics-nyxt"));
        let (mut v, _) = View::fresh(Page::Channels);
        onto(&mut v, &r, "add-channel");
        assert_eq!(press(&mut v, &r, Press::Kiosk), Errand::Nothing);
        assert!(!taught(&v).contains(&"video-metrics-kiosk"));
    }

    /// What importing a list left, as the import page says it: two channels
    /// staged, one found here already, a line that named no channel.
    fn imported() -> Imported {
        let here = channel(1);
        Imported {
            staged: 2,
            already: vec![(here.id, here.title)],
            left_out: 1,
        }
    }

    /// A scratch folder of this test's own.
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-video-metrics-view-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        dir
    }

    /// FAILS IF: the channels' list leads nowhere to import a list, or says
    /// nothing of the channels that wait, or lists a staged channel as one
    /// of its own; the import page misses a row of its own, in its order —
    /// Takeout, choosing the list, exporting, the decisions so far, all of
    /// them, registering once something is decided, then each staged
    /// channel by what registering does with it: the short rows before the
    /// long list —, or says what the last import left out and found here
    /// already when there was none, or elsewhere than beside the list
    /// chosen and after the channels to decide; a staged row's arrows do
    /// not turn its decision, its `RET` does not open its page, or the
    /// browsers' keys and glyphs do not reach its videos; or a staged
    /// channel's page does not say its decision, or says when it was added
    /// or what only a scan writes (when it was tried, where the scans
    /// stand, its videos).
    #[test]
    fn a_list_imported_waits_on_its_page_to_be_decided() {
        let mut r = readings();
        let t = texts("en-GB");
        let three = staged(3, Decision::Undecided).channel.id;
        let (spot3, spot4) = (
            format!("staged:{}", three.0),
            format!("staged:{}", staged(4, Decision::Keep).channel.id.0),
        );
        let videos = format!("https://www.youtube.com/channel/{}/videos", three.0);
        // THE CHANNELS' LIST: its way to the page, saying how many wait — and
        // none of them among its channels.
        let (mut v, _) = View::fresh(Page::Channels);
        let rows = v.rows(&r, COLUMNS, &t);
        let n = row_of(&rows, "import-export");
        // DECIDED OR NOT, both wait: one of the two is kept already.
        let waiting = t.t_args(
            key::VIDEO_METRICS_UNREGISTERED,
            &[("count", Arg::Number(2))],
        );
        assert!(
            rows[n].fields.iter().any(|f| f.text == waiting),
            "{:?}",
            rows[n].fields
        );
        assert!(!spots(&rows).contains(&format!("channel:{}", three.0).as_str()));
        onto(&mut v, &r, "import-export");
        assert_eq!(
            press(&mut v, &r, Press::Enter),
            Errand::Look(Look {
                page: Page::Import,
                query: None
            })
        );
        // THE PAGE, in its order; nothing of an import before there was one.
        let rows = v.rows(&r, COLUMNS, &t);
        let order: Vec<usize> = [
            "takeout",
            "choose-list",
            "save-list",
            "decisions",
            "decide-all",
            "register",
            spot3.as_str(),
            spot4.as_str(),
        ]
        .iter()
        .map(|s| row_of(&rows, s))
        .collect();
        assert!(order.is_sorted(), "out of order: {:?}", spots(&rows));
        assert!(!spots(&rows).contains(&"left-out"));
        // WHAT THE LAST IMPORT FOUND, beside the list chosen: what it left
        // out, then the channels here already — before the export and the
        // channels to decide, which run to hundreds.
        v.imported = Some(imported());
        let rows = v.rows(&r, COLUMNS, &t);
        let already = format!("already:{}", channel(1).id.0);
        let order: Vec<usize> = ["choose-list", "left-out", already.as_str(), "save-list"]
            .iter()
            .map(|s| row_of(&rows, s))
            .collect();
        assert!(order.is_sorted(), "out of order: {:?}", spots(&rows));
        // TAKEOUT, in Chrome.
        onto(&mut v, &r, "takeout");
        assert_eq!(
            press(&mut v, &r, Press::Enter),
            Errand::Kiosk(takeout::TAKEOUT_PAGE.to_owned())
        );
        // A STAGED ROW: its arrows turn its decision, the browsers reach its
        // videos — by key and by its two glyphs. Clicked on its words, which
        // end before its glyphs.
        let (left, _) = extent(COLUMNS);
        let on_its_words = |v: &mut View, spot: &str| {
            let rows = v.rows(&r, COLUMNS, &t);
            v.clicked(row_of(&rows, spot), left, &rows);
            v.rows(&r, COLUMNS, &t)
        };
        let rows = on_its_words(&mut v, &spot3);
        assert_eq!(
            press(&mut v, &r, Press::More),
            Errand::Edit(Edit::Decide(three.clone(), Decision::Keep))
        );
        assert_eq!(
            press(&mut v, &r, Press::Less),
            Errand::Edit(Edit::Decide(three.clone(), Decision::Discard))
        );
        assert_eq!(
            press(&mut v, &r, Press::Kiosk),
            Errand::Kiosk(videos.clone())
        );
        assert_eq!(press(&mut v, &r, Press::Nyxt), Errand::Nyxt(videos.clone()));
        let taught: Vec<&str> = v.hint(&rows, &t).iter().filter_map(|p| p.id).collect();
        assert!(taught.contains(&"video-metrics-more") && taught.contains(&"video-metrics-nyxt"));
        let n = row_of(&rows, &spot3);
        let glyphs = rows[n].controls.clone();
        assert_eq!(glyphs.len(), 2, "a staged row has no ways out");
        for (c, errand) in glyphs
            .iter()
            .zip([Errand::Kiosk(videos.clone()), Errand::Nyxt(videos)])
        {
            let (mut v, _) = View::fresh(Page::Import);
            v.imported = Some(imported());
            let rows = v.rows(&r, COLUMNS, &t);
            assert_eq!(v.click_control(n, c.from, &rows, &r), Some(errand));
            assert_eq!(v.cursor(&rows), n, "the cursor is not on the channel");
        }
        // ITS PAGE: what registering does with it, and nothing a scan writes.
        on_its_words(&mut v, &spot3);
        assert_eq!(
            press(&mut v, &r, Press::Enter),
            Errand::Look(Look {
                page: Page::Channel(three.clone()),
                query: None
            })
        );
        let rows = v.rows(&r, COLUMNS, &t);
        let here = spots(&rows);
        for kept in [
            "decision",
            "score",
            "shorts",
            "keep",
            "channel-in-nyxt",
            "remove-channel",
        ] {
            assert!(
                here.contains(&kept),
                "the staged page has no `{kept}`: {here:?}"
            );
        }
        for gone in ["added", "its-videos", "last-try", "reference"] {
            assert!(!here.contains(&gone), "the staged page says `{gone}`");
        }
        assert!(row_of(&rows, "decision") < row_of(&rows, "address"));
        onto(&mut v, &r, "decision");
        assert_eq!(
            press(&mut v, &r, Press::More),
            Errand::Edit(Edit::Decide(three, Decision::Keep))
        );
        // A CHANNEL OF THE CATALOGUE says what a scan wrote, and no decision.
        let (v, _) = View::fresh(Page::Channel(channel(1).id));
        let here = v.rows(&r, COLUMNS, &t);
        assert!(!spots(&here).contains(&"decision") && spots(&here).contains(&"its-videos"));
        // NOTHING DECIDED: nothing to register.
        for s in &mut r.staged {
            s.decision = Decision::Undecided;
        }
        let (v, _) = View::fresh(Page::Import);
        assert!(!spots(&v.rows(&r, COLUMNS, &t)).contains(&"register"));
    }

    /// FAILS IF: registering what was decided is not asked first — under
    /// its row, with the "no" first and the cursor born on it, saying how
    /// many are kept and how many dropped —, the "no" does anything, or the
    /// "yes" does anything but register; or a registering whose consent to
    /// scan could not be kept is said without how many channels went in and
    /// were dropped, or in the words of one channel added.
    #[test]
    fn registering_is_asked_with_no_first() {
        let r = readings();
        let t = texts("en-GB");
        let (mut v, _) = View::fresh(Page::Import);
        onto(&mut v, &r, "register");
        assert_eq!(press(&mut v, &r, Press::Enter), Errand::Nothing);
        let rows = v.rows(&r, COLUMNS, &t);
        assert_eq!(rows[v.cursor(&rows)].spot, "answer-no");
        let (register, no, yes) = (
            row_of(&rows, "register"),
            row_of(&rows, "answer-no"),
            row_of(&rows, "answer-yes"),
        );
        assert!(register < no && no < yes, "{:?}", spots(&rows));
        let asked = t.t_args(
            key::ASK_VIDEO_METRICS_REGISTER,
            &[("kept", Arg::Number(1)), ("discarded", Arg::Number(0))],
        );
        assert_eq!(asked_between(&rows, register, no), asked);
        assert_eq!(
            press(&mut v, &r, Press::Enter),
            Errand::Nothing,
            "the no did something"
        );
        let rows = v.rows(&r, COLUMNS, &t);
        assert_eq!(rows[v.cursor(&rows)].spot, "register");
        press(&mut v, &r, Press::Enter);
        press(&mut v, &r, Press::Down);
        assert_eq!(
            press(&mut v, &r, Press::Enter),
            Errand::Edit(Edit::Register)
        );
        // WHAT REGISTERING DID — and, the consent to scan them not kept, how
        // many went in and were dropped all the same, before why a scan will
        // not ask: never the one channel a person added.
        let done = Registered {
            kept: 12,
            discarded: 1,
            left: 0,
        };
        let registered = t.t_args(
            key::VIDEO_METRICS_REGISTERED,
            &[("kept", Arg::Number(12)), ("discarded", Arg::Number(1))],
        );
        let (said, _) = v.took(Done::Registered(Ok((done, None))), &r, &t);
        assert_eq!(said, Some(Said::Normal(registered.clone().into())));
        let why = words::unkept_alone_key(ronoctua_consent::Unkept::Unwritable);
        let (said, _) = v.took(Done::Registered(Ok((done, Some(why)))), &r, &t);
        let Some(Said::Long(said)) = said else {
            panic!("an unkept consent was not said as a trouble: {said:?}");
        };
        assert!(
            said.starts_with(&registered) && said.ends_with(&t.t(why)),
            "{said:?}"
        );
        assert!(!said.contains(&t.t(key::VIDEO_METRICS_NOT_LET_UNWRITABLE)));
    }

    /// FAILS IF: giving every staged channel one decision while theirs
    /// differ — by an arrow, `RET` or a double click, each a turn — throws
    /// away what each was given without asking first: under its row, "no"
    /// first and the cursor born on it, saying how many channels it changes
    /// and to what; or the "no" does anything, or the "yes" anything but
    /// give them all that decision; or the row says a turn goes on while it
    /// asks. Theirs alike, the ring turns at once — a turn back gives each
    /// what it had — and says so.
    #[test]
    fn deciding_all_of_them_is_asked_while_theirs_differ() {
        let mut r = readings();
        let t = texts("en-GB");
        let act = |v: &View, r: &Readings| {
            let rows = v.rows(r, COLUMNS, &t);
            does_of(&rows[v.cursor(&rows)].target)
        };
        // THEIRS DIFFER: one undecided, one kept.
        for press_it in [Press::More, Press::Less, Press::Enter] {
            let (mut v, _) = View::fresh(Page::Import);
            onto(&mut v, &r, "decide-all");
            assert_eq!(act(&v, &r), Some(key::VIDEO_METRICS_ACT_ASKS));
            assert_eq!(
                press(&mut v, &r, press_it),
                Errand::Nothing,
                "{press_it:?} gave them all one decision unasked"
            );
            let rows = v.rows(&r, COLUMNS, &t);
            assert_eq!(rows[v.cursor(&rows)].spot, "answer-no");
            let (all, no) = (row_of(&rows, "decide-all"), row_of(&rows, "answer-no"));
            assert!(all < no, "{:?}", spots(&rows));
            let asked = t.t_args(
                key::ASK_VIDEO_METRICS_DECIDE_ALL_UNDECIDED,
                &[("changed", Arg::Number(1))],
            );
            assert_eq!(asked_between(&rows, all, no), asked);
            assert_eq!(
                press(&mut v, &r, Press::Enter),
                Errand::Nothing,
                "the no did something"
            );
            let rows = v.rows(&r, COLUMNS, &t);
            assert_eq!(rows[v.cursor(&rows)].spot, "decide-all");
        }
        // AND THE YES GIVES THEM ALL ONE.
        let (mut v, _) = View::fresh(Page::Import);
        onto(&mut v, &r, "decide-all");
        assert_eq!(press(&mut v, &r, Press::More), Errand::Nothing);
        press(&mut v, &r, Press::Down);
        assert_eq!(
            press(&mut v, &r, Press::Enter),
            Errand::Edit(Edit::DecideAll(Decision::Undecided))
        );
        // THEIRS ALIKE: the ring turns, unasked.
        for s in &mut r.staged {
            s.decision = Decision::Keep;
        }
        let (mut v, _) = View::fresh(Page::Import);
        onto(&mut v, &r, "decide-all");
        assert_eq!(act(&v, &r), Some(key::VIDEO_METRICS_ACT_TURNS));
        assert_eq!(
            press(&mut v, &r, Press::More),
            Errand::Edit(Edit::DecideAll(Decision::Discard))
        );
        assert_eq!(
            press(&mut v, &r, Press::Less),
            Errand::Edit(Edit::DecideAll(Decision::Undecided))
        );
    }

    /// FAILS IF: choosing the list, or where the channels go, does not bring
    /// up the file picker over the import page — under its title, its rows
    /// the view's own and each explained, its keys and hint its own —; going
    /// back does not bring the page back with the cursor where it was; a
    /// file it answers is not imported, a name it answers not exported to;
    /// or its ring keeps something besides lists, or a list besides a
    /// `.csv` its type says is one.
    #[test]
    fn the_picker_chooses_the_list_and_where_it_goes() {
        use ronoctua_keymap::{Key, Sym};
        use std::path::Path;
        let somewhere = PathBuf::from("/nowhere/list.csv");
        assert_eq!(
            picked(Picking::Import, std::slice::from_ref(&somewhere)),
            Errand::Edit(Edit::Import(somewhere.clone()))
        );
        assert_eq!(
            picked(Picking::Export, std::slice::from_ref(&somewhere)),
            Errand::Export(somewhere)
        );
        assert_eq!(picked(Picking::Import, &[]), Errand::Nothing);
        assert!(a_list(Path::new("/x/subscriptions.CSV"), None).is_some());
        assert!(a_list(Path::new("/x/renamed"), Some("text/csv")).is_some());
        assert!(a_list(Path::new("/x/notes.txt"), Some("text/plain")).is_none());
        let r = readings();
        let t = texts("en-GB");
        let dir = scratch("picker");
        let list = dir.join("list.csv");
        std::fs::write(&list, format!("{}\n", takeout::HEADER)).expect("a list");
        // UP OVER THE PAGE, under its title.
        let (mut v, _) = View::fresh(Page::Import);
        onto(&mut v, &r, "choose-list");
        assert_eq!(press(&mut v, &r, Press::Enter), Errand::Nothing);
        assert!(v.picking(), "no picker for the list");
        let rows = v.rows(&r, COLUMNS, &t);
        assert_eq!(
            rows[0].fields.first().map(|f| f.text.clone()),
            Some(t.t(key::VIDEO_METRICS_PICK_IMPORT))
        );
        let takes = v.takes(&rows);
        let mut explained = 0;
        for (n, row) in rows.iter().enumerate().filter(|(n, _)| takes[*n]) {
            let e = v
                .explained_at(&rows, n, None, &t)
                .unwrap_or_else(|| panic!("row {n} ({}) is not explained", row.spot));
            assert!(!e.name.is_empty(), "row {n} ({}) has no name", row.spot);
            assert_eq!(e.said[0].0, t.t(key::ABOUT_VIDEO_METRICS_PICK));
            explained += 1;
        }
        assert!(explained > 0, "no row of the picker takes a click");
        assert!(v.explained_cursor(&rows, &t).is_some());
        let taught: Vec<&str> = v.hint(&rows, &t).iter().filter_map(|p| p.id).collect();
        assert!(taught.contains(&"file-picker-enter"), "{taught:?}");
        assert!(
            v.picker_takes(Key::simple(Sym::Char('n'))),
            "a letter is not the picker's"
        );
        // BACK, the cursor where it was.
        assert_eq!(press(&mut v, &r, Press::Escape), Errand::Nothing);
        assert!(!v.picking(), "the way back left the picker up");
        let rows = v.rows(&r, COLUMNS, &t);
        assert_eq!(rows[v.cursor(&rows)].spot, "choose-list");
        // A LIST TYPED — pasted — and taken: imported.
        press(&mut v, &r, Press::Enter);
        assert!(v.picker_pasted(&list.to_string_lossy()));
        let Errand::Edit(Edit::Import(got)) = press(&mut v, &r, Press::Enter) else {
            panic!("the list was not taken");
        };
        assert_eq!(got.canonicalize().ok(), list.canonicalize().ok());
        assert!(!v.picking(), "the picker stayed up after its answer");
        // AND WHERE THE CHANNELS GO: a name, in the folder typed, `.csv` added.
        let (mut v, _) = View::fresh(Page::Import);
        onto(&mut v, &r, "save-list");
        press(&mut v, &r, Press::Enter);
        assert!(v.picking(), "no picker for the export");
        let rows = v.rows(&r, COLUMNS, &t);
        assert_eq!(
            rows[0].fields.first().map(|f| f.text.clone()),
            Some(t.t(key::VIDEO_METRICS_PICK_EXPORT))
        );
        assert!(v.picker_pasted(&dir.join("exported").to_string_lossy()));
        let Errand::Export(to) = press(&mut v, &r, Press::Enter) else {
            panic!("no place to export to was taken");
        };
        assert_eq!(
            to.file_name().and_then(|n| n.to_str()),
            Some("exported.csv")
        );
        assert_eq!(
            to.parent().and_then(|p| p.canonicalize().ok()),
            dir.canonicalize().ok()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
