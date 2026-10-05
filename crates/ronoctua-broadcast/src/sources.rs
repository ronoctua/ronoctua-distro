//! The catalogue's sources: what a pasted link is, what each kind of source
//! yields, and where a person's own addresses are kept.
//!
//! # A link, recognised
//!
//! [`sniff`] takes the address a person pasted and the first bytes it
//! answered, and says what it is — an M3U list (an IPTV provider's, iptv-org's,
//! anybody's), an RSS or Atom feed (a podcast, a channel's video feed), a
//! YouTube channel or playlist, an archive.org collection — or refuses it
//! with a reason from a closed set ([`Refused`]; the sentence is the door's).
//!
//! # What each yields
//!
//! A [`Harvest`]: categories and items, ready for
//! [`crate::catalog::Writer::replace`]. [`playlist`] reads a list's groups
//! (`group-title`), logos, guide ids, countries and languages; [`feed`] reads
//! enclosures, `itunes:image`, `media:*` and categories; [`youtube`] lists a
//! channel's videos and its playlists (each a category) through yt-dlp; and
//! [`defaults`] turns the default sources' answers — iptv-org's index, the
//! archive's collections, the live searches, NASA's, Commons' and PeerTube's
//! pages — into the same.
//!
//! # Themes and the adult store
//!
//! A person's group lands under a theme by the table's aliases
//! ([`theme_of`]), and a group an adult word names ([`is_adult`],
//! `adultWords` in `assets/channels.nix`) always goes to the adult store.
//!
//! # A person's addresses are secret
//!
//! An IPTV provider's address carries the person's account in its query
//! (`?username=…&password=…`). So a person's address is an [`Address`]: its
//! `Debug` is masked and it has no `Display` and no `Serialize`; a door shows
//! [`Address::shown`] (`host/…`); and the one file that keeps them,
//! `sources.json`, is written `0600` from its first byte ([`book`]). Nothing
//! here logs.

use std::sync::OnceLock;

use ronoctua_consent::keys::Secret;
use serde::{Deserialize, Serialize};

use crate::Why;
use crate::catalog::{fold, has_phrase, words_of};
use crate::table::{ADULT_WORDS, THEMES};

pub mod book;
pub mod defaults;
pub mod feed;
pub mod playlist;
pub mod youtube;

pub use book::{Book, Kept, PersonSource, Unkept, Unread};
pub use feed::{Fed, feed};
pub use playlist::playlist;
pub use youtube::{Lister, youtube};

pub use crate::catalog::{Harvest, NewCategory, NewItem};

/// What kind of source a catalogue source is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// iptv-org's public index of TV channels (a default source).
    IptvOrg,
    /// The Internet Archive's collections, the themes' and the public-domain
    /// ones (a default source).
    Archive,
    /// YouTube's live searches (a default source).
    #[serde(rename = "youtube-lives")]
    Lives,
    /// The NASA Image and Video Library (a default source).
    Nasa,
    /// Wikimedia Commons' categories of free video (a default source).
    Commons,
    /// PeerTube, through SepiaSearch (a default source).
    #[serde(rename = "peertube")]
    PeerTube,
    /// A person's M3U list.
    Playlist,
    /// A person's RSS or Atom feed.
    Feed,
    /// A person's YouTube channel or playlist.
    #[serde(rename = "youtube")]
    YouTube,
    /// A person's archive.org collection.
    Collection,
}

impl Kind {
    /// Every kind, the defaults first.
    pub const ALL: [Self; 10] = [
        Self::IptvOrg,
        Self::Archive,
        Self::Lives,
        Self::Nasa,
        Self::Commons,
        Self::PeerTube,
        Self::Playlist,
        Self::Feed,
        Self::YouTube,
        Self::Collection,
    ];

    /// Its kebab-case name: the database's word and the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::IptvOrg => "iptv-org",
            Self::Archive => "archive",
            Self::Lives => "youtube-lives",
            Self::Nasa => "nasa",
            Self::Commons => "commons",
            Self::PeerTube => "peertube",
            Self::Playlist => "playlist",
            Self::Feed => "feed",
            Self::YouTube => "youtube",
            Self::Collection => "collection",
        }
    }

    /// The kind a word names.
    #[must_use]
    pub fn of(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == word)
    }

    /// Whether it is one of the default sources (declared in
    /// `assets/channels.nix`) rather than a person's.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(
            self,
            Self::IptvOrg
                | Self::Archive
                | Self::Lives
                | Self::Nasa
                | Self::Commons
                | Self::PeerTube
        )
    }
}

/// Why a source gave nothing, as a closed set. No sentence: the words are the
/// door's, one per variant ([`Refused::id`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Refused {
    /// It is not a web address (a scheme other than `http`/`https`, no host,
    /// a space).
    NotAnAddress,
    /// It is one video or one stream, not a list of them.
    NotAList,
    /// It is none of the shapes read: M3U, RSS, Atom, a YouTube channel or
    /// playlist, an archive.org collection.
    Unknown,
    /// The person has not let the host this default source asks.
    NotLet,
    /// A redirect from an `https` address down to `http`, which is never
    /// followed (a person's own `http` address is asked, in the clear).
    PlainHttp,
    /// The host could not be reached.
    Unreachable,
    /// The host refused the question.
    Declined,
    /// The host asked to be asked less often.
    TooManyAsks,
    /// There is nothing there.
    NothingFound,
    /// The answer is not what this reads.
    Malformed,
    /// The answer is larger than its ceiling.
    TooLarge,
    /// It was read, and nothing in it plays.
    NothingPlayable,
    /// This installation has no yt-dlp.
    NoYtDlp,
    /// yt-dlp could not list it.
    ListingFailed,
}

impl Refused {
    /// Every refusal, for a test that walks the words.
    pub const ALL: [Self; 14] = [
        Self::NotAnAddress,
        Self::NotAList,
        Self::Unknown,
        Self::NotLet,
        Self::PlainHttp,
        Self::Unreachable,
        Self::Declined,
        Self::TooManyAsks,
        Self::NothingFound,
        Self::Malformed,
        Self::TooLarge,
        Self::NothingPlayable,
        Self::NoYtDlp,
        Self::ListingFailed,
    ];

    /// Its kebab-case name: what a source's `last_error` keeps, and the
    /// suffix of the key a door says it with.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotAnAddress => "not-an-address",
            Self::NotAList => "not-a-list",
            Self::Unknown => "unknown",
            Self::NotLet => "not-let",
            Self::PlainHttp => "plain-http",
            Self::Unreachable => "unreachable",
            Self::Declined => "declined",
            Self::TooManyAsks => "too-many-asks",
            Self::NothingFound => "nothing-found",
            Self::Malformed => "malformed",
            Self::TooLarge => "too-large",
            Self::NothingPlayable => "nothing-playable",
            Self::NoYtDlp => "no-yt-dlp",
            Self::ListingFailed => "listing-failed",
        }
    }
}

impl From<Why> for Refused {
    fn from(why: Why) -> Self {
        match why {
            Why::Unreachable => Self::Unreachable,
            Why::Declined => Self::Declined,
            Why::TooManyAsks => Self::TooManyAsks,
            Why::NothingFound => Self::NothingFound,
            Why::Malformed => Self::Malformed,
            Why::TooLarge => Self::TooLarge,
            Why::NothingPlayable => Self::NothingPlayable,
            Why::NothingToAsk => Self::Unknown,
            Why::NotAName => Self::NotAnAddress,
            Why::NotLet => Self::NotLet,
        }
    }
}

impl From<ronoctua_ytdlp::Trouble> for Refused {
    fn from(t: ronoctua_ytdlp::Trouble) -> Self {
        use ronoctua_ytdlp::Trouble as T;
        match t {
            T::Absent => Self::NoYtDlp,
            T::TooLarge => Self::TooLarge,
            T::NotAnAddress => Self::NotAnAddress,
            T::NotAList => Self::NotAList,
            T::NothingToAsk => Self::Unknown,
            T::Unstartable | T::Slow | T::Failed | T::Garbled => Self::ListingFailed,
        }
    }
}

// ---------------------------------------------------------------------------
// A PERSON'S ADDRESS
// ---------------------------------------------------------------------------

/// The address of a person's own source: secret, because it often carries
/// their account.
///
/// `Debug` prints [`Address::shown`] and nothing else; there is no `Display`
/// and no `Serialize`. The text itself is read with [`Address::expose`], at
/// the one place it is sent, and wiped when this is dropped.
#[derive(Clone, PartialEq, Eq)]
pub struct Address(Secret);

impl Address {
    /// A person's address, as they pasted it (trimmed).
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(Secret::new(text.trim()))
    }

    /// The address itself — for the request that fetches it, and nowhere
    /// else.
    #[must_use]
    pub fn expose(&self) -> &str {
        self.0.expose()
    }

    /// The address as a door may show it: its host, and `/…` when anything
    /// follows it — never a user, a password, a path or a query ([`shown`]).
    #[must_use]
    pub fn shown(&self) -> String {
        shown(self.0.expose())
    }
}

impl std::fmt::Debug for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Address({})", self.shown())
    }
}

/// Whether a person's address is `http`: what it answers crosses the
/// network NOT ENCRYPTED — anybody on the way reads the list and the account
/// in its address. Taken only for a source the person pasted, and said beside
/// it (`crate::internet::Internet::get_own`); a default source is `https`.
/// Pure.
#[must_use]
pub fn is_plain(url: &str) -> bool {
    url::Url::parse(url.trim()).is_ok_and(|u| u.scheme() == "http")
}

/// What [`shown`] gives for an address it will not name a host of.
pub const MASK: &str = "…";

/// An address masked: `iptv.example/…`. Pure.
///
/// Its host as the web reads it (the WHATWG parser, the one
/// [`crate::ask::host_of`] and every fetch use), and `/…` when anything
/// follows. An address with no host, or one [`is_ambiguous`] — whose "host"
/// would be a piece of the person's account — is [`MASK`] alone.
#[must_use]
pub fn shown(url: &str) -> String {
    let url = url.trim();
    let Ok(parsed) = url::Url::parse(url) else {
        return MASK.to_owned();
    };
    let Some(host) = parsed.host_str().filter(|h| !h.is_empty()) else {
        return MASK.to_owned();
    };
    if is_ambiguous(url) {
        return MASK.to_owned();
    }
    let more = !matches!(parsed.path(), "" | "/")
        || parsed.query().is_some()
        || parsed.fragment().is_some();
    if more {
        format!("{}/…", host.to_ascii_lowercase())
    } else {
        host.to_ascii_lowercase()
    }
}

/// Whether an address could be read as another host than the one its writer
/// meant. Every parser ends the host at the first `/`, `\`, `?` or `#`, so a
/// user or a password with one of those in it (`https://ana/w:pw@iptv.example/`,
/// `https://ana?x:pw@iptv.example/`) is read as the host `ana` and a path or
/// a query — and an `@` is left where a well-formed address has none: inside
/// a path segment, or in a query or fragment that comes before any path.
///
/// An `@` that opens a segment (`youtube.com/@handle`) or sits in the query
/// of an address with a path (`/get.php?username=ana@mail.example`) is not
/// one. A query with an `@` and no path before it
/// (`https://host.example?u=ana@mail.example`) is refused too: it cannot be
/// told from the second shape above.
#[must_use]
pub fn is_ambiguous(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let end = rest.find(['/', '\\', '?', '#']).unwrap_or(rest.len());
    let tail = &rest[end..];
    if tail.starts_with(['?', '#']) {
        let before_a_path = tail.split(['/', '\\']).next().unwrap_or("");
        return before_a_path.contains('@');
    }
    let path = tail.split(['?', '#']).next().unwrap_or("");
    path.split(['/', '\\'])
        .any(|segment| segment.find('@').is_some_and(|at| at > 0))
}

// ---------------------------------------------------------------------------
// SNIFFING A LINK
// ---------------------------------------------------------------------------

/// Which of a YouTube address's lists it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum YouTubeList {
    /// A channel: its videos, and each of its playlists as a category.
    Channel,
    /// One playlist.
    Playlist,
}

/// What a link turned out to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Sniffed {
    /// An M3U list.
    Playlist,
    /// An RSS, Atom or RSS 1.0 feed.
    Feed,
    /// A YouTube channel or playlist (asked of yt-dlp, not fetched).
    YouTube {
        /// Which.
        list: YouTubeList,
    },
    /// An archive.org collection.
    Collection {
        /// Its identifier.
        identifier: String,
    },
}

impl Sniffed {
    /// The kind of source it makes.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Playlist => Kind::Playlist,
            Self::Feed => Kind::Feed,
            Self::YouTube { .. } => Kind::YouTube,
            Self::Collection { .. } => Kind::Collection,
        }
    }

    /// Whether the address alone says what it is: a YouTube channel or
    /// playlist, one YouTube video, an archive.org collection — known without
    /// fetching it, and [`sniff`] needs no bytes. `None` when only its bytes
    /// can say: any other address, a YouTube or archive.org one included (a
    /// channel's feed at `youtube.com/feeds/videos.xml`, an archive.org RSS
    /// or M3U).
    #[must_use]
    pub fn by_address(url: &str) -> Option<Result<Self, Refused>> {
        by_address(url)
    }
}

/// The hosts a YouTube address is written with.
const YOUTUBE_HOSTS: [&str; 4] = [
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
];

/// The query parameter `name` of an address.
fn query_param<'a>(url: &'a str, name: &str) -> Option<&'a str> {
    let query = url.split_once('?')?.1.split('#').next()?;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == name && !v.is_empty()).then_some(v)
    })
}

/// The path of an address, without its query.
fn path_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let from = rest.find('/').unwrap_or(rest.len());
    let path = &rest[from..];
    path.split(['?', '#']).next().unwrap_or("")
}

/// What an address says it is by itself: the YouTube and archive.org shapes
/// that are known without a byte. Every other address — on those hosts too —
/// is `None`, and its bytes say.
fn by_address(url: &str) -> Option<Result<Sniffed, Refused>> {
    const PLAYLIST: Sniffed = Sniffed::YouTube {
        list: YouTubeList::Playlist,
    };
    const CHANNEL: Sniffed = Sniffed::YouTube {
        list: YouTubeList::Channel,
    };
    let host = crate::ask::host_of(url)?;
    let segments: Vec<&str> = path_of(url).split('/').filter(|s| !s.is_empty()).collect();
    let list = query_param(url, "list").is_some();
    if host == "youtu.be" {
        // A SHORT ADDRESS is one video, or one video in a playlist.
        return Some(if list {
            Ok(PLAYLIST)
        } else {
            Err(Refused::NotAList)
        });
    }
    if YOUTUBE_HOSTS.contains(&host.as_str()) {
        return match segments.first().copied() {
            Some("playlist" | "watch") if list => Some(Ok(PLAYLIST)),
            Some(s) if s.starts_with('@') && s.len() > 1 => Some(Ok(CHANNEL)),
            Some("channel" | "c" | "user") if segments.len() >= 2 => Some(Ok(CHANNEL)),
            Some("watch" | "shorts" | "live" | "embed") => Some(Err(Refused::NotAList)),
            // A CHANNEL'S FEED (`feeds/videos.xml`), and anything else: the
            // bytes say.
            _ => None,
        };
    }
    if host == "archive.org" || host == "www.archive.org" {
        return match segments.as_slice() {
            ["details", id, ..] if crate::archive::is_identifier(id) => {
                Some(Ok(Sniffed::Collection {
                    identifier: (*id).to_owned(),
                }))
            }
            // A COLLECTION'S RSS, a list under `download/`: the bytes say.
            _ => None,
        };
    }
    None
}

/// The name of the first element of an XML text — read past the declaration,
/// comments, processing instructions and a doctype, without parsing the rest
/// (the first bytes of a feed are not a whole document).
fn root_element(text: &str) -> Option<&str> {
    let mut rest = text;
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("<?") {
            rest = &after[after.find("?>")? + 2..];
        } else if let Some(after) = rest.strip_prefix("<!--") {
            rest = &after[after.find("-->")? + 3..];
        } else if let Some(after) = rest.strip_prefix("<!") {
            rest = &after[after.find('>')? + 1..];
        } else {
            let after = rest.strip_prefix('<')?;
            let end = after
                .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .unwrap_or(after.len());
            let name = &after[..end];
            return (!name.is_empty()).then_some(name);
        }
    }
}

/// What a link is: `url` as pasted, and the first bytes it answered (empty
/// when the address alone says — [`Sniffed::by_address`]). Pure.
///
/// # Errors
///
/// [`Refused::NotAnAddress`] (not a web address, no host, or one
/// [`is_ambiguous`] — a request would go to a piece of the person's
/// account), [`Refused::NotAList`] (one video, one HLS stream),
/// [`Refused::Unknown`].
pub fn sniff(url: &str, first_bytes: &[u8]) -> Result<Sniffed, Refused> {
    let url = url.trim();
    if !crate::json::is_web_address(url) || crate::ask::host_of(url).is_none() || is_ambiguous(url)
    {
        return Err(Refused::NotAnAddress);
    }
    if let Some(answer) = by_address(url) {
        return answer;
    }
    let text = String::from_utf8_lossy(first_bytes);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    if text
        .get(..7)
        .is_some_and(|head| head.eq_ignore_ascii_case("#EXTM3U"))
    {
        // A STREAM'S OWN INDEX (HLS) begins the same way, and is one stream.
        if ronoctua_playlist::is_hls(text) {
            return Err(Refused::NotAList);
        }
        return Ok(Sniffed::Playlist);
    }
    if text.starts_with('<') {
        let root = root_element(text).unwrap_or("");
        let local = root.rsplit(':').next().unwrap_or(root);
        if matches!(local, "rss" | "feed" | "RDF") {
            return Ok(Sniffed::Feed);
        }
    }
    Err(Refused::Unknown)
}

// ---------------------------------------------------------------------------
// THEMES AND THE ADULT STORE
// ---------------------------------------------------------------------------

/// A theme's id, and its aliases (and its id) as words.
type Folded = (&'static str, Vec<Vec<String>>);

/// Each theme's aliases (and its own id), as words, folded once.
fn aliases() -> &'static [Folded] {
    static ALIASES: OnceLock<Vec<Folded>> = OnceLock::new();
    ALIASES.get_or_init(|| {
        THEMES
            .iter()
            .map(|t| {
                let mut all: Vec<Vec<String>> = t.aliases.iter().map(|a| words_of(a)).collect();
                all.push(words_of(t.id));
                (t.id, all)
            })
            .collect()
    })
}

/// The theme a person's group, category or playlist is filed under: the
/// theme whose alias is among its words — the LONGEST such alias, so
/// "Science Fiction" is science fiction and not science; on a tie, the
/// theme the table lists first. `None` when no alias is.
#[must_use]
pub fn theme_of(name: &str) -> Option<&'static str> {
    let words = words_of(name);
    if words.is_empty() {
        return None;
    }
    let mut best: Option<(usize, &'static str)> = None;
    for (id, all) in aliases() {
        for alias in all {
            if has_phrase(&words, alias) && best.is_none_or(|(n, _)| alias.len() > n) {
                best = Some((alias.len(), id));
            }
        }
    }
    best.map(|(_, id)| id)
}

/// Whether an adult word (`adultWords`) is among the words of `name`.
#[must_use]
pub fn is_adult(name: &str) -> bool {
    static FOLDED: OnceLock<Vec<String>> = OnceLock::new();
    let adult = FOLDED.get_or_init(|| ADULT_WORDS.iter().map(|w| fold(w)).collect());
    words_of(name).iter().any(|w| adult.contains(w))
}

#[cfg(test)]
mod tests;
