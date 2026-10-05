//! Our own two files: `.rset`, media played together, and `.rqueue`, a queue.
//!
//! # What each one holds
//!
//! A **set** is several media on one clock — a film, its dubbing on the
//! headphones, a commentary track that starts two seconds late, two subtitle
//! files — each a [`Lane`] with its own start, trim, volume, screens and sound
//! output, plus how the pictures share a screen ([`Arrangement`]).
//!
//! A **queue** is a tree: single files, sets, named playlists that can be
//! folded, and streams ([`Item`]). Sets and playlists are stored INLINE, so a
//! queue file stands on its own; their `from` only says where they came from.
//!
//! # Why JSON with a `format` and a `version`
//!
//! Both are files a person keeps and moves between machines. The `format`
//! field is how a file that is not ours is told apart from one of ours that is
//! damaged — two different sentences — and the `version` is how a file
//! written by a newer player is refused with a sentence rather than read with
//! the wrong grammar. A field this version does not know is ignored, so an
//! older player still reads what a newer one adds without changing meaning.
//!
//! # Paths
//!
//! A path inside the saved file's folder is written RELATIVE to it, so a
//! folder of films with its `.rset` can be moved or copied to another disk and
//! still play. Every other path is written absolute. When an absolute path is
//! missing on read, the same file name beside the saved file is tried — the
//! one guess that is right when a person gathered everything into one folder
//! — and asking the disk is the caller's `present`, so this module still does
//! no I/O of its own beyond reading the file it is handed.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{ENTRY_CEILING, Entry, Where, percent_decoded, scheme_of};

/// The `format` of a `.rset`.
pub const SET_FORMAT: &str = "ronoctua-set";
/// The `format` of a `.rqueue`.
pub const QUEUE_FORMAT: &str = "ronoctua-queue";
/// The newest `.rset` this reads and the one it writes.
pub const SET_VERSION: u32 = 1;
/// The newest `.rqueue` this reads and the one it writes.
pub const QUEUE_VERSION: u32 = 1;
/// A set's extension.
pub const SET_EXTENSION: &str = "rset";
/// A queue's extension.
pub const QUEUE_EXTENSION: &str = "rqueue";

/// The most of one of our files that is read.
///
/// Larger than [`crate::BYTE_CEILING`] on purpose: a queue at its ceiling of
/// ten thousand leaves, each a set of a few lanes, is several megabytes of
/// honest JSON. Past this it is not a queue anybody wrote.
pub const OWN_BYTE_CEILING: u64 = 16 * 1024 * 1024;

/// The loudest a lane can be, in percent — the player's own volume ceiling.
pub const VOLUME_MOST_PERCENT: u8 = 130;

/// How many other addresses a stream keeps.
///
/// A radio's `.pls` lists a handful of relays; a list of a thousand is not a
/// list of relays, and trying each would be a minute of silence.
pub const ALTERNATES_MOST: usize = 16;

/// A lane's name for the rest of its set's life.
///
/// Never an index: a lane removed from the middle must not rename the ones
/// after it while a picture thread still holds the old number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LaneId(pub u32);

/// What a lane contributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LaneKind {
    /// A picture.
    Video,
    /// A sound.
    Audio,
    /// Words over a picture.
    Subtitle,
}

/// Which screens a picture is shown on.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Screens {
    /// Every screen.
    #[default]
    Every,
    /// Only these, by connector name (`DP-1`, `eDP-1`).
    Only(Vec<String>),
}

impl Screens {
    /// Whether a picture routed like this is shown on `screen`.
    ///
    /// `None` is a door that does not know its screen yet, and it shows
    /// everything: a picture that vanished because a compositor was slow to
    /// name an output would be a bug nobody could see the cause of.
    #[must_use]
    pub fn shows_on(&self, screen: Option<&str>) -> bool {
        match (self, screen) {
            (Self::Every, _) | (Self::Only(_), None) => true,
            (Self::Only(these), Some(screen)) => these.iter().any(|s| s == screen),
        }
    }
}

/// Where a subtitle lane's lines sit on its picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stack {
    /// At the foot, the usual place.
    #[default]
    Bottom,
    /// At the top, out of the way of another lane or of burnt-in text.
    Top,
}

/// One medium in a set: what plays, from where to where, and where it goes.
///
/// Seconds are on two clocks, and the names say which: `start_seconds` is on
/// the SET's clock (when the lane begins), `from_seconds` and `until_seconds`
/// are on the SOURCE's clock (which part of the file plays).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lane {
    /// Its name for the set's life.
    pub id: LaneId,
    /// What it contributes.
    pub kind: LaneKind,
    /// The file or address it plays.
    pub source: String,
    /// Which of the source's tracks of this kind (mpv's id); `None` is the
    /// source's own choice, and for a subtitle file, the file itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<i64>,
    /// Its title, as the source or the person gave it — a fact, worded by the
    /// door that shows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Its language, as declared (`en`, `pt-BR`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// When it begins on the set's clock, seconds, never below zero.
    #[serde(default)]
    pub start_seconds: f64,
    /// Where in the source it begins, seconds.
    #[serde(default)]
    pub from_seconds: f64,
    /// Where in the source it stops, seconds; `None` is the source's end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until_seconds: Option<f64>,
    /// It plays; off, it is kept and costs nothing.
    #[serde(default = "on")]
    pub enabled: bool,
    /// Its own volume, percent, under the player's.
    #[serde(default = "full")]
    pub volume_percent: u8,
    /// Its own mute.
    #[serde(default)]
    pub muted: bool,
    /// The screens its picture goes to.
    #[serde(default)]
    pub screens: Screens,
    /// The sound output it goes to, by PipeWire's `node.name`; `None` is the
    /// machine's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sink: Option<String>,
    /// For a subtitle lane: the picture it is drawn over; `None` is the set's
    /// first picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub over: Option<LaneId>,
    /// For a subtitle lane: where its lines sit.
    #[serde(default)]
    pub stack: Stack,
}

/// The default of [`Lane::enabled`].
const fn on() -> bool {
    true
}

/// The default of [`Lane::volume_percent`].
const fn full() -> u8 {
    100
}

impl Lane {
    /// A lane of `kind` playing all of `source` from the set's first second,
    /// with every setting at its default.
    #[must_use]
    pub fn new(id: LaneId, kind: LaneKind, source: impl Into<String>) -> Self {
        Self {
            id,
            kind,
            source: source.into(),
            track: None,
            title: None,
            lang: None,
            start_seconds: 0.0,
            from_seconds: 0.0,
            until_seconds: None,
            enabled: true,
            volume_percent: full(),
            muted: false,
            screens: Screens::Every,
            sink: None,
            over: None,
            stack: Stack::Bottom,
        }
    }
}

/// How several pictures share one screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Form {
    /// Whichever of side by side, stacked or grid shows the most picture.
    #[default]
    Auto,
    /// The first picture alone.
    Single,
    /// In a row.
    SideBySide,
    /// In a column.
    Stacked,
    /// In rows and columns.
    Grid,
    /// The first large, the others in a strip under it.
    Strip,
    /// The first filling, the others small in a corner over it.
    Corner,
}

impl Form {
    /// Every form, in the order a ring shows them.
    pub const ALL: [Self; 7] = [
        Self::Auto,
        Self::Single,
        Self::SideBySide,
        Self::Stacked,
        Self::Grid,
        Self::Strip,
        Self::Corner,
    ];
}

/// Which pictures a set shows and how.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Arrangement {
    /// The form.
    pub form: Form,
    /// The pictures in this order first; the rest follow in lane order.
    pub order: Vec<LaneId>,
    /// Pictures not shown.
    pub hidden: Vec<LaneId>,
    /// Sound lanes whose cover is shown as a picture.
    pub covers: Vec<LaneId>,
}

/// Several media on one clock.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Set {
    /// What the person called it, when they did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What plays.
    #[serde(default)]
    pub lanes: Vec<Lane>,
    /// How the pictures share a screen.
    #[serde(default)]
    pub arrangement: Arrangement,
}

impl Set {
    /// The lane named `id`.
    #[must_use]
    pub fn lane(&self, id: LaneId) -> Option<&Lane> {
        self.lanes.iter().find(|lane| lane.id == id)
    }

    /// The lane named `id`, to change.
    pub fn lane_mut(&mut self, id: LaneId) -> Option<&mut Lane> {
        self.lanes.iter_mut().find(|lane| lane.id == id)
    }

    /// A name no lane of this set has had: one past the largest.
    ///
    /// Past the largest and not the first gap, so a lane removed a moment ago
    /// is never confused with the one added after it.
    #[must_use]
    pub fn next_lane_id(&self) -> LaneId {
        LaneId(
            self.lanes
                .iter()
                .map(|lane| lane.id.0.saturating_add(1))
                .max()
                .unwrap_or(1),
        )
    }

    /// The arrangement without names of lanes that are gone, each once.
    ///
    /// What a writer calls before saving, so the reader's strictness —
    /// [`Set::check`] refuses an arrangement that names nothing — never meets
    /// a leftover of an ordinary removal.
    pub fn tidy(&mut self) {
        let ids: Vec<LaneId> = self.lanes.iter().map(|lane| lane.id).collect();
        for list in [
            &mut self.arrangement.order,
            &mut self.arrangement.hidden,
            &mut self.arrangement.covers,
        ] {
            let mut seen = Vec::with_capacity(list.len());
            list.retain(|id| {
                let keep = ids.contains(id) && !seen.contains(id);
                seen.push(*id);
                keep
            });
        }
    }

    /// Whether this is a set the player can play as it says.
    ///
    /// # Errors
    ///
    /// [`Refusal::Damaged`] when two lanes share a name, a number is out of
    /// its range (a negative start, a trim that ends before it begins, a
    /// volume past [`VOLUME_MOST_PERCENT`]), a subtitle lane is laid over
    /// something that is not a picture, or the arrangement names a lane that
    /// is not there. Nothing is quietly corrected: a value this does not
    /// understand is a file it does not understand.
    pub fn check(&self) -> Result<(), Refusal> {
        let mut ids = Vec::with_capacity(self.lanes.len());
        for lane in &self.lanes {
            if ids.contains(&lane.id) {
                return Err(Refusal::Damaged);
            }
            ids.push(lane.id);
        }
        let kind_of = |id: LaneId| self.lane(id).map(|lane| lane.kind);
        for lane in &self.lanes {
            let seconds_ok = |s: f64| s.is_finite() && s >= 0.0;
            let trim_ok = lane
                .until_seconds
                .is_none_or(|until| until.is_finite() && until > lane.from_seconds);
            let over_ok = match (lane.kind, lane.over) {
                (_, None) => true,
                (LaneKind::Subtitle, Some(over)) => kind_of(over) == Some(LaneKind::Video),
                (LaneKind::Video | LaneKind::Audio, Some(_)) => false,
            };
            if !seconds_ok(lane.start_seconds)
                || !seconds_ok(lane.from_seconds)
                || !trim_ok
                || lane.volume_percent > VOLUME_MOST_PERCENT
                || !over_ok
                || lane.source.trim().is_empty()
            {
                return Err(Refusal::Damaged);
            }
        }
        let a = &self.arrangement;
        let named = |id: &LaneId| ids.contains(id);
        let pictured =
            |id: &LaneId| matches!(kind_of(*id), Some(LaneKind::Video | LaneKind::Audio));
        let sounded = |id: &LaneId| kind_of(*id) == Some(LaneKind::Audio);
        if !a.order.iter().all(named)
            || !a.hidden.iter().all(pictured)
            || !a.covers.iter().all(sounded)
        {
            return Err(Refusal::Damaged);
        }
        Ok(())
    }
}

/// Where a stream came from, so a door can go back to it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// A station of the radio-browser directory.
    RadioBrowser {
        /// Its `stationuuid`.
        uuid: String,
    },
    /// A channel of the iptv-org directory.
    Iptv {
        /// Its channel id (`BBCNews.uk`).
        channel: String,
    },
    /// An item of the Internet Archive.
    Archive {
        /// Its identifier.
        identifier: String,
    },
    /// A YouTube video or live.
    #[serde(rename = "youtube")]
    YouTube {
        /// Its video id.
        id: String,
    },
    /// An address the person typed or pasted.
    #[default]
    Typed,
}

/// Something that plays from the network and is not a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stream {
    /// The address tried first.
    pub url: String,
    /// What it is called, when anything said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The `Referer` it must be asked with, for the hosts that check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrer: Option<String>,
    /// The `User-Agent` it must be asked with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Addresses tried, in order, when the one before fails.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternates: Vec<String>,
    /// Where it was found.
    #[serde(default)]
    pub origin: Origin,
    /// It is a broadcast with no end, rather than a recording.
    #[serde(default)]
    pub live: bool,
}

impl Stream {
    /// A stream at `url`, typed, with nothing else said.
    #[must_use]
    pub fn typed(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            title: None,
            referrer: None,
            user_agent: None,
            alternates: Vec::new(),
            origin: Origin::Typed,
            live: false,
        }
    }

    /// The stream a fetched list leads to: its first address, the others as
    /// alternates (at most [`ALTERNATES_MOST`]), and the first title it gave.
    ///
    /// What a radio's `.pls` or `.m3u` is: several relays of one broadcast.
    /// `None` when the list names no address — a list from the network that
    /// names only refused entries leads nowhere.
    #[must_use]
    pub fn of_list(list: &crate::List, live: bool) -> Option<Self> {
        let mut addresses = list.entries.iter().filter_map(|entry| match &entry.at {
            Where::Remote(address) => Some(address.clone()),
            Where::Local(_) | Where::Refused => None,
        });
        let url = addresses.next()?;
        Some(Self {
            url,
            title: list.entries.iter().find_map(|entry| entry.title.clone()),
            referrer: None,
            user_agent: None,
            alternates: addresses.take(ALTERNATES_MOST).collect(),
            origin: Origin::Typed,
            live,
        })
    }

    /// Every address, the first one first.
    pub fn addresses(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.url.as_str()).chain(self.alternates.iter().map(String::as_str))
    }
}

/// One entry of a queue file: a leaf, or a playlist of more.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Item {
    /// One file or address.
    Single {
        /// The path or URL.
        location: String,
        /// Its title, once known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// How long it runs, when known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seconds: Option<f64>,
    },
    /// Media played together.
    Set {
        /// The set, inline.
        #[serde(flatten)]
        set: Set,
        /// The `.rset` it came from.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
    },
    /// A named list, folded or not in the door.
    Playlist {
        /// Its name.
        name: String,
        /// The file or address it was read from.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
        /// What it holds.
        #[serde(default)]
        entries: Vec<Item>,
    },
    /// Something from the network.
    Stream(Stream),
}

/// A queue, as a file keeps it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct QueueFile {
    /// What the person called it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What it holds.
    #[serde(default)]
    pub entries: Vec<Item>,
}

/// One of our files, read.
#[derive(Debug, Clone, PartialEq)]
pub enum Own {
    /// A `.rset`.
    Set(Set),
    /// A `.rqueue`.
    Queue(QueueFile),
}

/// Which of our files a name says it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnKind {
    /// `.rset`.
    Set,
    /// `.rqueue`.
    Queue,
}

/// Why one of our files gave nothing — each a sentence on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "refusal")]
pub enum Refusal {
    /// It could not be opened or read, or written.
    Unreadable,
    /// It is larger than [`OWN_BYTE_CEILING`].
    TooLarge,
    /// It is not one of ours (another format, or not JSON at all).
    NotOurs,
    /// A newer player wrote it.
    Newer {
        /// The version it says it is.
        found: u32,
    },
    /// It is ours, and something in it is not what the format says.
    Damaged,
}

/// Which of our files a file's NAME says it is.
#[must_use]
pub fn kind_by_name(path: &Path) -> Option<OwnKind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        SET_EXTENSION => Some(OwnKind::Set),
        QUEUE_EXTENSION => Some(OwnKind::Queue),
        _ => None,
    }
}

/// A file of ours, from the disk.
///
/// Which one it is comes from its `format` field, not its name: a `.rset`
/// renamed by hand is still a set.
///
/// # Errors
///
/// Every [`Refusal`]; see [`parse`].
pub fn read(path: &Path) -> Result<Own, Refusal> {
    let raw = read_bounded(path, OWN_BYTE_CEILING)?;
    let folder = path.parent().unwrap_or(Path::new("."));
    parse(&raw, folder, &|p: &Path| p.exists())
}

/// At most `most` bytes of `path`, or why not.
fn read_bounded(path: &Path, most: u64) -> Result<Vec<u8>, Refusal> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).map_err(|_| Refusal::Unreadable)?;
    let mut raw = Vec::new();
    file.take(most.saturating_add(1))
        .read_to_end(&mut raw)
        .map_err(|_| Refusal::Unreadable)?;
    if u64::try_from(raw.len()).unwrap_or(u64::MAX) > most {
        return Err(Refusal::TooLarge);
    }
    Ok(raw)
}

/// A file of ours, already read: `raw` is its bytes and `folder` where it was.
///
/// `present` answers whether a path exists; it is asked only about an
/// absolute path, and only to decide whether to try the same name beside the
/// file (see the module's header). Hand in `&|_| true` to never ask.
///
/// # Errors
///
/// [`Refusal::NotOurs`] for anything that is not JSON with one of our two
/// formats, [`Refusal::Newer`] for a version past ours, and
/// [`Refusal::Damaged`] for ours with something wrong inside.
pub fn parse(raw: &[u8], folder: &Path, present: &dyn Fn(&Path) -> bool) -> Result<Own, Refusal> {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(raw) else {
        // HALF A FILE OF OURS is damaged, and a picture renamed `.rset` is
        // not ours: the one difference that can be seen is our own name in
        // it.
        let text = String::from_utf8_lossy(raw);
        let ours = [SET_FORMAT, QUEUE_FORMAT]
            .iter()
            .any(|format| text.contains(&format!("\"{format}\"")));
        return Err(if ours {
            Refusal::Damaged
        } else {
            Refusal::NotOurs
        });
    };
    let format = value.get("format").and_then(serde_json::Value::as_str);
    let newest = match format {
        Some(SET_FORMAT) => SET_VERSION,
        Some(QUEUE_FORMAT) => QUEUE_VERSION,
        _ => return Err(Refusal::NotOurs),
    };
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
        .ok_or(Refusal::Damaged)?;
    if version > newest {
        return Err(Refusal::Newer { found: version });
    }
    let beside = Beside { folder, present };
    if format == Some(SET_FORMAT) {
        let mut set: Set = serde_json::from_value(value).map_err(|_| Refusal::Damaged)?;
        beside.resolve_set(&mut set)?;
        set.check()?;
        Ok(Own::Set(set))
    } else {
        let mut queue: QueueFile = serde_json::from_value(value).map_err(|_| Refusal::Damaged)?;
        for item in &mut queue.entries {
            beside.resolve_item(item)?;
        }
        Ok(Own::Queue(queue))
    }
}

/// A set as the text of a `.rset` saved in `folder`.
///
/// Paths inside `folder` are written relative to it; see the module's header.
#[must_use]
pub fn set_to_text(set: &Set, folder: &Path) -> String {
    #[derive(Serialize)]
    struct Out<'a> {
        format: &'static str,
        version: u32,
        #[serde(flatten)]
        set: &'a Set,
    }
    let set = kept_set(set, folder);
    pretty(&Out {
        format: SET_FORMAT,
        version: SET_VERSION,
        set: &set,
    })
}

/// A queue as the text of a `.rqueue` saved in `folder`.
#[must_use]
pub fn queue_to_text(queue: &QueueFile, folder: &Path) -> String {
    #[derive(Serialize)]
    struct Out<'a> {
        format: &'static str,
        version: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<&'a str>,
        entries: Vec<Item>,
    }
    pretty(&Out {
        format: QUEUE_FORMAT,
        version: QUEUE_VERSION,
        name: queue.name.as_deref(),
        entries: queue
            .entries
            .iter()
            .map(|item| kept_item(item, folder))
            .collect(),
    })
}

/// Indented, because a person may open it in an editor, and with the final
/// newline an editor would add.
fn pretty(value: &impl Serialize) -> String {
    // A `Serialize` of plain structs, strings and finite numbers into a
    // `String` has no way to fail but a non-finite float, which
    // `serde_json` writes as `null` rather than erroring.
    let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
    text.push('\n');
    text
}

/// The set as it is written in `folder`: paths relative where they can be,
/// and an arrangement that names only lanes that are there.
fn kept_set(set: &Set, folder: &Path) -> Set {
    let mut set = set.clone();
    set.tidy();
    for lane in &mut set.lanes {
        lane.source = written(&lane.source, folder);
    }
    set
}

/// The item as it is written in `folder`.
fn kept_item(item: &Item, folder: &Path) -> Item {
    match item {
        Item::Single {
            location,
            title,
            seconds,
        } => Item::Single {
            location: written(location, folder),
            title: title.clone(),
            seconds: *seconds,
        },
        Item::Set { set, from } => Item::Set {
            set: kept_set(set, folder),
            from: from.as_deref().map(|from| written(from, folder)),
        },
        Item::Playlist {
            name,
            from,
            entries,
        } => Item::Playlist {
            name: name.clone(),
            from: from.as_deref().map(|from| written(from, folder)),
            entries: entries.iter().map(|item| kept_item(item, folder)).collect(),
        },
        Item::Stream(stream) => Item::Stream(stream.clone()),
    }
}

/// How `location` is written in a file saved in `folder`.
///
/// Relative only when it is INSIDE the folder — no `..`: a path that climbs
/// out breaks as soon as the folder moves without its parent, which is the
/// one move relative paths are for.
fn written(location: &str, folder: &Path) -> String {
    if scheme_of(location).is_some() {
        return location.to_owned();
    }
    let path = Path::new(location);
    if !path.is_absolute() {
        return location.to_owned();
    }
    match path.strip_prefix(folder) {
        Ok(inside)
            if inside.components().next().is_some()
                && inside
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))) =>
        {
            inside.to_string_lossy().into_owned()
        }
        _ => location.to_owned(),
    }
}

/// Where the paths of a file read in `folder` point.
struct Beside<'a> {
    /// The folder of the file being read.
    folder: &'a Path,
    /// Whether a path exists.
    present: &'a dyn Fn(&Path) -> bool,
}

impl Beside<'_> {
    /// A written path or address, as the player opens it.
    fn located(&self, written: &str) -> Result<String, Refusal> {
        if written.trim().is_empty() || written.contains(['\0', '\n', '\r']) {
            return Err(Refusal::Damaged);
        }
        if let Some(scheme) = scheme_of(written) {
            if !scheme.eq_ignore_ascii_case("file") {
                return Ok(written.to_owned());
            }
            let path = percent_decoded(&written[scheme.len() + 3..]);
            return Ok(self.found(Path::new(&path)));
        }
        let path = Path::new(written);
        if path.is_absolute() {
            return Ok(self.found(path));
        }
        Ok(self.folder.join(path).to_string_lossy().into_owned())
    }

    /// An absolute path, or the same name beside the file when it is gone.
    fn found(&self, path: &Path) -> String {
        if !(self.present)(path)
            && let Some(name) = path.file_name()
        {
            let beside: PathBuf = self.folder.join(name);
            if (self.present)(&beside) {
                return beside.to_string_lossy().into_owned();
            }
        }
        path.to_string_lossy().into_owned()
    }

    /// Every path of a set.
    fn resolve_set(&self, set: &mut Set) -> Result<(), Refusal> {
        for lane in &mut set.lanes {
            lane.source = self.located(&lane.source)?;
        }
        Ok(())
    }

    /// Every path of an item, and its streams checked.
    fn resolve_item(&self, item: &mut Item) -> Result<(), Refusal> {
        match item {
            Item::Single {
                location, seconds, ..
            } => {
                if seconds.is_some_and(|s| !s.is_finite() || s < 0.0) {
                    return Err(Refusal::Damaged);
                }
                *location = self.located(location)?;
            }
            Item::Set { set, from } => {
                self.resolve_set(set)?;
                set.check()?;
                if let Some(from) = from {
                    *from = self.located(from)?;
                }
            }
            Item::Playlist {
                name,
                from,
                entries,
            } => {
                if name.trim().is_empty() {
                    return Err(Refusal::Damaged);
                }
                if let Some(from) = from {
                    *from = self.located(from)?;
                }
                for item in entries {
                    self.resolve_item(item)?;
                }
            }
            Item::Stream(stream) => {
                let remote = |address: &str| {
                    scheme_of(address).is_some_and(|s| !s.eq_ignore_ascii_case("file"))
                        && !address.contains(['\0', '\n', '\r'])
                };
                // A STREAM IS NEVER A FILE HERE: a queue somebody sent could
                // otherwise name a private file as a "stream".
                if !stream.addresses().all(remote) || stream.alternates.len() > ALTERNATES_MOST {
                    return Err(Refusal::Damaged);
                }
            }
        }
        Ok(())
    }
}

impl Own {
    /// What it names, flat, as the file manager's preview lists a playlist.
    ///
    /// A set is ONE entry — its media play together, not one after another —
    /// under its name, pointing at its first sound or picture. A playlist is
    /// what it holds. At most [`ENTRY_CEILING`].
    #[must_use]
    pub fn listing(&self) -> Vec<Entry> {
        let mut out = Vec::new();
        match self {
            Self::Set(set) => listed_set(set, &mut out),
            Self::Queue(queue) => {
                for item in &queue.entries {
                    listed(item, &mut out);
                }
            }
        }
        out.truncate(ENTRY_CEILING);
        out
    }
}

/// A location as the preview's [`Where`].
fn where_of(location: &str) -> Where {
    if scheme_of(location).is_some() {
        Where::Remote(location.to_owned())
    } else {
        Where::Local(PathBuf::from(location))
    }
}

/// A set as one listed entry.
fn listed_set(set: &Set, out: &mut Vec<Entry>) {
    let lead = set
        .lanes
        .iter()
        .find(|lane| lane.kind != LaneKind::Subtitle)
        .or_else(|| set.lanes.first());
    if let Some(lead) = lead {
        out.push(Entry {
            at: where_of(&lead.source),
            title: set.name.clone(),
            seconds: None,
            written: lead.source.clone(),
            attributes: None,
        });
    }
}

/// An item's leaves, listed.
fn listed(item: &Item, out: &mut Vec<Entry>) {
    if out.len() > ENTRY_CEILING {
        return;
    }
    match item {
        Item::Single {
            location,
            title,
            seconds,
        } => out.push(Entry {
            at: where_of(location),
            title: title.clone(),
            seconds: *seconds,
            written: location.clone(),
            attributes: None,
        }),
        Item::Set { set, .. } => listed_set(set, out),
        Item::Playlist { entries, .. } => {
            for item in entries {
                listed(item, out);
            }
        }
        Item::Stream(stream) => out.push(Entry {
            at: Where::Remote(stream.url.clone()),
            title: stream.title.clone(),
            seconds: None,
            written: stream.url.clone(),
            attributes: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The folder every fixture is saved in.
    fn folder() -> &'static Path {
        Path::new("/films/evening")
    }

    /// Everything exists.
    fn everything(_: &Path) -> bool {
        true
    }

    /// A set with one lane of each kind and every field away from its
    /// default somewhere.
    fn a_set() -> Set {
        let mut film = Lane::new(LaneId(1), LaneKind::Video, "/films/evening/film.mkv");
        film.screens = Screens::Only(vec!["DP-1".to_owned()]);
        let mut dub = Lane::new(LaneId(2), LaneKind::Audio, "/films/evening/film.mkv");
        dub.track = Some(2);
        dub.lang = Some("pt-BR".to_owned());
        dub.sink = Some("bluez_output.00_11_22_33_44_55.1".to_owned());
        dub.volume_percent = 80;
        let mut commentary = Lane::new(LaneId(3), LaneKind::Audio, "/elsewhere/commentary.opus");
        commentary.start_seconds = 2.0;
        commentary.from_seconds = 1.5;
        commentary.until_seconds = Some(600.0);
        commentary.muted = true;
        commentary.title = Some("Director".to_owned());
        let mut words = Lane::new(
            LaneId(5),
            LaneKind::Subtitle,
            "/films/evening/subs/film.en.srt",
        );
        words.over = Some(LaneId(1));
        words.stack = Stack::Top;
        words.enabled = false;
        Set {
            name: Some("Evening".to_owned()),
            lanes: vec![film, dub, commentary, words],
            arrangement: Arrangement {
                form: Form::SideBySide,
                order: vec![LaneId(1)],
                hidden: vec![],
                covers: vec![LaneId(3)],
            },
        }
    }

    /// FAILS IF: a set saved and read back is not the same set.
    #[test]
    fn a_set_comes_back_as_it_was_saved() {
        let set = a_set();
        let text = set_to_text(&set, folder());
        let back = parse(text.as_bytes(), folder(), &everything).expect("a set");
        assert_eq!(back, Own::Set(set));
    }

    /// FAILS IF: a queue saved and read back is not the same queue — every
    /// kind of entry, a playlist inside a playlist, and a stream with its
    /// origin.
    #[test]
    fn a_queue_comes_back_as_it_was_saved() {
        let mut radio = Stream::typed("https://ice.example/live.mp3");
        radio.title = Some("Rádio".to_owned());
        radio.alternates = vec!["https://ice2.example/live.mp3".to_owned()];
        radio.origin = Origin::RadioBrowser {
            uuid: "9617a958-0601-11e8-ae97-52543be04c81".to_owned(),
        };
        radio.live = true;
        let mut tube = Stream::typed("https://www.youtube.com/watch?v=abc");
        tube.origin = Origin::YouTube {
            id: "abc".to_owned(),
        };
        let queue = QueueFile {
            name: Some("Sunday".to_owned()),
            entries: vec![
                Item::Single {
                    location: "/films/evening/intro.flac".to_owned(),
                    title: Some("Intro".to_owned()),
                    seconds: Some(61.5),
                },
                Item::Set {
                    set: a_set(),
                    from: Some("/films/evening/evening.rset".to_owned()),
                },
                Item::Playlist {
                    name: "Album".to_owned(),
                    from: Some("/music/album/album.m3u".to_owned()),
                    entries: vec![
                        Item::Single {
                            location: "/music/album/01.flac".to_owned(),
                            title: None,
                            seconds: None,
                        },
                        Item::Playlist {
                            name: "Bonus".to_owned(),
                            from: None,
                            entries: vec![Item::Stream(tube)],
                        },
                    ],
                },
                Item::Stream(radio),
            ],
        };
        let text = queue_to_text(&queue, folder());
        let back = parse(text.as_bytes(), folder(), &everything).expect("a queue");
        assert_eq!(back, Own::Queue(queue));
    }

    /// FAILS IF: a path inside the saved file's folder is written absolute,
    /// or one outside it relative — the first breaks a moved folder, the
    /// second breaks the moment the folder moves without its parent.
    #[test]
    fn paths_inside_the_folder_are_written_relative_and_the_rest_absolute() {
        let text = set_to_text(&a_set(), folder());
        assert!(text.contains("\"source\": \"film.mkv\""), "{text}");
        assert!(text.contains("\"source\": \"subs/film.en.srt\""), "{text}");
        assert!(
            text.contains("\"source\": \"/elsewhere/commentary.opus\""),
            "{text}"
        );
        assert!(!text.contains(".."), "{text}");

        // A NEIGHBOUR THAT SHARES A PREFIX is not inside: `/films/evening2`
        // is not `/films/evening`.
        assert_eq!(
            written("/films/evening2/x.mkv", folder()),
            "/films/evening2/x.mkv"
        );
        // A PATH THAT CLIMBS OUT is not inside either.
        assert_eq!(
            written("/films/evening/../x.mkv", folder()),
            "/films/evening/../x.mkv"
        );
        // AN ADDRESS IS AN ADDRESS.
        assert_eq!(
            written("https://example.invalid/a.mp4", folder()),
            "https://example.invalid/a.mp4"
        );

        // AND A RELATIVE ONE IS READ BACK AGAINST THE FOLDER IT WAS FOUND IN,
        // wherever that is now.
        let Own::Set(moved) =
            parse(text.as_bytes(), Path::new("/mnt/usb/evening"), &everything).expect("a set")
        else {
            panic!("a set");
        };
        assert_eq!(moved.lanes[0].source, "/mnt/usb/evening/film.mkv");
        assert_eq!(moved.lanes[3].source, "/mnt/usb/evening/subs/film.en.srt");
        assert_eq!(moved.lanes[2].source, "/elsewhere/commentary.opus");
    }

    /// FAILS IF: a file whose absolute path is gone is not looked for beside
    /// the saved file, or is looked for when it is where it says.
    #[test]
    fn a_missing_absolute_path_is_looked_for_beside_the_file() {
        let text = br#"{"format":"ronoctua-queue","version":1,"entries":[
            {"single":{"location":"/old/disk/gone.flac"}},
            {"single":{"location":"/old/disk/nowhere.flac"}},
            {"single":{"location":"file:///old/disk/gone.flac"}}
        ]}"#;
        let present = |p: &Path| p == Path::new("/mnt/usb/gone.flac");
        let Own::Queue(queue) = parse(text, Path::new("/mnt/usb"), &present).expect("a queue")
        else {
            panic!("a queue");
        };
        let locations: Vec<&str> = queue
            .entries
            .iter()
            .map(|item| match item {
                Item::Single { location, .. } => location.as_str(),
                _ => panic!("singles"),
            })
            .collect();
        assert_eq!(
            locations,
            vec![
                "/mnt/usb/gone.flac",
                // NOWHERE TO BE FOUND: kept as written, and playing it says so.
                "/old/disk/nowhere.flac",
                "/mnt/usb/gone.flac",
            ]
        );
        // AND A PATH THAT IS THERE IS NEVER SECOND-GUESSED.
        let Own::Queue(queue) = parse(text, Path::new("/mnt/usb"), &everything).expect("a queue")
        else {
            panic!("a queue");
        };
        assert!(matches!(
            &queue.entries[0],
            Item::Single { location, .. } if location == "/old/disk/gone.flac"
        ));
    }

    /// FAILS IF: a newer file is read with this version's grammar, or a field
    /// this version does not know stops a file from being read.
    #[test]
    fn a_newer_file_is_refused_and_an_unknown_field_is_ignored() {
        let newer = br#"{"format":"ronoctua-set","version":2,"lanes":[]}"#;
        assert_eq!(
            parse(newer, folder(), &everything),
            Err(Refusal::Newer { found: 2 })
        );
        let richer = br#"{"format":"ronoctua-set","version":1,"colour":"teal",
            "lanes":[{"id":1,"kind":"video","source":"a.mkv","sparkle":true}],
            "arrangement":{"form":"grid","spin":3}}"#;
        let Own::Set(set) = parse(richer, folder(), &everything).expect("a set") else {
            panic!("a set");
        };
        assert_eq!(set.arrangement.form, Form::Grid);
        // AND WHAT WAS NOT SAID HAS ITS DEFAULT.
        let lane = &set.lanes[0];
        assert_eq!(lane.source, "/films/evening/a.mkv");
        assert!(lane.enabled);
        assert_eq!(lane.volume_percent, 100);
        assert_eq!(lane.screens, Screens::Every);
    }

    /// FAILS IF: a file that is not ours is taken for a damaged one of ours,
    /// or the other way round — two different sentences.
    #[test]
    fn a_file_that_is_not_ours_is_told_from_a_damaged_one() {
        let not_ours: [&[u8]; 4] = [
            b"ID3\x03\x00\x00\x00binary",
            br#"{"format":"something-else","version":1}"#,
            br#"[1,2,3]"#,
            b"",
        ];
        for raw in not_ours {
            assert_eq!(
                parse(raw, folder(), &everything),
                Err(Refusal::NotOurs),
                "{}",
                String::from_utf8_lossy(raw)
            );
        }
        let damaged: [&[u8]; 4] = [
            // CUT IN HALF while being written.
            br#"{"format":"ronoctua-set","version":1,"lanes":[{"id":1,"#,
            // NO VERSION.
            br#"{"format":"ronoctua-set","lanes":[]}"#,
            // A LANE WITH NO SOURCE.
            br#"{"format":"ronoctua-set","version":1,"lanes":[{"id":1,"kind":"video"}]}"#,
            // A KIND THAT DOES NOT EXIST.
            br#"{"format":"ronoctua-set","version":1,"lanes":[{"id":1,"kind":"smell","source":"a"}]}"#,
        ];
        for raw in damaged {
            assert_eq!(
                parse(raw, folder(), &everything),
                Err(Refusal::Damaged),
                "{}",
                String::from_utf8_lossy(raw)
            );
        }
    }

    /// FAILS IF: a set whose numbers or names do not mean anything is played
    /// as if they did.
    #[test]
    fn a_set_that_says_nothing_sensible_is_refused_and_not_corrected() {
        let broken: [fn(&mut Set); 8] = [
            |s| s.lanes[1].id = LaneId(1),
            |s| s.lanes[0].start_seconds = -1.0,
            |s| s.lanes[2].until_seconds = Some(1.0),
            |s| s.lanes[1].volume_percent = 200,
            // A SUBTITLE OVER A SOUND is over nothing.
            |s| s.lanes[3].over = Some(LaneId(2)),
            // A PICTURE OVER A PICTURE is not what `over` means.
            |s| s.lanes[0].over = Some(LaneId(1)),
            |s| s.arrangement.order.push(LaneId(9)),
            // A COVER OF A PICTURE is not a cover.
            |s| s.arrangement.covers.push(LaneId(1)),
        ];
        for (n, breaking) in broken.iter().enumerate() {
            let mut set = a_set();
            breaking(&mut set);
            assert_eq!(set.check(), Err(Refusal::Damaged), "case {n}");
        }
        assert_eq!(a_set().check(), Ok(()));
    }

    /// FAILS IF: a queue somebody sent can name a file on this machine as a
    /// "stream".
    #[test]
    fn a_stream_never_names_a_file_here() {
        for url in ["file:///home/me/.ssh/id_ed25519", "/home/me/secret.wav", ""] {
            let text = format!(
                r#"{{"format":"ronoctua-queue","version":1,"entries":[{{"stream":{{"url":"{url}"}}}}]}}"#
            );
            assert_eq!(
                parse(text.as_bytes(), folder(), &everything),
                Err(Refusal::Damaged),
                "{url}"
            );
        }
    }

    /// FAILS IF: the words in the file stop being the ones the format
    /// documents — another program, or a person with an editor, reads them.
    #[test]
    fn the_file_says_what_the_format_documents() {
        let mut stream = Stream::typed("https://x.example/a.m3u8");
        stream.origin = Origin::Iptv {
            channel: "BBCNews.uk".to_owned(),
        };
        let queue = QueueFile {
            name: None,
            entries: vec![
                Item::Set {
                    set: a_set(),
                    from: None,
                },
                Item::Stream(stream),
                Item::Playlist {
                    name: "P".to_owned(),
                    from: None,
                    entries: vec![],
                },
            ],
        };
        let text = queue_to_text(&queue, folder());
        for needle in [
            r#""format": "ronoctua-queue""#,
            r#""version": 1"#,
            r#""set": {"#,
            r#""lanes": ["#,
            r#""form": "side-by-side""#,
            r#""kind": "subtitle""#,
            r#""start_seconds": 2.0"#,
            r#""volume_percent": 80"#,
            r#""only": ["#,
            r#""stack": "top""#,
            r#""iptv": {"#,
            r#""playlist": {"#,
        ] {
            assert!(text.contains(needle), "{needle} is missing from:\n{text}");
        }
        assert!(
            set_to_text(&a_set(), folder())
                .starts_with("{\n  \"format\": \"ronoctua-set\",\n  \"version\": 1,"),
            "the format is not what a reader sees first"
        );
    }

    /// FAILS IF: a file is read past its ceiling, or a small one is refused.
    #[test]
    fn a_file_is_read_within_its_ceiling() {
        let dir = std::env::temp_dir().join(format!("ronoctua-own-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temporary folder");
        let path = dir.join("evening.rset");
        std::fs::write(&path, set_to_text(&a_set(), &dir)).expect("written");
        assert_eq!(read_bounded(&path, 10), Err(Refusal::TooLarge));
        assert!(read_bounded(&path, OWN_BYTE_CEILING).is_ok());
        let Own::Set(set) = read(&path).expect("a set") else {
            panic!("a set");
        };
        assert_eq!(set.name.as_deref(), Some("Evening"));
        assert_eq!(read(&dir.join("absent.rset")), Err(Refusal::Unreadable));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    /// FAILS IF: saving keeps an arrangement's name of a lane that is gone —
    /// the reader would then refuse what the writer wrote.
    #[test]
    fn what_is_saved_is_what_is_read() {
        let mut set = a_set();
        set.lanes.retain(|lane| lane.id != LaneId(1));
        set.lanes[2].over = None;
        set.arrangement.hidden = vec![LaneId(3), LaneId(3)];
        let text = set_to_text(&set, folder());
        let Own::Set(back) = parse(text.as_bytes(), folder(), &everything).expect("a set") else {
            panic!("a set");
        };
        assert!(back.arrangement.order.is_empty(), "{:?}", back.arrangement);
        assert_eq!(back.arrangement.hidden, vec![LaneId(3)]);
        assert_eq!(set.next_lane_id(), LaneId(6));
    }

    /// FAILS IF: the preview lists a set as its lanes one after another, or
    /// loses what a playlist inside the queue holds.
    #[test]
    fn the_preview_lists_a_set_as_one_and_a_playlist_as_its_entries() {
        let queue = Own::Queue(QueueFile {
            name: None,
            entries: vec![
                Item::Set {
                    set: a_set(),
                    from: None,
                },
                Item::Playlist {
                    name: "A".to_owned(),
                    from: None,
                    entries: vec![Item::Single {
                        location: "/a/1.flac".to_owned(),
                        title: None,
                        seconds: Some(3.0),
                    }],
                },
                Item::Stream(Stream::typed("https://x.example/live")),
            ],
        });
        let listed = queue.listing();
        assert_eq!(listed.len(), 3, "{listed:#?}");
        assert_eq!(listed[0].title.as_deref(), Some("Evening"));
        assert_eq!(
            listed[0].at,
            Where::Local(PathBuf::from("/films/evening/film.mkv"))
        );
        assert_eq!(listed[1].seconds, Some(3.0));
        assert!(matches!(listed[2].at, Where::Remote(_)));
    }

    /// FAILS IF: a relay list stops becoming one stream with its relays, or
    /// a list naming no address becomes a stream.
    #[test]
    fn a_relay_list_is_one_stream() {
        let list = crate::parse_text(
            "[playlist]\nFile1=https://a.example/x\nTitle1=Jazz\nFile2=https://b.example/x\n",
            crate::Kind::Pls,
            crate::Base::Address("https://radio.example/x.pls"),
        )
        .expect("a list");
        let stream = Stream::of_list(&list, true).expect("a stream");
        assert_eq!(
            stream.addresses().collect::<Vec<_>>(),
            vec!["https://a.example/x", "https://b.example/x"]
        );
        assert_eq!(stream.title.as_deref(), Some("Jazz"));
        assert!(stream.live);
        let local = crate::parse_text("a.mp3\n", crate::Kind::M3u, crate::Base::Folder(folder()))
            .expect("a list");
        assert_eq!(Stream::of_list(&local, true), None);
    }

    /// FAILS IF: a name stops saying which of our files it is.
    #[test]
    fn a_name_says_which_of_ours() {
        assert_eq!(kind_by_name(Path::new("a/b.rset")), Some(OwnKind::Set));
        assert_eq!(kind_by_name(Path::new("b.RQUEUE")), Some(OwnKind::Queue));
        assert_eq!(kind_by_name(Path::new("b.json")), None);
    }

    /// FAILS IF: a picture routed to some screens is shown on others, or
    /// vanishes from a door that does not know its screen yet.
    #[test]
    fn a_picture_goes_where_it_is_routed() {
        let only = Screens::Only(vec!["DP-1".to_owned()]);
        assert!(only.shows_on(Some("DP-1")));
        assert!(!only.shows_on(Some("HDMI-A-1")));
        assert!(only.shows_on(None));
        assert!(Screens::Every.shows_on(Some("HDMI-A-1")));
    }
}
