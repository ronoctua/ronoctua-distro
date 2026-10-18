//! FAST-ACCESS — the search of everything (`LEADER d`): applications,
//! commands, files and what they say, manual pages, programs to install,
//! videos, icons, sums, words and the web, in one box.
//!
//! # What this file is
//!
//! The MODEL, which no door owns: what is typed and what it means
//! ([`Query`], [`Scope`]), what came back ([`Section`]s of [`Hit`]s, each
//! filled by its own source and at its own pace), where the cursor is — by
//! the hit's identity, [`HitId`], never by a line — what a key does
//! ([`Press`] → [`Errand`]s) and what the person chose (`fast-access.json`,
//! [`Settings`]). The footer's region (`fast_access_region`), the window and the
//! column (`crate::fast_access_window`) hand it a key or a click the same way
//! and paint what it draws (`crate::fast_access_rows`,
//! `crate::fast_access_detail`); the threads that fill it are
//! `crate::fast_access_search`'s.
//!
//! # The pipeline, per key
//!
//! A key changes what is typed: the prefix is read into a [`Scope`], the
//! GENERATION moves, and the sources that live in this process — the
//! applications, the commands with their keys, the icons, a sum — answer on
//! the spot (a few hundred microseconds; [`View::retyped`]). The index is
//! asked on one thread where the last question wins, and the network after a
//! pause of [`ONLINE_PAUSE`] with no key, one thread per source, each with a
//! stop the next key pulls (`fast_access_search::Feed`). What arrives slots
//! into its section — an answer of an older generation is dropped — and the
//! page never moves for it: the cursor keeps its hit and the first line shown
//! stays where it was ([`View::arrived`]).
//!
//! # A prefix narrows
//!
//! A slash and a letter at the start ([`PREFIXES`]): `/a` audio, `/y`
//! YouTube, `/t en …` a translation into English, and the rest. A slash alone
//! lists them; "2+2" answers with none. The letters are code — they are what
//! is typed — and what each searches is a sentence of the `.ftl`.

use std::path::PathBuf;
use std::sync::Arc;

use ronoctua_iconset::Collection;
use ronoctua_keymap::{Key, Sym};

/// The search's window, which tiles — its `app_id` owned by the table of the
/// windows that open floating (`crate::floating`).
pub const APP_ID: &str = crate::floating::APP_ID_FAST_ACCESS;

/// The search's floating window (`LEADER SPC`), floated by its row of the
/// table (`crate::floating`).
pub const APP_ID_FLOAT: &str = crate::floating::APP_ID_FAST_ACCESS_FLOAT;

/// How long the typing must pause before the network is asked: a person
/// typing a word sends nothing until the word is there.
pub const ONLINE_PAUSE: std::time::Duration = std::time::Duration::from_millis(250);

/// The settings' file, under the configuration root.
pub const FILE: &str = "fast-access.json";

/// The keys the header teaches — `(notation, id)`, each id a text key with an
/// `about-<id>` sentence. Each is also a click (`chrome::hint_of`).
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("RET", "fast-access-enter"),
    ("C-RET", "fast-access-secondary"),
    ("C-c", "fast-access-copy"),
    ("TAB", "fast-access-detail"),
    ("C-w", "fast-access-whole"),
    ("C-u", "fast-access-clear"),
];

/// The key of the YouTube scope alone: Shorts shown or left out.
pub const OF_YOUTUBE: &[(&str, &str)] = &[("C-s", "fast-access-shorts")];

/// The key of a strip of icons, while the cursor is on one.
pub const OF_A_STRIP: &[(&str, &str)] = &[("<right>", "fast-access-page")];

/// Every command this view reaches, for `bindings::reachable`.
pub const COMMANDS: [&str; 8] = [
    "fast-access-open",
    "fast-access-window",
    "fast-access-column",
    "fast-access-float",
    "fast-access-launch",
    "fast-access-open-path",
    "fast-access-reveal",
    "fast-access-man",
];

/// Every command of the registry this view reaches — its doors, and what a
/// door of it hands the footer. Its keys (`OF_THE_VIEW`) are the view's own,
/// read by `press_of_key`, and no command: they explain themselves with
/// `about-<id>`.
pub fn reached() -> impl Iterator<Item = &'static str> {
    COMMANDS.into_iter()
}

// ---------------------------------------------------------------------------
// What is typed, and what it means.
// ---------------------------------------------------------------------------

/// What a search is narrowed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Everything that answers in a moment: no network.
    All,
    /// Audio files (`/a`).
    Audio,
    /// Video files (`/v`).
    Video,
    /// The video catalogue and YouTube (`/y`).
    YouTube,
    /// Pictures (`/i`).
    Images,
    /// Documents (`/o`).
    Documents,
    /// Files and folders by name only (`/f`).
    Names,
    /// What files say (`/c`).
    Contents,
    /// Manual pages (`/m`).
    Manuals,
    /// Programs to install (`/p`).
    Programs,
    /// The desktop's commands (`/:`).
    Commands,
    /// The web (`/w`).
    Web,
    /// What a word means (`/d`).
    Dictionary,
    /// Other words for a word (`/s`).
    Synonyms,
    /// A translation (`/t [xx]`).
    Translate,
    /// Icons and characters (`/g`).
    Icons,
    /// Sums and units (`/=`).
    Math,
    /// Currencies (`/$`).
    Currency,
    /// Time zones (`/z`).
    Zones,
    /// What was opened from here (`/r`).
    Recent,
    /// A slash alone: every way to narrow.
    Prefixes,
}

/// What starts a prefix — and, alone, lists them all.
pub const SLASH: &str = "/";

/// The prefixes, in the order the slash lists them: the letter typed after
/// it, and the scope.
pub const PREFIXES: [(&str, Scope); 19] = [
    ("a", Scope::Audio),
    ("v", Scope::Video),
    ("y", Scope::YouTube),
    ("i", Scope::Images),
    ("o", Scope::Documents),
    ("f", Scope::Names),
    ("c", Scope::Contents),
    ("m", Scope::Manuals),
    ("p", Scope::Programs),
    (":", Scope::Commands),
    ("w", Scope::Web),
    ("d", Scope::Dictionary),
    ("s", Scope::Synonyms),
    ("t", Scope::Translate),
    ("g", Scope::Icons),
    ("=", Scope::Math),
    ("$", Scope::Currency),
    ("z", Scope::Zones),
    ("r", Scope::Recent),
];

impl Scope {
    /// Every scope, for the tests that walk the words.
    #[cfg(test)]
    pub const ALL: [Self; 21] = [
        Self::All,
        Self::Audio,
        Self::Video,
        Self::YouTube,
        Self::Images,
        Self::Documents,
        Self::Names,
        Self::Contents,
        Self::Manuals,
        Self::Programs,
        Self::Commands,
        Self::Web,
        Self::Dictionary,
        Self::Synonyms,
        Self::Translate,
        Self::Icons,
        Self::Math,
        Self::Currency,
        Self::Zones,
        Self::Recent,
        Self::Prefixes,
    ];

    /// Its kebab-case name: `fast-access-scope-<id>`, and `-does` after it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::YouTube => "youtube",
            Self::Images => "images",
            Self::Documents => "documents",
            Self::Names => "names",
            Self::Contents => "contents",
            Self::Manuals => "manuals",
            Self::Programs => "programs",
            Self::Commands => "commands",
            Self::Web => "web",
            Self::Dictionary => "dictionary",
            Self::Synonyms => "synonyms",
            Self::Translate => "translate",
            Self::Icons => "icons",
            Self::Math => "math",
            Self::Currency => "currency",
            Self::Zones => "zones",
            Self::Recent => "recent",
            Self::Prefixes => "prefixes",
        }
    }

    /// The letter after the slash that narrows to it.
    #[must_use]
    pub fn letter(self) -> Option<&'static str> {
        PREFIXES.iter().find(|(_, s)| *s == self).map(|(l, _)| *l)
    }

    /// What it searches, in the reader's words — its prefix, when the
    /// sentence shows one, from the code (`{ $prefix }`): the letters are
    /// what is typed, the sentences what the `.ftl` says about them.
    #[must_use]
    pub fn does(self, texts: &ronoctua_i18n::Texts) -> String {
        let prefix = self
            .letter()
            .map(|l| format!("{SLASH}{l}"))
            .unwrap_or_default();
        texts.t_with(
            &format!("fast-access-scope-{}-does", self.id()),
            &[("prefix", &prefix)],
        )
    }

    /// Whether its answer takes the WHOLE area — no detail beside it: the
    /// grid of videos, a card of words, the slash's list.
    #[must_use]
    pub const fn whole(self) -> bool {
        matches!(
            self,
            Self::YouTube | Self::Dictionary | Self::Synonyms | Self::Translate | Self::Prefixes
        )
    }

    /// Whether it asks the network (after the pause).
    #[must_use]
    pub const fn online(self) -> bool {
        matches!(
            self,
            Self::YouTube
                | Self::Web
                | Self::Dictionary
                | Self::Synonyms
                | Self::Translate
                | Self::Currency
        )
    }
}

/// What is typed, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// What it is narrowed to.
    pub scope: Scope,
    /// The words, the prefix taken off (and the language, for `/t xx`).
    pub words: String,
    /// The language a translation goes into, when `/t` named one.
    pub tongue: Option<String>,
}

/// `text` read: a prefix at its start narrows it. Pure.
#[must_use]
pub fn parse(text: &str) -> Query {
    let trimmed = text.trim_start();
    let plain = || Query {
        scope: Scope::All,
        words: text.trim().to_owned(),
        tongue: None,
    };
    let Some(rest) = trimmed.strip_prefix('/') else {
        return plain();
    };
    if rest.trim().is_empty() {
        return Query {
            scope: Scope::Prefixes,
            words: String::new(),
            tongue: None,
        };
    }
    // THE LETTER, then a space or the end: `/archive` is a word with a slash
    // in front (a path being typed), not `/a` and `rchive`.
    let mut chars = rest.char_indices();
    let Some((_, letter)) = chars.next() else {
        return plain();
    };
    let after = chars.next().map_or(rest.len(), |(at, _)| at);
    let tail = &rest[after..];
    if !(tail.is_empty() || tail.starts_with(char::is_whitespace)) {
        return plain();
    }
    let Some((_, scope)) = PREFIXES.iter().find(|(l, _)| l.starts_with(letter)) else {
        return plain();
    };
    let words = tail.trim();
    if *scope == Scope::Translate {
        // `/t en the words`: a first word that is a language's code sends
        // the translation there — when words follow it.
        let mut split = words.splitn(2, char::is_whitespace);
        if let (Some(code), Some(more)) = (split.next(), split.next())
            && ronoctua_words::Tongue::new(code).is_some()
            && code.chars().count() <= 3
            && !more.trim().is_empty()
        {
            return Query {
                scope: *scope,
                words: more.trim().to_owned(),
                tongue: Some(code.to_owned()),
            };
        }
    }
    Query {
        scope: *scope,
        words: words.to_owned(),
        tongue: None,
    }
}

// ---------------------------------------------------------------------------
// What came back.
// ---------------------------------------------------------------------------

/// A section of the results — each filled by its own source, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SectionId {
    /// The slash's list.
    Prefixes,
    /// A sum's answer.
    Math,
    /// A currency's.
    Currency,
    /// Time zones.
    Zones,
    /// A word's meanings, its synonyms or a translation.
    Words,
    /// What was opened from here.
    Recent,
    /// Installed applications.
    Apps,
    /// The registry's commands.
    Commands,
    /// The three strips of icons.
    Icons,
    /// Files by name.
    Files,
    /// Folders by name.
    Folders,
    /// Files by what they say.
    Contents,
    /// The video catalogue, from the index.
    Videos,
    /// YouTube's grid: the catalogue and the search.
    YouTube,
    /// The web.
    Web,
    /// Manual pages.
    Manuals,
    /// Programs to install.
    Packages,
    /// The search's own settings, under an empty box.
    Settings,
}

impl SectionId {
    /// Every section, in order — for the tests that walk the words.
    #[cfg(test)]
    pub const ALL: [Self; 18] = [
        Self::Prefixes,
        Self::Math,
        Self::Currency,
        Self::Zones,
        Self::Words,
        Self::Recent,
        Self::Apps,
        Self::Commands,
        Self::Icons,
        Self::Files,
        Self::Folders,
        Self::Contents,
        Self::Videos,
        Self::YouTube,
        Self::Web,
        Self::Manuals,
        Self::Packages,
        Self::Settings,
    ];

    /// Its kebab-case name: `fast-access-section-<id>`.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Prefixes => "prefixes",
            Self::Math => "math",
            Self::Currency => "currency",
            Self::Zones => "zones",
            Self::Words => "words",
            Self::Recent => "recent",
            Self::Apps => "apps",
            Self::Commands => "commands",
            Self::Icons => "icons",
            Self::Files => "files",
            Self::Folders => "folders",
            Self::Contents => "contents",
            Self::Videos => "videos",
            Self::YouTube => "youtube",
            Self::Web => "web",
            Self::Manuals => "manuals",
            Self::Packages => "packages",
            Self::Settings => "settings",
        }
    }

    /// The sections a scope shows, in order.
    #[must_use]
    pub fn of(scope: Scope) -> &'static [Self] {
        match scope {
            Scope::All => &[
                Self::Math,
                Self::Apps,
                Self::Commands,
                Self::Icons,
                Self::Files,
                Self::Folders,
                Self::Contents,
                Self::Videos,
                Self::Manuals,
                Self::Packages,
                Self::Web,
            ],
            Scope::Audio | Scope::Video | Scope::Images | Scope::Documents | Scope::Names => {
                &[Self::Files, Self::Folders]
            }
            Scope::Contents => &[Self::Contents],
            Scope::YouTube => &[Self::YouTube],
            Scope::Manuals => &[Self::Manuals],
            Scope::Programs => &[Self::Packages],
            Scope::Commands => &[Self::Commands],
            Scope::Web => &[Self::Web],
            Scope::Dictionary | Scope::Synonyms | Scope::Translate => &[Self::Words],
            Scope::Icons => &[Self::Icons],
            Scope::Math => &[Self::Math],
            Scope::Currency => &[Self::Currency],
            Scope::Zones => &[Self::Zones],
            Scope::Recent => &[Self::Recent],
            Scope::Prefixes => &[Self::Prefixes],
        }
    }
}

/// A hit's identity — what the cursor keeps, so an answer arriving above it
/// never moves it: `app:<id>`, `cmd:<id>`, `file:<path>`, `yt:<id>`…
pub type HitId = String;

/// One answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// Its identity.
    pub id: HitId,
    /// What a row shows as its name — data, or words already translated.
    pub title: String,
    /// The quieter line beside it, when there is one.
    pub note: Option<String>,
    /// What it is, and what acting on it reaches.
    pub what: What,
}

/// What a hit is.
#[derive(Debug, Clone, PartialEq)]
pub enum What {
    /// A prefix of the slash's list.
    Prefix(Scope),
    /// A sum's answer, with its alternatives.
    Math(Box<ronoctua_calc::Answer>),
    /// An application of the launcher.
    App {
        /// Its desktop id.
        desktop_id: String,
        /// Its glyph.
        glyph: char,
    },
    /// A command of the registry.
    Command {
        /// Its id.
        id: &'static str,
        /// The keys that run it, when a sequence reaches it.
        keys: Option<String>,
    },
    /// An icon, glyph or character.
    Icon(crate::icons_page::Icon),
    /// A file or a folder.
    File {
        /// Where.
        path: PathBuf,
        /// A folder.
        folder: bool,
        /// Found by what it says.
        content: bool,
        /// Its type.
        mime: Option<String>,
        /// Its size.
        size_bytes: Option<u64>,
        /// When it changed.
        mtime_s: Option<i64>,
    },
    /// A manual page.
    Man {
        /// Its name.
        name: String,
        /// Its section.
        section: String,
    },
    /// A program to install.
    Package {
        /// Its attribute in the Nix index.
        attr: String,
    },
    /// A video — of the catalogue, or found on YouTube.
    Video(Box<Video>),
    /// A web page.
    Web {
        /// Its address.
        url: String,
        /// A line of what it says.
        snippet: Option<String>,
        /// Which source answered (`fast-access-web-<id>`).
        source: &'static str,
    },
    /// "Search the web in the kiosk": an address for the browser, nothing
    /// asked by us.
    Kiosk(String),
    /// A word's card.
    Words(Card),
    /// A time zone.
    Zone {
        /// Its name, as tzdata writes it.
        name: String,
        /// How far ahead of UTC it is now, in seconds.
        offset_s: i32,
    },
    /// The row that asks for a host nobody let yet (`consent_rows`).
    Consent {
        /// The hosts.
        hosts: Vec<String>,
        /// What for (a key).
        what: &'static str,
    },
    /// The YouTube scope's switch for Shorts.
    Shorts(bool),
    /// A setting of the search.
    Setting(Setting),
}

/// A video, as a row and its detail need it.
#[derive(Debug, Clone, PartialEq)]
pub struct Video {
    /// YouTube's id.
    pub id: String,
    /// Its address.
    pub url: String,
    /// Its channel, when said.
    pub channel: Option<String>,
    /// Its length, when said.
    pub duration_s: Option<f64>,
    /// How many saw it, when said.
    pub views: Option<u64>,
    /// Whether it is a Short.
    pub short: bool,
    /// Whether it is in the person's catalogue.
    pub local: bool,
    /// Its picture: a file of the catalogue's, when there is one.
    pub thumb_file: Option<PathBuf>,
    /// Its picture's key, once its pixels are at hand.
    pub thumb: Option<String>,
}

/// A card of words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Card {
    /// What a word means.
    Senses(Vec<ronoctua_words::Sense>),
    /// Other words for it.
    Synonyms(Vec<String>),
    /// A translation.
    Translation(ronoctua_words::Translation),
}

/// A setting of the search's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// Keep the window ready after it closes.
    Park,
    /// Where a web video plays.
    Door,
    /// The person's SearXNG, asked beside the keyless sources — typed.
    SearXNG,
    /// The person's Brave Search key — typed, hidden, kept `0600`.
    Brave,
}

/// The question line's id for the person's SearXNG address: the answer comes
/// back under it (`fast_access_window::Search::answered`).
pub const ASK_SEARXNG: &str = "fast-access-searxng";

/// The question line's id for the Brave Search key (hidden as it is typed).
pub const ASK_BRAVE: &str = "fast-access-brave";

/// Where a web video plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VideoDoor {
    /// Asked each time.
    #[default]
    Ask,
    /// The player.
    Player,
    /// The Chrome kiosk.
    Kiosk,
}

impl VideoDoor {
    /// The ring a press turns.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Ask => Self::Player,
            Self::Player => Self::Kiosk,
            Self::Kiosk => Self::Ask,
        }
    }

    /// Its words' key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Ask => "fast-access-door-ask",
            Self::Player => "fast-access-play-player",
            Self::Kiosk => "fast-access-play-kiosk",
        }
    }
}

/// What a section holds now.
#[derive(Debug, Clone, PartialEq)]
pub enum Filled {
    /// Asked, not answered yet.
    Pending,
    /// Answered.
    Hits(Vec<Hit>),
    /// Not answered, and why — a sentence, never an empty section.
    Refused(Said),
}

/// A sentence to say, as a key and its values: what a thread with no
/// language hands back, said in the reader's by whoever draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// The key.
    pub key: String,
    /// Its values, as text.
    pub args: Vec<(&'static str, String)>,
}

impl Said {
    /// A sentence with no values.
    #[must_use]
    pub const fn key(key: String) -> Self {
        Self {
            key,
            args: Vec::new(),
        }
    }

    /// The sentence, in the reader's words.
    #[must_use]
    pub fn said(&self, texts: &ronoctua_i18n::Texts) -> String {
        let args: Vec<(&str, &str)> = self.args.iter().map(|(n, v)| (*n, v.as_str())).collect();
        texts.t_with(&self.key, &args)
    }
}

/// One section of the results.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// Which.
    pub id: SectionId,
    /// What it holds.
    pub filled: Filled,
}

impl Section {
    /// Its hits, or none.
    #[must_use]
    pub fn hits(&self) -> &[Hit] {
        match &self.filled {
            Filled::Hits(h) => h,
            _ => &[],
        }
    }
}

/// What a source handed back for one generation.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// Of which generation of what is typed.
    pub generation: u64,
    /// Which section.
    pub section: SectionId,
    /// What it holds.
    pub filled: Filled,
}

// ---------------------------------------------------------------------------
// The settings and what was opened.
// ---------------------------------------------------------------------------

/// What `fast-access.json` keeps.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Settings {
    /// Keep the window ready (parked) after it closes.
    pub park: bool,
    /// Where a web video plays.
    pub door: VideoDoor,
    /// Shorts shown among YouTube's results.
    pub shorts: bool,
    /// The person's SearXNG, asked beside the keyless sources (`https://…`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub searxng: Option<String>,
    /// What was opened from here, the newest last.
    pub recent: Vec<Opened>,
}

/// One thing opened from the search.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Opened {
    /// Its hit's identity.
    pub id: HitId,
    /// What its row said — data.
    pub title: String,
    /// How many times.
    pub uses: u32,
    /// When last, seconds since the epoch.
    pub at_s: i64,
}

/// The store of `fast-access.json`.
#[must_use]
pub fn store() -> ronoctua_store::Store<Settings> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
}

/// The most entries remembered (`assets/cache.nix`).
#[must_use]
pub fn recent_most() -> usize {
    ronoctua_inspect::cache::policy::FAST_ACCESS_RECENT_MOST
}

/// `settings` with `hit` opened once more at `now_s`, the oldest let go past
/// [`recent_most`]. Pure.
#[must_use]
pub fn remembered(mut settings: Settings, hit: &Hit, now_s: i64, most: usize) -> Settings {
    let title = hit.title.clone();
    match settings.recent.iter().position(|o| o.id == hit.id) {
        Some(i) => {
            let mut o = settings.recent.remove(i);
            o.uses = o.uses.saturating_add(1);
            o.at_s = now_s;
            o.title = title;
            settings.recent.push(o);
        }
        None => settings.recent.push(Opened {
            id: hit.id.clone(),
            title,
            uses: 1,
            at_s: now_s,
        }),
    }
    let over = settings.recent.len().saturating_sub(most);
    settings.recent.drain(..over);
    settings
}

/// How much `opened` weighs now, 0 to 1: its uses, halved every week since it
/// was last used — what the index's ranking and `/r` read. Pure.
#[must_use]
pub fn frecency(opened: &Opened, now_s: i64) -> f32 {
    const WEEK_S: f32 = 7.0 * 24.0 * 3600.0;
    #[allow(clippy::cast_precision_loss)]
    let age = (now_s.saturating_sub(opened.at_s)).max(0) as f32;
    #[allow(clippy::cast_precision_loss)]
    let uses = opened.uses as f32;
    let raw = uses * 0.5_f32.powf(age / WEEK_S);
    (raw / (raw + 4.0)).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Keys and what they ask of a door.
// ---------------------------------------------------------------------------

/// A key, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Press {
    /// Text typed.
    Type(String),
    /// The last character out.
    Back,
    /// Everything typed out (`C-u`).
    Clear,
    /// The cursor up.
    Up,
    /// The cursor down.
    Down,
    /// The cursor left — in a strip, a grid, or the icon's page.
    Left,
    /// The cursor right.
    Right,
    /// A page up.
    PageUp,
    /// A page down.
    PageDown,
    /// The first hit.
    Home,
    /// The last hit.
    End,
    /// `RET`: what the hit is for.
    Enter,
    /// `C-RET`: the second thing it offers.
    Secondary,
    /// `C-c`: copy it.
    Copy,
    /// `TAB`: the keys into the detail, and back.
    Detail,
    /// `C-w`: the detail takes the whole area.
    Whole,
    /// `C-s`: Shorts shown or left out.
    Shorts,
    /// `C-a`: the host the consent row names is let.
    Allow,
    /// `C-v`: the clipboard into what is typed.
    Paste,
    /// `Esc`.
    Escape,
}

/// The press `key` is, in this view.
#[must_use]
pub fn press_of_key(key: Key) -> Option<Press> {
    if key.sym == Sym::Esc {
        return Some(Press::Escape);
    }
    if crate::line::pastes(key) {
        return Some(Press::Paste);
    }
    let notation = key.notation();
    Some(match notation.as_str() {
        "RET" => Press::Enter,
        "C-RET" => Press::Secondary,
        "C-c" => Press::Copy,
        "TAB" | "S-TAB" => Press::Detail,
        "C-w" => Press::Whole,
        "C-s" => Press::Shorts,
        "C-a" => Press::Allow,
        "C-u" => Press::Clear,
        "C-g" => Press::Escape,
        "DEL" | "<backspace>" => Press::Back,
        "<up>" | "C-p" => Press::Up,
        "<down>" | "C-n" => Press::Down,
        "<left>" => Press::Left,
        "<right>" => Press::Right,
        "<prior>" => Press::PageUp,
        "<next>" => Press::PageDown,
        "<home>" => Press::Home,
        "<end>" => Press::End,
        "SPC" => Press::Type(" ".to_owned()),
        _ => match key.sym {
            Sym::Backspace => Press::Back,
            Sym::Char(c) if !key.mods.ctrl && !key.mods.alt && !key.mods.logo => {
                Press::Type(c.to_string())
            }
            _ => return None,
        },
    })
}

/// What a key or a click asks of the door it happened in.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// The view closes: `Esc` on an empty box — in every door.
    Close,
    /// What was asked is done (something was opened): the footer's view and
    /// the window close after it, and the column stays.
    Done,
    /// Ask the index and the network for this generation
    /// (`fast_access_search::Feed::ask`).
    Search(Box<crate::fast_access_search::Ask>),
    /// Open an application, through the footer.
    Launch(String),
    /// Open a file with what opens it, through the footer.
    OpenPath(PathBuf),
    /// Show a file in the file manager, through the footer.
    Reveal(PathBuf),
    /// Run a command of the registry, through the footer.
    Run(&'static str),
    /// Read a manual page in a terminal, through the footer.
    Man(String),
    /// Play an address in the player, through the footer.
    Player(String),
    /// Open an address in the Chrome kiosk, through the footer.
    Kiosk(String),
    /// Open the programs' centre on a search, through the footer.
    Software(String),
    /// Queue a program's install (`Work::Software`), its questions asked —
    /// what they said yes to in `consent`.
    Install {
        /// Its attribute.
        attr: String,
        /// Its name, as the row has it (data) — the queue's label.
        name: String,
        /// What was said yes to.
        consent: ronoctua_software::order::Consent,
    },
    /// Say this on the door's line.
    Say(String),
    /// Put this on the clipboard, and say so.
    Copy(String),
    /// Read the clipboard into what is typed.
    Paste,
    /// Let these hosts, then ask again.
    Allow(Vec<String>),
    /// Write the settings (`fast-access.json`), merged.
    Keep(Settings),
    /// The detail's question line: a name, a date, a tag.
    Ask(crate::line::Line),
    /// The detail's file: something to change in it (`fast_access_detail`).
    Amend(crate::fast_access_detail::Amend),
}

// ---------------------------------------------------------------------------
// The view.
// ---------------------------------------------------------------------------

/// Where the keys are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    /// On the results.
    #[default]
    Results,
    /// On the detail.
    Detail,
}

/// The question a web video asks before it plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayIn {
    /// The video's address.
    pub url: String,
    /// Its title — data.
    pub title: String,
    /// The choice under the cursor: 0 the player, 1 the kiosk, 2 "remember".
    pub cursor: usize,
    /// Whether "do the same from now on" is ticked.
    pub remember: bool,
}

/// What the in-process sources read: handed in by the door that holds them.
pub struct Ground {
    /// The launcher's applications.
    pub catalogue: crate::launch::Catalogue,
    /// The registry's commands, with their keys.
    pub commands: Vec<(&'static str, &'static str, Option<String>)>,
    /// The person's zone, and the zoneinfo root.
    pub zone: ronoctua_files::when::Zone,
    /// Where other zones are read.
    pub zoneinfo: Option<PathBuf>,
    /// The keys that show the queue (`LEADER t`), for the sentence an
    /// install says — a key is never text (`keys-are-never-text`).
    pub queue_keys: String,
}

impl std::fmt::Debug for Ground {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ground")
            .field("applications", &self.catalogue.len())
            .field("commands", &self.commands.len())
            .finish_non_exhaustive()
    }
}

/// The search: what is typed, what came back, where the cursor is.
#[derive(Debug)]
pub struct View {
    /// What is typed.
    text: String,
    /// What it means.
    query: Query,
    /// Moves with every change of what is typed.
    generation: u64,
    /// The results, by section, in order.
    sections: Vec<Section>,
    /// The hit the cursor is on, by identity — a key's or a click's.
    cursor: Option<HitId>,
    /// The hit the view SHOWED as the cursor's when data last arrived, while
    /// the person had not moved it (or its own hit was gone): an answer that
    /// lands above it does not take the highlight, the detail and `RET` with
    /// it ([`View::hold`]).
    held: Option<HitId>,
    /// Where in a strip of icons the cursor's page starts, per collection.
    pub strip_pages: [usize; 3],
    /// The first line of the results shown.
    pub offset: usize,
    /// Where the keys are.
    pub focus: Focus,
    /// The detail takes the whole area.
    pub whole: bool,
    /// The detail is at its wider width (a press on the divider).
    pub wide: bool,
    /// A web video's question, while it is asked.
    pub play_in: Option<PlayIn>,
    /// What the person chose.
    pub settings: Settings,
    /// The icon tables, once read.
    pub icons: Option<Arc<ronoctua_iconset::Set>>,
    /// Why there are none.
    pub icons_refused: bool,
    /// The icons' query grammar, built with the tables.
    grammar: Option<ronoctua_iconset::Grammar>,
    /// The reader's language subtag (`en`, `pt`).
    subtag: String,
    /// The rates at hand, for a currency.
    pub rates: Option<ronoctua_calc::rates::Rates>,
    /// The detail's own state.
    pub detail: crate::fast_access_detail::Detail,
    /// Lines the wheel asked for and the results have not moved yet.
    pub lines_owed: f64,
    /// A bar being dragged: the results' or the detail's.
    pub grabbed: Option<crate::fast_access_rows::Grab>,
    /// The pointer's target, for the line under the shortcuts.
    pub hover: Option<crate::fast_access_rows::Target>,
    /// The videos' pictures at hand, by key — kept for as long as the view,
    /// the oldest let go past [`PICTURES_KEPT`].
    pub pictures: std::collections::BTreeMap<String, Arc<ronoctua_render::image::Rgba>>,
    /// The order the pictures came in, the oldest first.
    pictures_order: std::collections::VecDeque<String>,
    /// The videos whose picture was asked for (not to ask twice).
    pub asked_pictures: std::collections::BTreeSet<String>,
    /// Each video's picture key, once it arrived — what a hit of the same
    /// video in a LATER answer is pointed at, the picture being at hand.
    picture_of: std::collections::BTreeMap<String, String>,
}

/// How many videos' pictures a view keeps: two screens of the grid.
pub const PICTURES_KEPT: usize = 2 * ronoctua_tokens::space::FASTACCESS_GRIDMOST as usize;

impl View {
    /// An empty search, the settings as `fast-access.json` says.
    #[must_use]
    pub fn new(settings: Settings, texts: &ronoctua_i18n::Texts) -> Self {
        let mut v = Self {
            text: String::new(),
            query: parse(""),
            generation: 0,
            sections: Vec::new(),
            cursor: None,
            held: None,
            strip_pages: [0; 3],
            offset: 0,
            focus: Focus::Results,
            whole: false,
            wide: false,
            play_in: None,
            settings,
            icons: None,
            icons_refused: false,
            grammar: None,
            subtag: crate::icons_page::subtag_of(texts),
            rates: None,
            detail: crate::fast_access_detail::Detail::default(),
            lines_owed: 0.0,
            grabbed: None,
            hover: None,
            pictures: std::collections::BTreeMap::new(),
            pictures_order: std::collections::VecDeque::new(),
            asked_pictures: std::collections::BTreeSet::new(),
            picture_of: std::collections::BTreeMap::new(),
        };
        v.sections = v.empty_page(texts);
        v
    }

    /// What is typed.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// What it means.
    #[must_use]
    pub const fn query(&self) -> &Query {
        &self.query
    }

    /// The generation of what is typed.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// The results, in order.
    #[must_use]
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// The hit the cursor keeps — whether or not it is in the results now.
    #[must_use]
    pub fn cursor_id(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    /// Every hit in order: what the cursor walks.
    pub fn hits(&self) -> impl Iterator<Item = &Hit> {
        self.sections.iter().flat_map(Section::hits)
    }

    /// The hit under the cursor: the one it keeps; when that one is not
    /// among the results now (or the person never moved it), the one the
    /// view showed when data last came ([`View::hold`]); else the first —
    /// without the cursor forgetting its own, which comes back with its hit.
    #[must_use]
    pub fn current(&self) -> Option<&Hit> {
        let find =
            |id: &Option<HitId>| id.as_ref().and_then(|id| self.hits().find(|h| &h.id == id));
        find(&self.cursor)
            .or_else(|| find(&self.held))
            .or_else(|| self.hits().next())
    }

    /// DATA IS ABOUT TO ARRIVE: what is shown as the cursor's now is held, so
    /// a section that answers above it moves neither the highlight nor the
    /// detail, and `RET` acts on what the person saw — the cursor keeps its
    /// `HitId` even before anybody moved it.
    fn hold(&mut self) {
        self.held = self.current().map(|h| h.id.clone());
    }

    /// What is typed changed: the cursor starts again at the top of the new
    /// answers.
    fn new_words(&mut self) {
        self.cursor = None;
        self.held = None;
    }

    /// The tables arrived.
    pub fn took_icons(
        &mut self,
        set: Result<Arc<ronoctua_iconset::Set>, ()>,
        texts: &ronoctua_i18n::Texts,
    ) {
        self.hold();
        match set {
            Ok(set) => {
                self.grammar = Some(ronoctua_iconset::Grammar::new(&set));
                self.icons = Some(set);
                self.icons_refused = false;
            }
            Err(()) => self.icons_refused = true,
        }
        // THE ICONS' SECTION IS IN-PROCESS: answered again now, for the
        // generation on screen.
        if let Some(s) = self.sections.iter_mut().find(|s| s.id == SectionId::Icons) {
            s.filled = icons_section(
                self.icons.as_deref(),
                self.grammar.as_ref(),
                self.icons_refused,
                &self.query,
                &self.subtag,
                texts,
            );
        }
    }

    /// The page an empty box shows: what a search is for, the recent, and
    /// the search's settings.
    fn empty_page(&self, texts: &ronoctua_i18n::Texts) -> Vec<Section> {
        let now = ronoctua_index::now_s();
        vec![
            Section {
                id: SectionId::Recent,
                filled: recent_section(&self.settings, "", now, texts),
            },
            Section {
                id: SectionId::Settings,
                filled: Filled::Hits(settings_rows(&self.settings, texts)),
            },
        ]
    }

    /// What is typed changed: the scope read again, the generation moved, the
    /// in-process sources answered now, and the door handed what the index
    /// and the network are to be asked.
    pub fn retyped(&mut self, ground: &Ground, texts: &ronoctua_i18n::Texts) -> Vec<Errand> {
        self.generation = self.generation.wrapping_add(1);
        self.query = parse(&self.text);
        self.offset = 0;
        self.strip_pages = [0; 3];
        self.focus = Focus::Results;
        self.detail = crate::fast_access_detail::Detail::default();
        self.play_in = None;
        if self.text.trim().is_empty() {
            self.sections = self.empty_page(texts);
            // NOTHING IS ASKED, and whatever was asked is stopped: the feed
            // drops an empty ask's answers and stops its threads.
            return vec![Errand::Search(Box::new(
                crate::fast_access_search::Ask::nothing(self.generation),
            ))];
        }
        let now = ronoctua_index::now_s();
        let mut sections = Vec::new();
        for id in SectionId::of(self.query.scope) {
            let filled = match id {
                SectionId::Prefixes => Filled::Hits(prefix_rows(&self.query.words, texts)),
                SectionId::Math => {
                    math_section(&self.query, &self.text, ground, self.rates.as_ref(), texts)
                }
                SectionId::Currency => {
                    currency_section(&self.query, ground, self.rates.as_ref(), texts)
                }
                SectionId::Apps => apps_section(&ground.catalogue, &self.query),
                SectionId::Commands => commands_section(&ground.commands, &self.query, texts),
                SectionId::Icons => icons_section(
                    self.icons.as_deref(),
                    self.grammar.as_ref(),
                    self.icons_refused,
                    &self.query,
                    &self.subtag,
                    texts,
                ),
                SectionId::Recent => recent_section(&self.settings, &self.query.words, now, texts),
                SectionId::Zones => Filled::Pending,
                _ => Filled::Pending,
            };
            // A SECTION THE SCOPE ALWAYS SHOWS BUT NOTHING ANSWERED IN-PROCESS
            // is left out of "everything": an empty sum is no sum.
            if self.query.scope == Scope::All
                && matches!(
                    id,
                    SectionId::Math | SectionId::Apps | SectionId::Commands | SectionId::Icons
                )
                && matches!(&filled, Filled::Hits(h) if h.is_empty())
            {
                continue;
            }
            sections.push(Section { id: *id, filled });
        }
        // THE WEB IN "EVERYTHING" is one row that sends nothing of ours.
        if self.query.scope == Scope::All
            && let Some(s) = sections.iter_mut().find(|s| s.id == SectionId::Web)
        {
            s.filled = Filled::Hits(vec![kiosk_row(&self.query.words, texts)]);
        }
        self.sections = sections;
        let ask = crate::fast_access_search::Ask::of(
            self.generation,
            &self.query,
            &self.settings,
            &self.subtag,
            self.settings.recent.as_slice(),
        );
        vec![Errand::Search(Box::new(ask))]
    }

    /// An answer arrived. Dropped when it is of another generation; the
    /// cursor keeps its hit and the page its first line.
    pub fn arrived(&mut self, answer: Answer) -> bool {
        if answer.generation != self.generation {
            return false;
        }
        self.hold();
        if answer.section == SectionId::YouTube {
            return self.youtube_arrived(answer.filled);
        }
        if answer.section == SectionId::Videos {
            return self.videos_arrived(answer.filled);
        }
        match self.sections.iter_mut().find(|s| s.id == answer.section) {
            Some(s) => {
                // A WEB SECTION OF "EVERYTHING" keeps its kiosk row first.
                if let (Filled::Hits(old), Filled::Hits(new)) = (&s.filled, &answer.filled)
                    && s.id == SectionId::Web
                    && let Some(kiosk) = old.iter().find(|h| matches!(h.what, What::Kiosk(_)))
                    && !new.iter().any(|h| matches!(h.what, What::Kiosk(_)))
                {
                    let mut all = vec![kiosk.clone()];
                    all.extend(new.iter().cloned());
                    s.filled = Filled::Hits(all);
                } else {
                    s.filled = answer.filled;
                }
                true
            }
            None => false,
        }
    }

    /// The videos of "everything": the index's answer (the catalogue's
    /// videos) and YouTube's search (`fast_access_videos::online_in_everything`)
    /// come on two threads in either order, and each keeps the other's — the
    /// catalogue's first, a video both found once.
    fn videos_arrived(&mut self, filled: Filled) -> bool {
        let Some(section) = self.sections.iter_mut().find(|s| s.id == SectionId::Videos) else {
            return false;
        };
        let local = |h: &Hit| !matches!(&h.what, What::Video(v) if !v.local);
        let Filled::Hits(incoming) = filled else {
            section.filled = filled;
            return true;
        };
        let online = incoming.iter().any(|h| !local(h));
        let old: Vec<Hit> = match &section.filled {
            Filled::Hits(h) => h.iter().filter(|h| local(h) == online).cloned().collect(),
            _ => Vec::new(),
        };
        let (first, then) = if online {
            (old, incoming)
        } else {
            (incoming, old)
        };
        let mut seen = std::collections::BTreeSet::new();
        let merged: Vec<Hit> = first
            .into_iter()
            .chain(then)
            .filter(|h| seen.insert(h.id.clone()))
            .collect();
        section.filled = Filled::Hits(merged);
        true
    }

    /// YouTube's grid: the catalogue's answer and the search's, merged by the
    /// video's id — the catalogue's first, with its picture; the switch's row
    /// and the consent rows kept from whichever brought them.
    fn youtube_arrived(&mut self, filled: Filled) -> bool {
        let Some(old) = self
            .sections
            .iter()
            .find(|s| s.id == SectionId::YouTube)
            .map(|s| s.hits().to_vec())
        else {
            return false;
        };
        let incoming = match filled {
            Filled::Hits(h) => h,
            refused @ Filled::Refused(_) => {
                // A REFUSAL OF THE SEARCH keeps what the catalogue answered.
                if old
                    .iter()
                    .any(|h| matches!(&h.what, What::Video(v) if v.local))
                {
                    return false;
                }
                self.set_section(SectionId::YouTube, refused);
                return true;
            }
            Filled::Pending => return false,
        };
        let online = incoming.iter().any(|h| {
            matches!(&h.what, What::Video(v) if !v.local) || matches!(h.what, What::Consent { .. })
        });
        let keep_old = |h: &Hit| match &h.what {
            What::Video(v) => v.local == online,
            What::Consent { .. } => !online,
            _ => true,
        };
        let (first, second): (Vec<Hit>, Vec<Hit>) = if online {
            (old.into_iter().filter(keep_old).collect(), incoming)
        } else {
            (incoming, old.into_iter().filter(keep_old).collect())
        };
        let mut merged: Vec<Hit> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        // THE SWITCH AND THE CONSENT ROWS FIRST, then the videos.
        for h in first.iter().chain(second.iter()) {
            if !matches!(h.what, What::Video(_)) && seen.insert(h.id.clone()) {
                merged.push(h.clone());
            }
        }
        for h in first.into_iter().chain(second) {
            if matches!(h.what, What::Video(_)) && seen.insert(h.id.clone()) {
                merged.push(h);
            }
        }
        // A PICTURE ALREADY AT HAND goes with its video.
        for h in &mut merged {
            if let What::Video(v) = &mut h.what
                && v.thumb.is_none()
            {
                let key = crate::fast_access_videos::picture_key(&v.id);
                if self.pictures.contains_key(&key) {
                    v.thumb = Some(key);
                }
            }
        }
        self.set_section(SectionId::YouTube, Filled::Hits(merged));
        true
    }

    /// A section's answer, replaced.
    fn set_section(&mut self, id: SectionId, filled: Filled) {
        if let Some(s) = self.sections.iter_mut().find(|s| s.id == id) {
            s.filled = filled;
        }
    }

    /// A video's picture arrived: every hit of that video carries its key.
    pub fn thumbed(&mut self, video: &str, key: &str) -> bool {
        let mut moved = false;
        for s in &mut self.sections {
            if let Filled::Hits(hits) = &mut s.filled {
                for h in hits {
                    if let What::Video(v) = &mut h.what
                        && v.id == video
                        && v.thumb.as_deref() != Some(key)
                    {
                        v.thumb = Some(key.to_owned());
                        moved = true;
                    }
                }
            }
        }
        moved
    }

    /// A video's picture arrived: kept under its key (the oldest let go past
    /// [`PICTURES_KEPT`]) and every hit of the video pointed at it.
    pub fn pictured(
        &mut self,
        video: &str,
        key: String,
        rgba: Arc<ronoctua_render::image::Rgba>,
    ) -> bool {
        if !self.pictures.contains_key(&key) {
            self.pictures_order.push_back(key.clone());
        }
        self.pictures.insert(key.clone(), rgba);
        self.picture_of.insert(video.to_owned(), key.clone());
        while self.pictures_order.len() > PICTURES_KEPT {
            if let Some(old) = self.pictures_order.pop_front() {
                self.pictures.remove(&old);
                // A PICTURE LET GO is asked for again when a video wants it.
                self.picture_of.retain(|video, k| {
                    let gone = *k == old;
                    if gone {
                        self.asked_pictures.remove(video);
                    }
                    !gone
                });
            }
        }
        self.thumbed(video, &key)
    }

    /// The pictures the videos on screen want and nobody asked for yet —
    /// the catalogue's files, decoded off the loop by the feed.
    pub fn pictures_wanted(&mut self) -> Vec<(String, PathBuf)> {
        // A VIDEO WHOSE PICTURE IS AT HAND — an answer of a later key, the
        // same video found again — is pointed at it, and nothing is asked.
        let known: Vec<(String, String)> = self
            .hits()
            .filter_map(|h| match &h.what {
                What::Video(v) if v.thumb.is_none() => self
                    .picture_of
                    .get(&v.id)
                    .filter(|k| self.pictures.contains_key(*k))
                    .map(|k| (v.id.clone(), k.clone())),
                _ => None,
            })
            .collect();
        for (video, key) in known {
            self.thumbed(&video, &key);
        }
        let hits: Vec<Hit> = self.hits().cloned().collect();
        let wanted = crate::fast_access_videos::wanted(&hits, &self.asked_pictures);
        for (id, _) in &wanted {
            self.asked_pictures.insert(id.clone());
        }
        wanted
    }

    /// The rates at hand changed: a sum with a currency is answered again.
    pub fn rates_are(
        &mut self,
        rates: Option<ronoctua_calc::rates::Rates>,
        ground: &Ground,
        texts: &ronoctua_i18n::Texts,
    ) {
        self.hold();
        self.rates = rates;
        for s in &mut self.sections {
            match s.id {
                SectionId::Math => {
                    s.filled =
                        math_section(&self.query, &self.text, ground, self.rates.as_ref(), texts);
                }
                SectionId::Currency => {
                    s.filled = currency_section(&self.query, ground, self.rates.as_ref(), texts);
                }
                _ => {}
            }
        }
    }

    /// The rates did not come for `generation`: the older ones on disk serve
    /// — the answer dated by them (`calc-rates-of` in its detail) — or, with
    /// none, the currency's section SAYS why instead of searching for ever.
    /// Answers whether anything moved.
    pub fn rates_failed(
        &mut self,
        generation: u64,
        why: ronoctua_calc::rates::Why,
        stale: Option<ronoctua_calc::rates::Rates>,
        ground: &Ground,
        texts: &ronoctua_i18n::Texts,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        if let Some(r) = stale {
            self.rates_are(Some(r), ground, texts);
            return true;
        }
        self.hold();
        match self
            .sections
            .iter_mut()
            .find(|s| s.id == SectionId::Currency && s.filled == Filled::Pending)
        {
            Some(s) => {
                s.filled = Filled::Refused(Said::key(format!("calc-rates-why-{}", why.id())));
                true
            }
            None => false,
        }
    }

    /// The launcher's applications were scanned again: their section — and
    /// only theirs — is answered again IN PLACE, for what is typed now. No
    /// generation moves, nothing in flight is dropped, the page and the
    /// cursor stay: this is data, not a key.
    pub fn apps_are(&mut self, ground: &Ground) -> bool {
        if self.text.trim().is_empty() {
            return false;
        }
        let order = SectionId::of(self.query.scope);
        if !order.contains(&SectionId::Apps) {
            return false;
        }
        self.hold();
        let filled = apps_section(&ground.catalogue, &self.query);
        let empty = matches!(&filled, Filled::Hits(h) if h.is_empty());
        let at = self.sections.iter().position(|s| s.id == SectionId::Apps);
        match at {
            // "EVERYTHING" LEAVES OUT a section with nothing, as `retyped`.
            Some(i) if empty && self.query.scope == Scope::All => {
                self.sections.remove(i);
            }
            Some(i) => self.sections[i].filled = filled,
            None if empty && self.query.scope == Scope::All => return false,
            None => {
                let rank = |id: SectionId| order.iter().position(|o| *o == id);
                let mine = rank(SectionId::Apps);
                let i = self
                    .sections
                    .iter()
                    .position(|s| rank(s.id) > mine)
                    .unwrap_or(self.sections.len());
                self.sections.insert(
                    i,
                    Section {
                        id: SectionId::Apps,
                        filled,
                    },
                );
            }
        }
        true
    }

    /// Text typed — a key, a space included — its control characters made
    /// spaces.
    pub fn typed(
        &mut self,
        text: &str,
        ground: &Ground,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Errand> {
        let text: String = text
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        if text.is_empty() {
            return Vec::new();
        }
        self.text.push_str(&text);
        self.new_words();
        self.retyped(ground, texts)
    }

    /// Moves the cursor onto `id` (a click, a key).
    pub fn point_at(&mut self, id: &str) {
        self.cursor = Some(id.to_owned());
        self.held = None;
    }

    /// The cursor onto the first hit, or the last.
    pub fn cursor_to_end(&mut self, last: bool) {
        let id = if last {
            self.hits().last().map(|h| h.id.clone())
        } else {
            self.hits().next().map(|h| h.id.clone())
        };
        if let Some(id) = id {
            self.cursor = Some(id);
        }
    }

    /// The cursor's hit, recorded as opened: what `/r` lists and the index
    /// weighs. The settings to write.
    #[must_use]
    pub fn remember(&mut self, hit: &Hit) -> Settings {
        self.settings = remembered(
            self.settings.clone(),
            hit,
            ronoctua_index::now_s(),
            recent_most(),
        );
        self.settings.clone()
    }

    /// One press, and what the door is to do after it. `area` is where the
    /// view is, for the keys that move by what is drawn (a page, a grid's
    /// row).
    pub fn key(
        &mut self,
        press: Press,
        area: crate::icons_page::Area,
        ground: &Ground,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Errand> {
        // THE QUESTION a web video asks takes every key while it is up.
        if self.play_in.is_some() {
            return self.play_in_key(&press);
        }
        if self.focus == Focus::Detail {
            match press {
                Press::Detail => {
                    self.focus = Focus::Results;
                    return Vec::new();
                }
                Press::Escape => {
                    // A QUESTION UP IS ANSWERED `no` FIRST: `Esc` takes it
                    // down, and the keys stay where it was asked.
                    if self.detail.confirm.take().is_some() {
                        return Vec::new();
                    }
                    self.focus = Focus::Results;
                    self.whole = false;
                    return Vec::new();
                }
                Press::Whole => {
                    self.whole = !self.whole;
                    return Vec::new();
                }
                _ => {
                    let hit = self.current().cloned();
                    // A ROW THAT DOES WHAT THE RESULTS DO is done by the
                    // view, which holds what opening reaches.
                    if press == Press::Enter
                        && let Some(h) = hit.as_ref()
                    {
                        match crate::fast_access_detail::row_act(self, h, texts) {
                            Some(crate::fast_access_detail::Act::Enter) => {
                                return self.enter(false, ground, texts);
                            }
                            Some(crate::fast_access_detail::Act::Secondary) => {
                                return self.enter(true, ground, texts);
                            }
                            Some(crate::fast_access_detail::Act::Narrow) => {
                                self.focus = Focus::Results;
                                return self.enter(false, ground, texts);
                            }
                            _ => {}
                        }
                    }
                    return crate::fast_access_detail::key(self, hit.as_ref(), &press, area, texts);
                }
            }
        }
        match press {
            Press::Type(t) => self.typed(&t, ground, texts),
            Press::Back => {
                if self.text.is_empty() {
                    return Vec::new();
                }
                crate::line::pop_cluster(&mut self.text);
                self.new_words();
                self.retyped(ground, texts)
            }
            Press::Clear => {
                if self.text.is_empty() {
                    return Vec::new();
                }
                self.text.clear();
                self.new_words();
                self.retyped(ground, texts)
            }
            Press::Paste => vec![Errand::Paste],
            Press::Escape => {
                if self.whole {
                    self.whole = false;
                    return Vec::new();
                }
                vec![Errand::Close]
            }
            Press::Up | Press::Down | Press::Left | Press::Right => {
                crate::fast_access_rows::moved(self, &press, area, texts);
                Vec::new()
            }
            Press::PageUp | Press::PageDown => {
                crate::fast_access_rows::paged(self, press == Press::PageDown, area, texts);
                Vec::new()
            }
            Press::Home => {
                self.cursor_to_end(false);
                self.offset = 0;
                Vec::new()
            }
            Press::End => {
                self.cursor_to_end(true);
                crate::fast_access_rows::follow(self, area, texts);
                Vec::new()
            }
            Press::Detail => {
                if self.current().is_some() && !self.query.scope.whole() {
                    self.focus = Focus::Detail;
                }
                Vec::new()
            }
            Press::Whole => {
                if !self.query.scope.whole() && self.current().is_some() {
                    self.whole = !self.whole;
                }
                Vec::new()
            }
            Press::Shorts => {
                if self.query.scope != Scope::YouTube {
                    return Vec::new();
                }
                self.toggle_shorts(ground, texts)
            }
            Press::Allow => match self.current().map(|h| h.what.clone()) {
                Some(What::Consent { hosts, .. }) => vec![Errand::Allow(hosts)],
                _ => self
                    .hits()
                    .find_map(|h| match &h.what {
                        What::Consent { hosts, .. } => Some(vec![Errand::Allow(hosts.clone())]),
                        _ => None,
                    })
                    .unwrap_or_default(),
            },
            Press::Enter => self.enter(false, ground, texts),
            Press::Secondary => self.enter(true, ground, texts),
            Press::Copy => self
                .current()
                .and_then(|h| copy_of(h, texts))
                .map(|t| vec![Errand::Copy(t)])
                .unwrap_or_default(),
        }
    }

    /// Shorts shown, or left out: the setting written and the scope asked
    /// again.
    fn toggle_shorts(&mut self, ground: &Ground, texts: &ronoctua_i18n::Texts) -> Vec<Errand> {
        self.settings.shorts = !self.settings.shorts;
        let mut out = vec![Errand::Keep(self.settings.clone())];
        out.extend(self.retyped(ground, texts));
        out
    }

    /// The web video's question, answered by a key.
    fn play_in_key(&mut self, press: &Press) -> Vec<Errand> {
        let Some(q) = self.play_in.as_mut() else {
            return Vec::new();
        };
        match press {
            Press::Up | Press::Left => q.cursor = q.cursor.saturating_sub(1),
            Press::Down | Press::Right => q.cursor = (q.cursor + 1).min(2),
            Press::Escape => self.play_in = None,
            Press::Enter => return self.play_in_choose(None),
            _ => {}
        }
        Vec::new()
    }

    /// The question's choice under the cursor (or `at`, a click) taken.
    pub fn play_in_choose(&mut self, at: Option<usize>) -> Vec<Errand> {
        let Some(q) = self.play_in.as_mut() else {
            return Vec::new();
        };
        let choice = at.unwrap_or(q.cursor);
        q.cursor = choice;
        if choice == 2 {
            q.remember = !q.remember;
            return Vec::new();
        }
        let Some(q) = self.play_in.take() else {
            return Vec::new();
        };
        let door = if choice == 0 {
            VideoDoor::Player
        } else {
            VideoDoor::Kiosk
        };
        let mut out = Vec::new();
        if q.remember {
            self.settings.door = door;
            out.push(Errand::Keep(self.settings.clone()));
        }
        out.push(match door {
            VideoDoor::Kiosk => Errand::Kiosk(q.url),
            _ => Errand::Player(q.url),
        });
        out.push(Errand::Done);
        out
    }

    /// `RET` (or `C-RET`): what the cursor's hit is for — or its second
    /// thing. A hit that opens something closes the view after it (the
    /// column's door keeps itself); one that changes the search does not.
    fn enter(
        &mut self,
        secondary: bool,
        ground: &Ground,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Errand> {
        let Some(hit) = self.current().cloned() else {
            return Vec::new();
        };
        let mut out: Vec<Errand> = Vec::new();
        let opens = |e: Errand| vec![e, Errand::Done];
        let acted = match (&hit.what, secondary) {
            (What::Prefix(scope), _) => {
                if let Some(letter) = scope.letter() {
                    self.text = format!("{SLASH}{letter} ");
                    self.new_words();
                    return self.retyped(ground, texts);
                }
                return Vec::new();
            }
            (What::Setting(s), _) => {
                match s {
                    Setting::Park => self.settings.park = !self.settings.park,
                    Setting::Door => self.settings.door = self.settings.door.next(),
                    // TYPED, on the door's question line — the answer comes
                    // back under the line's id (`Search::answered`).
                    Setting::SearXNG => {
                        return vec![Errand::Ask(crate::line::Line::new(
                            ASK_SEARXNG,
                            false,
                            self.settings.searxng.as_deref(),
                        ))];
                    }
                    Setting::Brave => {
                        return vec![Errand::Ask(crate::line::Line::new(ASK_BRAVE, true, None))];
                    }
                }
                let keep = Errand::Keep(self.settings.clone());
                self.hold();
                self.sections = self.empty_page(texts);
                return vec![keep];
            }
            (What::Shorts(_), _) => return self.toggle_shorts(ground, texts),
            (What::Consent { hosts, .. }, _) => return vec![Errand::Allow(hosts.clone())],
            (What::App { desktop_id, .. }, _) => opens(Errand::Launch(desktop_id.clone())),
            (What::Command { id, .. }, _) => opens(Errand::Run(id)),
            (What::File { path, .. }, false) => opens(Errand::OpenPath(path.clone())),
            (What::File { path, .. }, true) => opens(Errand::Reveal(path.clone())),
            (What::Man { name, section }, _) => opens(Errand::Man(format!("{name}.{section}"))),
            // A PROGRAM IS INSTALLED THROUGH THE QUEUE, its questions asked
            // first — the licence, the size — as the programs' centre asks
            // them; `C-RET` opens the centre on it instead.
            (What::Package { attr }, false) => {
                return self.install(
                    attr,
                    &hit.title,
                    ronoctua_software::order::Consent::default(),
                    texts,
                );
            }
            (What::Package { attr }, true) => opens(Errand::Software(attr.clone())),
            (What::Video(v), second) => {
                let door = match (self.settings.door, second) {
                    (VideoDoor::Ask, false) => {
                        self.play_in = Some(PlayIn {
                            url: v.url.clone(),
                            title: hit.title.clone(),
                            cursor: 0,
                            remember: false,
                        });
                        return Vec::new();
                    }
                    (VideoDoor::Ask, true)
                    | (VideoDoor::Kiosk, true)
                    | (VideoDoor::Player, false) => VideoDoor::Player,
                    (VideoDoor::Kiosk, false) | (VideoDoor::Player, true) => VideoDoor::Kiosk,
                };
                opens(match door {
                    VideoDoor::Kiosk => Errand::Kiosk(v.url.clone()),
                    _ => Errand::Player(v.url.clone()),
                })
            }
            (What::Web { url, .. }, false) => opens(Errand::Kiosk(url.clone())),
            (What::Web { url, .. }, true) => return vec![Errand::Copy(url.clone())],
            (What::Kiosk(url), _) => opens(Errand::Kiosk(url.clone())),
            (What::Icon(_) | What::Math(_) | What::Words(_) | What::Zone { .. }, _) => {
                return copy_of(&hit, texts)
                    .map(|t| vec![Errand::Copy(t)])
                    .unwrap_or_default();
            }
        };
        out.push(Errand::Keep(self.remember(&hit)));
        out.extend(acted);
        out
    }

    /// Installs the program `attr` (named `name`), with what was said yes to
    /// so far: a badge that refuses (broken, insecure) says why; a licence
    /// that is not free, the person has not let before, asks; a download past
    /// the person's threshold asks — each a question in the detail, `no`
    /// first and the cursor on it — and with nothing left to ask, the order
    /// goes to the queue (`Errand::Install`). The programs' centre's own
    /// questions, in its own words, from its own engine's reading
    /// (`fast_access_search::Program`): installing from here never skips one.
    pub fn install(
        &mut self,
        attr: &str,
        name: &str,
        consent: ronoctua_software::order::Consent,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Errand> {
        use crate::key;
        use ronoctua_software::found::Badge;
        let read = self
            .detail
            .program
            .as_ref()
            .filter(|(a, _)| a == attr)
            .map(|(_, p)| p.clone());
        let details = read.as_ref().and_then(|p| p.details.as_ref().ok());
        if let Some(why) = details.and_then(|d| d.badges.iter().find_map(|b| b.refuses())) {
            return vec![Errand::Say(
                texts.t(crate::software_words::refusal_key(why)),
            )];
        }
        let ask = |title: String, yes: &str, then: ronoctua_software::order::Consent| {
            crate::fast_access_detail::Confirm {
                title,
                answers: vec![texts.t(key::SOFTWARE_NO), texts.t(yes)],
                does: vec![
                    None,
                    Some(crate::fast_access_detail::Amend::Install {
                        attr: attr.to_owned(),
                        name: name.to_owned(),
                        consent: then,
                    }),
                ],
                cursor: 0,
            }
        };
        let unfree = details.is_some_and(|d| d.badges.contains(&Badge::Unfree))
            && !read.as_ref().is_some_and(|p| p.unfree_let)
            && !consent.unfree;
        let question = if unfree {
            Some(ask(
                texts.t_with(key::SOFTWARE_SURE_UNFREE, &[("app", name)]),
                key::SOFTWARE_YES_UNFREE,
                ronoctua_software::order::Consent {
                    unfree: true,
                    ..consent
                },
            ))
        } else {
            let above = read.as_ref().map_or(0, |p| p.ask_above_mb);
            let bytes = details.and_then(|d| d.download_bytes);
            match bytes {
                Some(b) if above > 0 && b > above.saturating_mul(1_000_000) && !consent.big => {
                    Some(ask(
                        texts.t_with(
                            key::SOFTWARE_SURE_DOWNLOAD,
                            &[
                                ("app", name),
                                (
                                    "size",
                                    &crate::sizes::readable_bytes(b, texts.decimal_mark()),
                                ),
                            ],
                        ),
                        key::SOFTWARE_YES_DOWNLOAD,
                        ronoctua_software::order::Consent {
                            big: true,
                            ..consent
                        },
                    ))
                }
                _ => None,
            }
        };
        if let Some(q) = question {
            self.detail.confirm = Some(q);
            self.focus = Focus::Detail;
            return Vec::new();
        }
        let hit = self.current().cloned();
        let mut out = Vec::new();
        if let Some(h) = hit.filter(|h| matches!(&h.what, What::Package { attr: a } if a == attr)) {
            out.push(Errand::Keep(self.remember(&h)));
        }
        out.push(Errand::Install {
            attr: attr.to_owned(),
            name: name.to_owned(),
            consent,
        });
        out
    }

    /// The detail follows the cursor's hit: a new one starts at the top, an
    /// icon gets its page and a file its panels. Called by the door before a
    /// frame and after a key.
    pub fn settle_detail(
        &mut self,
        texts: &std::rc::Rc<ronoctua_i18n::Texts>,
        zone: &ronoctua_files::when::Zone,
    ) {
        let hit = if self.query.scope.whole() {
            None
        } else {
            self.current().cloned()
        };
        crate::fast_access_detail::follow_hit(self, hit.as_ref(), texts, zone);
    }

    /// The clipboard's text, into what is typed — on one line, its ends
    /// trimmed.
    pub fn pasted(
        &mut self,
        text: &str,
        ground: &Ground,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Errand> {
        self.typed(&crate::line::one_line(text), ground, texts)
    }

    /// The settings file changed (this door's write, or another's).
    pub fn settings_are(&mut self, settings: Settings, texts: &ronoctua_i18n::Texts) {
        self.settings = settings;
        if self.text.trim().is_empty() {
            self.hold();
            self.sections = self.empty_page(texts);
        }
    }

    /// A refusal for every section still pending — the door could not ask
    /// (no threads); said, never an empty page.
    pub fn refuse_pending(&mut self, said: &Said) {
        for s in &mut self.sections {
            if s.filled == Filled::Pending {
                s.filled = Filled::Refused(said.clone());
            }
        }
    }

    /// Replaces the results with these — a test's.
    #[cfg(test)]
    pub fn with_sections(&mut self, sections: Vec<Section>) {
        self.sections = sections;
    }

    /// Sets what is typed without asking anything — a test's, a
    /// photograph's, and a kept window's woken with a query (which asks
    /// right after, with `retyped`).
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_owned();
        self.query = parse(text);
        self.new_words();
    }
}

/// What copying a hit puts on the clipboard.
#[must_use]
pub fn copy_of(hit: &Hit, texts: &ronoctua_i18n::Texts) -> Option<String> {
    Some(match &hit.what {
        What::File { path, .. } => path.display().to_string(),
        What::Video(v) => v.url.clone(),
        What::Web { url, .. } | What::Kiosk(url) => url.clone(),
        What::Math(a) => crate::calc::answer_text(&a.value, texts.decimal_mark(), texts)
            .trim_start_matches(['=', '≈', ' '])
            .to_owned(),
        What::Zone { offset_s, .. } => {
            crate::fast_access_rows::zone_time(*offset_s, ronoctua_index::now_s())
        }
        What::Command { id, .. } => (*id).to_owned(),
        What::App { desktop_id, .. } => desktop_id.clone(),
        What::Man { name, section } => format!("{name}({section})"),
        What::Package { attr } => attr.clone(),
        What::Words(Card::Translation(t)) => t.text.clone(),
        What::Words(Card::Synonyms(s)) => s.join(", "),
        What::Words(Card::Senses(s)) => s.first()?.gloss.clone(),
        What::Icon(icon) => {
            // THE CHARACTER ITSELF: what the icons app's first format copies.
            hit.note
                .clone()
                .filter(|_| icon.collection == Collection::Unicode)
                .unwrap_or_else(|| hit.title.clone())
        }
        What::Prefix(_) | What::Consent { .. } | What::Shorts(_) | What::Setting(_) => return None,
    })
}

// ---------------------------------------------------------------------------
// The in-process sources.
// ---------------------------------------------------------------------------

/// How many hits a section of "everything" shows; a narrowed scope shows
/// many more.
#[must_use]
pub const fn most_of(scope: Scope, section: SectionId) -> usize {
    match (scope, section) {
        (Scope::All, SectionId::Files) => 8,
        (Scope::All, _) => 5,
        _ => 100,
    }
}

/// The slash's list: every prefix whose letter or words match what follows.
fn prefix_rows(words: &str, texts: &ronoctua_i18n::Texts) -> Vec<Hit> {
    let folded = ronoctua_apps::fold::fold(words.trim());
    PREFIXES
        .iter()
        .filter(|(letter, scope)| {
            folded.is_empty()
                || *letter == folded
                || ronoctua_apps::fold::fold(&texts.t(&format!("fast-access-scope-{}", scope.id())))
                    .contains(&folded)
        })
        .map(|(letter, scope)| Hit {
            id: format!("prefix:{letter}"),
            title: format!(
                "{SLASH}{letter}  {}",
                texts.t(&format!("fast-access-scope-{}", scope.id()))
            ),
            note: Some(scope.does(texts)),
            what: What::Prefix(*scope),
        })
        .collect()
}

/// A sum's answer: `quick` for "everything", the whole parse for `/=`.
fn math_section(
    query: &Query,
    _text: &str,
    ground: &Ground,
    rates: Option<&ronoctua_calc::rates::Rates>,
    texts: &ronoctua_i18n::Texts,
) -> Filled {
    let mark = texts.decimal_mark();
    let env = ronoctua_calc::Env {
        now_unix_s: ronoctua_index::now_s(),
        zone: ground.zone.clone(),
        zoneinfo: ground.zoneinfo.clone(),
        rates: rates.cloned(),
    };
    let words = query.words.trim();
    let answer = if query.scope == Scope::Math {
        match ronoctua_calc::parse(words, mark).and_then(|e| ronoctua_calc::eval(&e, &env)) {
            Ok(value) => Some(ronoctua_calc::Answer {
                shape: ronoctua_calc::Shape::Arithmetic,
                also: Vec::new(),
                value,
            }),
            Err(r) if words.is_empty() => {
                let _ = r;
                return Filled::Hits(Vec::new());
            }
            Err(r) => return Filled::Refused(Said::key(format!("calc-why-{}", r.why.id()))),
        }
    } else {
        ronoctua_calc::quick(words, mark, &env)
    };
    Filled::Hits(
        answer
            .into_iter()
            .map(|a| math_hit(words, a, texts))
            .collect(),
    )
}

/// A sum's hit: the expression as typed, its answer as the calculator says
/// it.
fn math_hit(words: &str, answer: ronoctua_calc::Answer, texts: &ronoctua_i18n::Texts) -> Hit {
    Hit {
        id: "math".to_owned(),
        title: words.to_owned(),
        note: Some(crate::calc::answer_text(
            &answer.value,
            texts.decimal_mark(),
            texts,
        )),
        what: What::Math(Box::new(answer)),
    }
}

/// A currency's answer: with rates at hand, the sum; without, the row that
/// asks for the bank's host — or, let, nothing until they come.
fn currency_section(
    query: &Query,
    ground: &Ground,
    rates: Option<&ronoctua_calc::rates::Rates>,
    texts: &ronoctua_i18n::Texts,
) -> Filled {
    if query.words.trim().is_empty() {
        return Filled::Hits(Vec::new());
    }
    if rates.is_none() {
        let host = ronoctua_calc::rates::HOST;
        if !ronoctua_consent::allows(host) {
            return Filled::Hits(vec![consent_hit(&[host], "calc-rates-for")]);
        }
        return Filled::Pending;
    }
    let mark = texts.decimal_mark();
    let env = ronoctua_calc::Env {
        now_unix_s: ronoctua_index::now_s(),
        zone: ground.zone.clone(),
        zoneinfo: ground.zoneinfo.clone(),
        rates: rates.cloned(),
    };
    match ronoctua_calc::parse(query.words.trim(), mark).and_then(|e| ronoctua_calc::eval(&e, &env))
    {
        Ok(value) => Filled::Hits(vec![math_hit(
            query.words.trim(),
            ronoctua_calc::Answer {
                shape: ronoctua_calc::Shape::Currency,
                also: Vec::new(),
                value,
            },
            texts,
        )]),
        Err(r) => Filled::Refused(Said::key(format!("calc-why-{}", r.why.id()))),
    }
}

/// The consent row, as a hit of its section.
#[must_use]
pub fn consent_hit(hosts: &[&str], what: &'static str) -> Hit {
    Hit {
        id: format!("consent:{}", hosts.join(",")),
        title: String::new(),
        note: None,
        what: What::Consent {
            hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
            what,
        },
    }
}

/// Whether `keys` hold the words as they were typed — one piece of one key —
/// which is what "everything" asks of an in-process list: a loose
/// subsequence of six letters matches half the registry.
fn holds(q: &str, keys: &[&str]) -> bool {
    keys.iter().any(|k| k.contains(q))
}

/// The applications whose name or id the words find, best first — a piece of
/// the name in "everything", any subsequence once narrowed.
fn apps_section(catalogue: &crate::launch::Catalogue, query: &Query) -> Filled {
    let q = ronoctua_apps::fold::fold(query.words.trim());
    if q.is_empty() {
        return Filled::Hits(Vec::new());
    }
    let keys: Vec<[&str; 2]> = catalogue
        .iter()
        .map(|e| [e.key.as_str(), e.id.as_str()])
        .collect();
    let items: Vec<&[&str]> = keys.iter().map(<[&str; 2]>::as_slice).collect();
    let loose = query.scope != Scope::All;
    Filled::Hits(
        crate::minibuffer::rank(&q, &items)
            .into_iter()
            .filter(|i| loose || holds(&q, items[*i]))
            .take(most_of(query.scope, SectionId::Apps))
            .filter_map(|i| catalogue.get(i))
            .map(|e| Hit {
                id: format!("app:{}", e.id),
                title: e.name.clone(),
                note: (!e.description.is_empty()).then(|| e.description.clone()),
                what: What::App {
                    desktop_id: e.id.clone(),
                    glyph: crate::launch::glyph_of(e),
                },
            })
            .collect(),
    )
}

/// The commands whose words or id the words find, best first, with the keys
/// that run them.
fn commands_section(
    commands: &[(&'static str, &'static str, Option<String>)],
    query: &Query,
    texts: &ronoctua_i18n::Texts,
) -> Filled {
    let q = ronoctua_apps::fold::fold(query.words.trim());
    if q.is_empty() && query.scope != Scope::Commands {
        return Filled::Hits(Vec::new());
    }
    let labels: Vec<String> = commands
        .iter()
        .map(|(_, label, _)| ronoctua_apps::fold::fold(&texts.t(label)))
        .collect();
    let keys: Vec<[&str; 2]> = commands
        .iter()
        .zip(&labels)
        .map(|((id, _, _), l)| [l.as_str(), *id])
        .collect();
    let items: Vec<&[&str]> = keys.iter().map(<[&str; 2]>::as_slice).collect();
    let loose = query.scope != Scope::All;
    Filled::Hits(
        crate::minibuffer::rank(&q, &items)
            .into_iter()
            .filter(|i| loose || holds(&q, items[*i]))
            .take(most_of(query.scope, SectionId::Commands))
            .filter_map(|i| commands.get(i))
            .map(|(id, label, keys)| Hit {
                id: format!("cmd:{id}"),
                title: texts.t(label),
                note: keys.clone(),
                what: What::Command {
                    id,
                    keys: keys.clone(),
                },
            })
            .collect(),
    )
}

/// The icons: EXACTLY three strips, one per collection, each its hits in
/// order — the rows draw a page of each.
fn icons_section(
    set: Option<&ronoctua_iconset::Set>,
    grammar: Option<&ronoctua_iconset::Grammar>,
    refused: bool,
    query: &Query,
    subtag: &str,
    texts: &ronoctua_i18n::Texts,
) -> Filled {
    if query.words.trim().is_empty() && query.scope != Scope::Icons {
        return Filled::Hits(Vec::new());
    }
    let (Some(set), Some(grammar)) = (set, grammar) else {
        let _ = texts;
        return if refused {
            Filled::Refused(Said::key("fast-access-icons-refused".to_owned()))
        } else {
            Filled::Pending
        };
    };
    let q = ronoctua_iconset::Query::parse(&query.words, grammar);
    let most = most_of(query.scope, SectionId::Icons).max(60);
    let mut hits = Vec::new();
    for c in Collection::ALL {
        for h in ronoctua_iconset::search(set, c, &q, subtag, most) {
            hits.push(icon_hit(
                set,
                crate::icons_page::Icon {
                    collection: c,
                    index: h.index,
                },
            ));
        }
    }
    Filled::Hits(hits)
}

/// An icon's name, as its collection writes it (data).
#[must_use]
pub fn icon_name(set: &ronoctua_iconset::Set, icon: crate::icons_page::Icon) -> String {
    match icon.collection {
        Collection::NerdFonts => set
            .nerd
            .get(icon.index)
            .and_then(|g| g.name.clone())
            .unwrap_or_default(),
        Collection::Lucide => set
            .lucide
            .get(icon.index)
            .map(|f| f.name.clone())
            .unwrap_or_default(),
        Collection::Unicode => set
            .unicode
            .get(icon.index)
            .map(|c| c.name.clone())
            .unwrap_or_default(),
    }
}

/// An icon as a hit: its drawn text as the title, what copying it puts on
/// the clipboard as the note.
#[must_use]
pub fn icon_hit(set: &ronoctua_iconset::Set, icon: crate::icons_page::Icon) -> Hit {
    let (drawn, copied) = match icon.collection {
        Collection::NerdFonts => {
            let g = &set.nerd[icon.index];
            let ch = char::from_u32(g.cp).map(String::from).unwrap_or_default();
            (ch.clone(), ch)
        }
        Collection::Lucide => {
            let f = &set.lucide[icon.index];
            let ch = char::from_u32(f.cp).map(String::from).unwrap_or_default();
            (ch.clone(), ch)
        }
        Collection::Unicode => {
            let c = &set.unicode[icon.index];
            (c.drawn(), c.text())
        }
    };
    Hit {
        id: format!("icon:{}:{}", icon.collection.id(), icon.index),
        title: drawn,
        note: Some(copied),
        what: What::Icon(icon),
    }
}

/// What was opened from here, the most used lately first; the words narrow
/// it by its row's words.
fn recent_section(
    settings: &Settings,
    words: &str,
    now_s: i64,
    texts: &ronoctua_i18n::Texts,
) -> Filled {
    let q = ronoctua_apps::fold::fold(words.trim());
    let mut opened: Vec<&Opened> = settings
        .recent
        .iter()
        .filter(|o| q.is_empty() || ronoctua_apps::fold::fold(&o.title).contains(&q))
        .collect();
    opened.sort_by(|a, b| {
        frecency(b, now_s)
            .partial_cmp(&frecency(a, now_s))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.at_s.cmp(&a.at_s))
    });
    let _ = texts;
    if opened.is_empty() && q.is_empty() {
        return Filled::Refused(Said::key("fast-access-recent-none".to_owned()));
    }
    Filled::Hits(opened.into_iter().filter_map(recent_hit).collect())
}

/// A remembered entry as a hit — what its id says it was.
fn recent_hit(o: &Opened) -> Option<Hit> {
    let (kind, rest) = o.id.split_once(':')?;
    let what = match kind {
        "app" => What::App {
            desktop_id: rest.to_owned(),
            glyph: ronoctua_tokens::icon::GENERIC,
        },
        "file" | "folder" | "content" => What::File {
            path: PathBuf::from(rest),
            folder: kind == "folder",
            content: kind == "content",
            mime: None,
            size_bytes: None,
            mtime_s: None,
        },
        "yt" => What::Video(Box::new(Video {
            id: rest.to_owned(),
            url: format!("https://www.youtube.com/watch?v={rest}"),
            channel: None,
            duration_s: None,
            views: None,
            short: false,
            local: false,
            thumb_file: None,
            thumb: None,
        })),
        "man" => {
            let (name, section) = rest.rsplit_once('.')?;
            What::Man {
                name: name.to_owned(),
                section: section.to_owned(),
            }
        }
        "pkg" => What::Package {
            attr: rest.to_owned(),
        },
        "web" => What::Web {
            url: rest.to_owned(),
            snippet: None,
            source: "duckduckgo",
        },
        _ => return None,
    };
    Some(Hit {
        id: o.id.clone(),
        title: o.title.clone(),
        note: None,
        what,
    })
}

/// The search's settings, as rows under an empty box.
fn settings_rows(settings: &Settings, texts: &ronoctua_i18n::Texts) -> Vec<Hit> {
    let yes_no = |on: bool| {
        texts.t(if on {
            "fast-access-yes"
        } else {
            "fast-access-no"
        })
    };
    vec![
        Hit {
            id: "setting:park".to_owned(),
            title: texts.t(crate::key::FAST_ACCESS_SETTING_PARK),
            note: Some(yes_no(settings.park)),
            what: What::Setting(Setting::Park),
        },
        Hit {
            id: "setting:door".to_owned(),
            title: texts.t(crate::key::FAST_ACCESS_SETTING_DOOR),
            note: Some(texts.t(settings.door.key())),
            what: What::Setting(Setting::Door),
        },
        // THE WEB'S TWO SOURCES THE PERSON ADDS: each said as it is, the key
        // never shown — only whether one is kept.
        Hit {
            id: "setting:searxng".to_owned(),
            title: texts.t(crate::key::FAST_ACCESS_SETTING_SEARXNG),
            note: Some(
                settings
                    .searxng
                    .clone()
                    .unwrap_or_else(|| texts.t(crate::key::FAST_ACCESS_SEARXNG_NONE)),
            ),
            what: What::Setting(Setting::SearXNG),
        },
        Hit {
            id: "setting:brave".to_owned(),
            title: texts.t(crate::key::FAST_ACCESS_SETTING_BRAVE),
            note: Some(texts.t(if ronoctua_websearch::keys::has_brave() {
                "fast-access-brave-kept-short"
            } else {
                "fast-access-brave-none"
            })),
            what: What::Setting(Setting::Brave),
        },
    ]
}

/// The person's SearXNG, typed: an `https` address with a host is kept (its
/// base, no trailing `/`), nothing typed takes it away. `Err` with the
/// sentence that says why the address is not one. Pure.
///
/// # Errors
///
/// The address is not `https` with a host.
pub fn searxng_typed(
    settings: &Settings,
    typed: &str,
    texts: &ronoctua_i18n::Texts,
) -> Result<Settings, String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Ok(Settings {
            searxng: None,
            ..settings.clone()
        });
    }
    match ronoctua_websearch::sources::searxng_base(typed) {
        Some((_, base)) => Ok(Settings {
            searxng: Some(base),
            ..settings.clone()
        }),
        None => Err(texts.t_with(crate::key::FAST_ACCESS_SEARXNG_BAD, &[("address", typed)])),
    }
}

/// "Search the web in the kiosk": the browser's own page for the words.
fn kiosk_row(words: &str, texts: &ronoctua_i18n::Texts) -> Hit {
    Hit {
        id: "kiosk".to_owned(),
        title: texts.t_with(
            crate::key::FAST_ACCESS_WEB_KIOSK,
            &[("words", words.trim())],
        ),
        note: None,
        what: What::Kiosk(ronoctua_websearch::kiosk_address(words)),
    }
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "fast-access",
    commands: &[
        // THE SEARCH OVER EVERYTHING, in its four doors.
        ronoctua_cmd::Command {
            id: "fast-access-open",
            label: crate::key::CMD_FAST_ACCESS_OPEN,
        },
        ronoctua_cmd::Command {
            id: "fast-access-window",
            label: crate::key::CMD_FAST_ACCESS_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "fast-access-column",
            label: crate::key::CMD_FAST_ACCESS_COLUMN,
        },
        ronoctua_cmd::Command {
            id: "fast-access-float",
            label: crate::key::CMD_FAST_ACCESS_FLOAT,
        },
        // WHAT A DOOR OF THE SEARCH ASKS THE FOOTER TO OPEN, which outlives
        // the window: by name only, each with its argument.
        ronoctua_cmd::Command {
            id: "fast-access-launch",
            label: crate::key::CMD_FAST_ACCESS_LAUNCH,
        },
        ronoctua_cmd::Command {
            id: "fast-access-open-path",
            label: crate::key::CMD_FAST_ACCESS_OPEN_PATH,
        },
        ronoctua_cmd::Command {
            id: "fast-access-reveal",
            label: crate::key::CMD_FAST_ACCESS_REVEAL,
        },
        ronoctua_cmd::Command {
            id: "fast-access-man",
            label: crate::key::CMD_FAST_ACCESS_MAN,
        },
    ],
    bindings: &[
        // `d` — THE SEARCH OVER EVERYTHING: the doubled key the footer's door,
        // `w` the window's, `c` the column's; and `SPC`, the window floating
        // over the whole room — the one key a search should be.
        ("d d", "fast-access-open", None),
        ("d w", "fast-access-window", None),
        ("d c", "fast-access-column", None),
        ("SPC", "fast-access-float", None),
    ],
    groups: &[("d", crate::key::GROUP_FAST_ACCESS)],
    by_name: &[
        // WHAT A DOOR OF THE SEARCH HANDS THE FOOTER (`fast_access_window::Out`):
        // each takes the argument a key could not give.
        "fast-access-launch",
        "fast-access-open-path",
        "fast-access-reveal",
        "fast-access-man",
    ],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::fast_access::reached().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("the language")
    }

    /// What a door hands the in-process sources, with nothing of this
    /// machine in it.
    pub(crate) fn ground() -> Ground {
        Ground {
            catalogue: Arc::default(),
            commands: vec![(
                "fast-access-window",
                crate::key::CMD_FAST_ACCESS_WINDOW,
                Some("LEADER d w".to_owned()),
            )],
            zone: ronoctua_files::when::Zone::utc(),
            zoneinfo: None,
            queue_keys: "LEADER t".to_owned(),
        }
    }

    fn area() -> crate::icons_page::Area {
        crate::icons_page::Area {
            left: 1,
            top: 2,
            columns: 120,
            lines: 30,
            right_px: 1200.0,
            bottom_px: 800.0,
        }
    }

    fn file(id: &str) -> Hit {
        Hit {
            id: format!("file:/home/a/{id}"),
            title: id.to_owned(),
            note: None,
            what: What::File {
                path: PathBuf::from(format!("/home/a/{id}")),
                folder: false,
                content: false,
                mime: None,
                size_bytes: None,
                mtime_s: None,
            },
        }
    }

    /// FAILS IF: a prefix does not narrow, a word that starts with a slash is
    /// read as a prefix, the slash alone lists nothing, or `/t xx` does not
    /// send the translation there.
    #[test]
    fn a_prefix_narrows_and_a_path_does_not() {
        let q = parse("/y lofi beats");
        assert_eq!((q.scope, q.words.as_str()), (Scope::YouTube, "lofi beats"));
        assert_eq!(parse("/").scope, Scope::Prefixes);
        assert_eq!(parse("  / ").scope, Scope::Prefixes);
        let path = parse("/archive");
        assert_eq!((path.scope, path.words.as_str()), (Scope::All, "/archive"));
        let t = parse("/t pt good morning");
        assert_eq!(t.scope, Scope::Translate);
        assert_eq!(t.tongue.as_deref(), Some("pt"));
        assert_eq!(t.words, "good morning");
        let one = parse("/t hello");
        assert_eq!((one.tongue, one.words.as_str()), (None, "hello"));
        assert_eq!(parse("2+2").scope, Scope::All);
        // A SPACE TYPED KEY BY KEY is kept: `/g arrow` is the icons' scope.
        let t = texts("en-GB");
        let mut view = View::new(Settings::default(), &t);
        for c in "/g arrow".chars() {
            let _ = view.key(Press::Type(c.to_string()), area(), &ground(), &t);
        }
        assert_eq!(view.text(), "/g arrow");
        assert_eq!(view.query().scope, Scope::Icons);
        // EVERY PREFIX IS A SCOPE'S, one letter each.
        for (letter, scope) in PREFIXES {
            assert_eq!(parse(&format!("/{letter} x")).scope, scope, "/{letter}");
            assert_eq!(scope.letter(), Some(letter));
        }
    }

    /// FAILS IF: a sum needs its prefix — "2+2" typed alone is answered in
    /// the box, in-process, before any thread is asked.
    #[test]
    fn two_plus_two_answers_without_a_prefix() {
        let t = texts("en-GB");
        let mut view = View::new(Settings::default(), &t);
        let errands = view.typed("2+2", &ground(), &t);
        assert!(
            errands.iter().any(|e| matches!(e, Errand::Search(_))),
            "{errands:?}"
        );
        let math = view
            .sections()
            .iter()
            .find(|s| s.id == SectionId::Math)
            .expect("a sum's section");
        let hit = math.hits().first().expect("an answer");
        assert_eq!(copy_of(hit, &t).as_deref(), Some("4"));
        assert_eq!(view.current().map(|h| h.id.as_str()), Some("math"));
    }

    /// FAILS IF: "everything" fills with loose subsequences of what was
    /// typed — six letters match half the registry — or a narrowed scope
    /// loses them, or a section of "everything" grows past its share.
    #[test]
    fn everything_takes_a_piece_and_a_scope_takes_a_subsequence() {
        let t = texts("en-GB");
        let commands: Vec<(&'static str, &'static str, Option<String>)> = (0..20)
            .map(|_| {
                (
                    "fast-access-window",
                    crate::key::CMD_FAST_ACCESS_WINDOW,
                    None,
                )
            })
            .collect();
        let g = Ground {
            commands,
            ..ground()
        };
        let mut view = View::new(Settings::default(), &t);
        let _ = view.typed("window", &g, &t);
        let shown = |v: &View| {
            v.sections()
                .iter()
                .find(|s| s.id == SectionId::Commands)
                .map_or(0, |s| s.hits().len())
        };
        assert_eq!(shown(&view), most_of(Scope::All, SectionId::Commands));
        let _ = view.key(Press::Clear, area(), &g, &t);
        let _ = view.typed("sewi", &g, &t);
        assert_eq!(shown(&view), 0, "a loose subsequence in everything");
        let _ = view.key(Press::Clear, area(), &g, &t);
        let _ = view.typed("/: sewi", &g, &t);
        assert!(shown(&view) > 0, "the commands' scope lost a subsequence");
    }

    /// FAILS IF: the slash alone lists fewer than every prefix, or a row of
    /// it does not narrow the box to its letter.
    #[test]
    fn the_slash_alone_lists_every_way_to_narrow() {
        let t = texts("pt-BR");
        let mut view = View::new(Settings::default(), &t);
        let _ = view.typed("/", &ground(), &t);
        assert_eq!(view.hits().count(), PREFIXES.len());
        let _ = view.key(Press::Down, area(), &ground(), &t);
        let _ = view.key(Press::Enter, area(), &ground(), &t);
        assert_eq!(view.text(), "/v ");
        assert_eq!(view.query().scope, Scope::Video);
    }

    /// FAILS IF: the cursor follows a row's PLACE when answers arrive, an
    /// answer of an older generation is taken, or data moves the page.
    #[test]
    fn the_cursor_keeps_its_hit_and_data_never_moves_the_page() {
        let t = texts("en-GB");
        let mut view = View::new(Settings::default(), &t);
        let _ = view.typed("report", &ground(), &t);
        let generation = view.generation();
        assert!(view.arrived(Answer {
            generation,
            section: SectionId::Files,
            filled: Filled::Hits(vec![file("a"), file("b")]),
        }));
        view.point_at("file:/home/a/b");
        view.offset = 1;
        // A COMMIT OF THE INDEX: the same question answered again, with a
        // new file before the cursor's.
        assert!(view.arrived(Answer {
            generation,
            section: SectionId::Files,
            filled: Filled::Hits(vec![file("0"), file("a"), file("b")]),
        }));
        assert_eq!(view.cursor_id(), Some("file:/home/a/b"));
        assert_eq!(view.current().map(|h| h.title.as_str()), Some("b"));
        assert_eq!(view.offset, 1, "data moved the page");
        // AN OLDER GENERATION'S ANSWER is dropped.
        assert!(!view.arrived(Answer {
            generation: generation.wrapping_sub(1),
            section: SectionId::Files,
            filled: Filled::Hits(vec![file("stale")]),
        }));
        assert!(view.hits().all(|h| h.title != "stale"));
        // AND A HIT THAT LEAVES gives the cursor to the first, without
        // forgetting its own: it comes back when its hit does.
        assert!(view.arrived(Answer {
            generation,
            section: SectionId::Files,
            filled: Filled::Hits(vec![file("a")]),
        }));
        assert_eq!(view.cursor_id(), Some("file:/home/a/b"));
        assert_ne!(view.current().map(|h| h.title.as_str()), Some("b"));
    }

    fn application(id: &str, name: &str) -> ronoctua_apps::DesktopEntry {
        ronoctua_apps::DesktopEntry {
            id: id.to_owned(),
            name: name.to_owned(),
            description: String::new(),
            icon: String::new(),
            categories: Vec::new(),
            mime_types: Vec::new(),
            command: (id.to_owned(), Vec::new()),
            terminal: false,
            working_directory: None,
            startup_wm_class: None,
            key: ronoctua_apps::fold::fold(name),
        }
    }

    /// FAILS IF: with nobody having moved the cursor, a section that answers
    /// ABOVE the hit on screen takes the highlight (and the detail, and what
    /// `RET` opens) — the cursor is the first hit's by its identity, not by
    /// its place; or the launcher's scan arriving asks everything again,
    /// sending the page to the top and dropping what is in flight; or a new
    /// key keeps an old answer's cursor.
    #[test]
    fn the_first_answer_holds_the_cursor_when_more_arrive_above() {
        let t = texts("en-GB");
        let mut view = View::new(Settings::default(), &t);
        let _ = view.typed("report", &ground(), &t);
        view.with_sections(vec![
            Section {
                id: SectionId::Icons,
                filled: Filled::Pending,
            },
            Section {
                id: SectionId::Files,
                filled: Filled::Pending,
            },
        ]);
        let generation = view.generation();
        assert!(view.arrived(Answer {
            generation,
            section: SectionId::Files,
            filled: Filled::Hits(vec![file("a"), file("b")]),
        }));
        assert_eq!(view.current().map(|h| h.title.as_str()), Some("a"));
        // THE ICONS ANSWER LATER, ABOVE THE FILES.
        assert!(view.arrived(Answer {
            generation,
            section: SectionId::Icons,
            filled: Filled::Hits(vec![file("icon")]),
        }));
        assert_eq!(
            view.current().map(|h| h.title.as_str()),
            Some("a"),
            "data above moved the cursor"
        );
        assert_eq!(view.cursor_id(), None, "nobody moved the cursor");
        // THE LAUNCHER'S SCAN: the applications' section, in place.
        view.offset = 2;
        let mut g = ground();
        g.catalogue = Arc::new(vec![application("report-tool", "Report tool")]);
        assert!(view.apps_are(&g));
        assert_eq!(view.generation(), generation, "the scan asked again");
        assert_eq!(view.offset, 2, "the scan moved the page");
        assert_eq!(view.current().map(|h| h.title.as_str()), Some("a"));
        let ids: Vec<SectionId> = view.sections().iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            [SectionId::Apps, SectionId::Icons, SectionId::Files],
            "the applications' section is not where the scope puts it"
        );
        // A NEW KEY starts at the top of its own answers.
        let _ = view.key(Press::Back, area(), &g, &t);
        assert_eq!(view.text(), "repor");
        assert_eq!(view.cursor_id(), None);
        assert_eq!(
            view.current().map(|h| h.id.as_str()),
            Some("app:report-tool")
        );
    }

    /// FAILS IF: `RET` on a program to install queues it without the
    /// programs' centre's questions — a licence that is not free, a download
    /// past the person's threshold — or a question does not start on `no`, or
    /// `no` installs, or the queue's order loses what was said yes to.
    #[test]
    fn a_program_asks_the_centres_questions_before_the_queue() {
        use ronoctua_software::found::Badge;
        let t = texts("en-GB");
        let mut view = View::new(Settings::default(), &t);
        view.set_text("/p steam");
        view.with_sections(vec![Section {
            id: SectionId::Packages,
            filled: Filled::Hits(vec![Hit {
                id: "pkg:steam".to_owned(),
                title: "steam".to_owned(),
                note: None,
                what: What::Package {
                    attr: "steam".to_owned(),
                },
            }]),
        }]);
        view.detail.program = Some((
            "steam".to_owned(),
            crate::fast_access_search::Program {
                details: Ok(ronoctua_software::details::Details {
                    badges: vec![Badge::Unfree],
                    download_bytes: Some(3_000_000_000),
                    ..Default::default()
                }),
                unfree_let: false,
                ask_above_mb: 500,
            },
        ));
        let g = ground();
        // THE LICENCE FIRST, `no` under the cursor — and `no` installs nothing.
        assert!(view.key(Press::Enter, area(), &g, &t).is_empty());
        let q = view.detail.confirm.clone().expect("the licence's question");
        assert_eq!(q.cursor, 0);
        assert_eq!(q.answers[0], t.t(crate::key::SOFTWARE_NO));
        assert!(view.key(Press::Enter, area(), &g, &t).is_empty());
        assert!(view.detail.confirm.is_none());
        // YES TO THE LICENCE: the change carries it.
        let _ = view.key(Press::Escape, area(), &g, &t);
        assert_eq!(view.focus, Focus::Results);
        let _ = view.key(Press::Enter, area(), &g, &t);
        let _ = view.key(Press::Down, area(), &g, &t);
        let yes = view.key(Press::Enter, area(), &g, &t);
        let [
            Errand::Amend(crate::fast_access_detail::Amend::Install {
                attr,
                name,
                consent,
            }),
        ] = yes.as_slice()
        else {
            panic!("{yes:?}");
        };
        assert!(consent.unfree && !consent.big);
        // THEN THE SIZE, past the threshold — and only then the queue.
        assert!(view.install(attr, name, *consent, &t).is_empty());
        assert!(view.detail.confirm.is_some(), "the download was not asked");
        let big = ronoctua_software::order::Consent {
            big: true,
            ..*consent
        };
        let queued = view.install(attr, name, big, &t);
        assert!(
            queued.contains(&Errand::Install {
                attr: "steam".to_owned(),
                name: "steam".to_owned(),
                consent: big,
            }),
            "{queued:?}"
        );
        // A LICENCE THE PERSON LET BEFORE is not asked again.
        if let Some((_, p)) = view.detail.program.as_mut() {
            p.unfree_let = true;
            p.ask_above_mb = 0;
        }
        view.detail.confirm = None;
        let again = view.install("steam", "steam", Default::default(), &t);
        assert!(
            again.iter().any(|e| matches!(e, Errand::Install { .. })),
            "{again:?}"
        );
    }

    /// FAILS IF: a SearXNG address that is not `https` with a host is kept,
    /// nothing typed does not take it away, or a refusal of the Brave key
    /// has no words in a language.
    #[test]
    fn the_persons_searxng_is_an_https_address_or_nothing() {
        use ronoctua_websearch::keys::KeyTrouble;
        let t = texts("en-GB");
        let kept =
            searxng_typed(&Settings::default(), " https://sx.example/ ", &t).expect("an address");
        assert_eq!(kept.searxng.as_deref(), Some("https://sx.example"));
        for wrong in ["http://sx.example", "sx.example", "https://", "https://a b"] {
            assert!(searxng_typed(&kept, wrong, &t).is_err(), "{wrong} was kept");
        }
        assert_eq!(
            searxng_typed(&kept, "  ", &t).expect("nothing").searxng,
            None
        );
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for why in [
                KeyTrouble::NoHome,
                KeyTrouble::Empty,
                KeyTrouble::Malformed,
                KeyTrouble::Unwritable,
            ] {
                let key = format!("fast-access-web-key-{}", why.id());
                assert!(t.has(&key), "{language}: {key}");
            }
            for key in [ASK_SEARXNG, ASK_BRAVE] {
                assert!(t.has(&format!("ask-{key}")), "{language}: ask-{key}");
            }
        }
    }

    /// FAILS IF: rates that did not come leave `/$` "searching" for ever
    /// instead of saying why; the older rates on disk are not used when there
    /// are some, or the answer does not say which day's they are; or a
    /// failure of an older key's fetch touches the section of a newer one.
    #[test]
    fn rates_that_did_not_come_are_said_or_the_old_ones_dated() {
        use ronoctua_calc::rates::{Currency, Day, Rates, Why};
        let t = texts("en-GB");
        let g = ground();
        let mut view = View::new(Settings::default(), &t);
        view.set_text("/$ 10 eur in usd");
        view.with_sections(vec![Section {
            id: SectionId::Currency,
            filled: Filled::Pending,
        }]);
        let generation = view.generation();
        // AN OLDER KEY'S FAILURE is not this one's.
        assert!(!view.rates_failed(generation.wrapping_sub(1), Why::Unreachable, None, &g, &t));
        assert_eq!(view.sections()[0].filled, Filled::Pending);
        assert!(view.rates_failed(generation, Why::Unreachable, None, &g, &t));
        assert_eq!(
            view.sections()[0].filled,
            Filled::Refused(Said::key("calc-rates-why-unreachable".to_owned()))
        );
        // WITH RATES ON DISK, HOWEVER OLD: the sum, dated by them.
        view.with_sections(vec![Section {
            id: SectionId::Currency,
            filled: Filled::Pending,
        }]);
        let old = Rates {
            day: Day {
                year: 2026,
                month: 9,
                day: 1,
            },
            per_euro: [(
                Currency::new("USD").expect("a currency"),
                ronoctua_calc::BigRational::from_integer(2.into()),
            )]
            .into_iter()
            .collect(),
        };
        assert!(view.rates_failed(generation, Why::Unreachable, Some(old.clone()), &g, &t));
        let hit = view.current().cloned().expect("an answer at the old rates");
        assert!(matches!(hit.what, What::Math(_)), "{hit:?}");
        let rows = crate::fast_access_detail::rows_of(&view, &hit, &t);
        let dated = t.t_with(
            "calc-rates-of",
            &[("date", &crate::calc::rates_day(&old, &t))],
        );
        assert!(
            rows.iter().any(|r| r.label == dated),
            "the answer does not say which day's rates: {rows:?}"
        );
    }

    /// FAILS IF: "search the web in the kiosk" builds an address the kiosk
    /// refuses (`ronoctua_browser::url::allowed`) — the row would be a press
    /// that opens nothing.
    #[test]
    fn the_kiosks_search_is_an_address_the_kiosk_takes() {
        let t = texts("pt-BR");
        for words in ["report", "rust & go", "é? #x / y", "  spaces  ", "日本語"] {
            let hit = kiosk_row(words, &t);
            let What::Kiosk(url) = &hit.what else {
                panic!("{hit:?}");
            };
            assert!(ronoctua_browser::url::allowed(url), "{words:?} → {url}");
        }
    }

    fn video(id: &str) -> Hit {
        Hit {
            id: format!("yt:{id}"),
            title: id.to_owned(),
            note: None,
            what: What::Video(Box::new(Video {
                id: id.to_owned(),
                url: format!("https://www.youtube.com/watch?v={id}"),
                channel: None,
                duration_s: None,
                views: None,
                short: false,
                local: true,
                thumb_file: Some(PathBuf::from(format!("/thumbs/{id}.jpg"))),
                thumb: None,
            })),
        }
    }

    /// FAILS IF: in "everything", YouTube's answer takes the place of the
    /// catalogue's videos (or the catalogue's of YouTube's), the catalogue's
    /// stop coming first, or a video both found shows twice — the two
    /// threads answer in either order.
    #[test]
    fn everything_keeps_the_catalogue_first_and_youtube_after() {
        let t = texts("en-GB");
        let online = |id: &str| {
            let mut h = video(id);
            if let What::Video(v) = &mut h.what {
                v.local = false;
            }
            h
        };
        for youtube_first in [false, true] {
            let mut view = View::new(Settings::default(), &t);
            let _ = view.typed("lo", &ground(), &t);
            view.with_sections(vec![Section {
                id: SectionId::Videos,
                filled: Filled::Pending,
            }]);
            let g = view.generation();
            let catalogue = Answer {
                generation: g,
                section: SectionId::Videos,
                filled: Filled::Hits(vec![video("a1"), video("b2")]),
            };
            let youtube = Answer {
                generation: g,
                section: SectionId::Videos,
                filled: Filled::Hits(vec![online("b2"), online("c3")]),
            };
            let (one, two) = if youtube_first {
                (youtube, catalogue)
            } else {
                (catalogue, youtube)
            };
            assert!(view.arrived(one));
            assert!(view.arrived(two));
            let ids: Vec<&str> = view.hits().map(|h| h.id.as_str()).collect();
            assert_eq!(
                ids,
                ["yt:a1", "yt:b2", "yt:c3"],
                "youtube first: {youtube_first}"
            );
        }
    }

    /// FAILS IF: a video's picture is asked twice, or a video found again by
    /// the next key is left without the picture already at hand.
    #[test]
    fn a_picture_is_asked_once_and_found_again() {
        let t = texts("en-GB");
        let mut view = View::new(Settings::default(), &t);
        let _ = view.typed("/y lo", &ground(), &t);
        let g = view.generation();
        assert!(view.arrived(Answer {
            generation: g,
            section: SectionId::YouTube,
            filled: Filled::Hits(vec![video("a1")]),
        }));
        let wanted = view.pictures_wanted();
        assert_eq!(
            wanted,
            vec![("a1".to_owned(), PathBuf::from("/thumbs/a1.jpg"))]
        );
        assert!(view.pictures_wanted().is_empty(), "asked twice");
        let rgba = Arc::new(ronoctua_render::image::Rgba::new(1, 1, vec![0, 0, 0, 255]).unwrap());
        assert!(view.pictured("a1", "key-a1".to_owned(), rgba));
        // THE NEXT KEY: the same video, a new hit with no picture yet.
        let _ = view.typed("f", &ground(), &t);
        let g = view.generation();
        assert!(view.arrived(Answer {
            generation: g,
            section: SectionId::YouTube,
            filled: Filled::Hits(vec![video("a1")]),
        }));
        assert!(view.pictures_wanted().is_empty(), "asked again");
        let thumb = view.hits().find_map(|h| match &h.what {
            What::Video(v) => v.thumb.clone(),
            _ => None,
        });
        assert_eq!(thumb.as_deref(), Some("key-a1"));
    }

    /// FAILS IF: `RET` on an application opens nothing, opens it without
    /// remembering it, or does not say that what was asked is done.
    #[test]
    fn enter_on_an_application_launches_it_through_the_footer() {
        let t = texts("en-GB");
        let mut view = View::new(Settings::default(), &t);
        view.set_text("fire");
        view.with_sections(vec![Section {
            id: SectionId::Apps,
            filled: Filled::Hits(vec![Hit {
                id: "app:firefox.desktop".to_owned(),
                title: "Firefox".to_owned(),
                note: None,
                what: What::App {
                    desktop_id: "firefox.desktop".to_owned(),
                    glyph: ronoctua_tokens::icon::GENERIC,
                },
            }]),
        }]);
        let errands = view.key(Press::Enter, area(), &ground(), &t);
        assert!(errands.contains(&Errand::Launch("firefox.desktop".to_owned())));
        assert!(errands.contains(&Errand::Done));
        let Some(Errand::Keep(kept)) = errands.first() else {
            panic!("not remembered: {errands:?}");
        };
        assert_eq!(
            kept.recent.last().map(|o| o.id.as_str()),
            Some("app:firefox.desktop")
        );
    }

    /// FAILS IF: what was opened is forgotten past fewer than the ceiling, a
    /// second opening is a second entry, or the most used weighs less than
    /// the once-used of the same age.
    #[test]
    fn what_was_opened_is_remembered_by_its_id() {
        let hit = file("a");
        let once = remembered(Settings::default(), &hit, 100, 3);
        let twice = remembered(once, &hit, 200, 3);
        assert_eq!(twice.recent.len(), 1);
        assert_eq!(twice.recent[0].uses, 2);
        assert_eq!(twice.recent[0].at_s, 200);
        let mut s = twice;
        for id in ["b", "c", "d"] {
            s = remembered(s, &file(id), 300, 3);
        }
        assert_eq!(s.recent.len(), 3, "past the ceiling, the oldest goes");
        assert!(s.recent.iter().all(|o| o.id != "file:/home/a/a"));
        let many = Opened {
            id: "x".to_owned(),
            title: String::new(),
            uses: 9,
            at_s: 0,
        };
        let one = Opened {
            uses: 1,
            ..many.clone()
        };
        assert!(frecency(&many, 0) > frecency(&one, 0));
        let week = 7 * 24 * 3600;
        assert!(
            frecency(&many, week) < frecency(&many, 0),
            "weight halves with time"
        );
    }

    /// FAILS IF: `Esc` leaves a whole detail open under it instead of giving
    /// the results back first, or does not close the view after.
    #[test]
    fn escape_gives_the_area_back_then_closes() {
        let t = texts("en-GB");
        let mut view = View::new(Settings::default(), &t);
        view.set_text("a");
        view.with_sections(vec![Section {
            id: SectionId::Files,
            filled: Filled::Hits(vec![file("a")]),
        }]);
        let _ = view.key(Press::Whole, area(), &ground(), &t);
        assert!(view.whole);
        assert!(view.key(Press::Escape, area(), &ground(), &t).is_empty());
        assert!(!view.whole);
        assert_eq!(
            view.key(Press::Escape, area(), &ground(), &t),
            vec![Errand::Close]
        );
    }

    /// FAILS IF: a scope, a section, or a key of the view lacks its words in
    /// a language — every text the view draws is a key in both.
    #[test]
    fn every_scope_section_and_key_has_its_words() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for scope in Scope::ALL {
                for key in [
                    format!("fast-access-scope-{}", scope.id()),
                    format!("fast-access-scope-{}-does", scope.id()),
                ] {
                    if scope == Scope::All && key.ends_with("-does") {
                        continue;
                    }
                    assert!(t.has(&key), "{language}: {key}");
                }
            }
            for section in SectionId::ALL {
                let key = format!("fast-access-section-{}", section.id());
                assert!(t.has(&key), "{language}: {key}");
            }
            for (_, id) in OF_THE_VIEW.iter().chain(OF_YOUTUBE).chain(OF_A_STRIP) {
                assert!(t.has(id), "{language}: {id}");
                assert!(t.has(&format!("about-{id}")), "{language}: about-{id}");
            }
            for id in COMMANDS {
                assert!(t.has(&format!("cmd-{id}")), "{language}: cmd-{id}");
            }
            for door in [VideoDoor::Ask, VideoDoor::Player, VideoDoor::Kiosk] {
                assert!(t.has(door.key()), "{language}: {}", door.key());
            }
        }
    }

    /// FAILS IF: a key the view teaches is not read by its reader — a hint
    /// that does nothing.
    #[test]
    fn every_key_taught_is_read() {
        for (notation, id) in OF_THE_VIEW.iter().chain(OF_YOUTUBE).chain(OF_A_STRIP) {
            let key = ronoctua_keymap::Key::parse(notation).expect("a key");
            assert!(
                press_of_key(key).is_some(),
                "{notation} ({id}) reads as nothing"
            );
        }
        for (notation, _) in crate::consent_rows::OF_THE_CONSENT {
            let key = ronoctua_keymap::Key::parse(notation).expect("a key");
            assert_eq!(press_of_key(key), Some(Press::Allow));
        }
    }
}
