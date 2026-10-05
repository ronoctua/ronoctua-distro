//! A list of things to play, read out of the file that names them.
//!
//! # Why a crate of its own
//!
//! Two programs read playlists: the file manager's preview
//! (`ronoctua-inspect::playlist`, which decides WHICH media types are one) and
//! the player daemon, which expands the spellings libmpv cannot open (XSPF and
//! ASX) into its queue. The daemon must not link `ronoctua-inspect` — that
//! crate brings every decoder the preview has — so the parsers live here, with
//! nothing but an XML reader beside them, and the two callers agree about what
//! an entry is because there is only one reader.
//!
//! # What this deliberately does NOT do
//!
//! It does not touch the network, and nothing here can be made to. A remote
//! entry is LISTED and never opened: fetching is `ronoctua-transfers`' work, a
//! separate daemon, and this crate gains no edge to it.
//!
//! It does not follow a playlist that names another playlist. That would be an
//! unbounded read driven by a file's content.
//!
//! It does not `stat` the entries. Resolving one is arithmetic on a `Path`;
//! asking the disk about N of them is N syscalls for a panel, and whether a
//! file is there is a truth the row tells when somebody plays it. The one
//! exception is [`own`]'s fallback for a moved folder, and even there the
//! question is asked through a function the CALLER hands in.
//!
//! # Our own two files
//!
//! [`own`] reads and writes `.rset` (media played together, with their
//! settings) and `.rqueue` (a queue: files, sets, named playlists and
//! streams). They live here, beside the other spellings, so the preview reads
//! them for free and the daemon links nothing new to save one.

pub mod own;

use std::path::{Path, PathBuf};

/// The most of a playlist that is read.
///
/// `text::BYTE_CEILING`'s number and its argument: a file that claims to be a
/// list of names should not be able to allocate four hundred megabytes in the
/// inspector's thread before anything checks its size.
pub const BYTE_CEILING: u64 = 1024 * 1024;

/// How many entries are ASKED how long they run, when the list does not say.
///
/// # A list that declares nothing still deserves a timeline
///
/// Most playlists carry `#EXTINF`, and those cost nothing here. A bare list of
/// names — the hand-written kind — carries no lengths at all, and without them
/// the transport has a play glyph and no track: the owner reported exactly
/// that, *"também a playlist sem linha de tempo"*.
///
/// So the gap is filled by asking, and asking costs a child process: measured
/// on this machine, `ffprobe` takes 113 ms whatever the file's size, because
/// what dominates is starting it. Sixteen of them is about a second and three
/// quarters, spent on the inspector's own thread, on the ONE file the cursor
/// is resting on — the thread drains to the newest ask, so walking a cursor
/// past a folder of playlists costs one of these and not one each.
///
/// Past the ceiling an entry keeps whatever the list said about it, which for
/// a bare list is nothing. It is a bound on a cost, not a promise about a
/// length.
pub const TIMED_CEILING: usize = 16;

/// How many entries are kept.
///
/// `text::LINE_CEILING`'s number, and the same reasoning: past a couple of
/// thousand rows nobody is reading the list, and the panel says it was cut.
pub const ENTRY_CEILING: usize = 2_000;

/// Which spelling of "a list of things to play" this turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The plain one: a name a line, with optional `#EXTINF:` before each.
    M3u,
    /// The ini one, whose fields are joined by a NUMBER rather than by order.
    Pls,
    /// The XML one, whose durations are in MILLISECONDS.
    Xspf,
    /// Microsoft's XML one, whose tag names are CASE-INSENSITIVE and whose
    /// durations are a clock.
    Asx,
    /// One of our own two, `.rset` or `.rqueue` ([`own`]), listed flat the way
    /// the preview lists the others. [`kind_by_name`] never answers it: the
    /// player asks [`own::kind_by_name`], because a set is not a list.
    Own,
}

/// Where an entry points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Where {
    /// A file on this machine, already resolved against the playlist's folder.
    Local(PathBuf),
    /// Somewhere else, shown and never opened.
    Remote(String),
    /// A name this system will not turn into a path.
    ///
    /// A NUL or a newline inside one — which no filesystem accepts and no
    /// honest playlist writes.
    Refused,
}

/// One thing to play.
///
/// `Eq` is not derived, and it is the declared duration's doing: a float has
/// no total equality. `PartialEq` is what the comparisons here want — "is this
/// still the same answer" — which is the decision `files::Looked` already made
/// for the same reason.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Where it points.
    pub at: Where,
    /// What the playlist called it, when it said.
    pub title: Option<String>,
    /// How long the playlist SAYS it runs.
    ///
    /// The playlist's word and not the file's — nothing here opens the file to
    /// check, and a list that lies about a length is still the list.
    pub seconds: Option<f64>,
    /// What the playlist wrote, before it was resolved.
    ///
    /// The row shows this, because a person who wrote `../Album/01.flac`
    /// should see that and not a path they never typed. The resolved one goes
    /// on the message line.
    pub written: String,
    /// What an extended M3U said about it beyond a length and a title — its
    /// group, its logo, its guide id, its country and language, and the two
    /// headers a stream asks with. `None` when the list said none of these
    /// (every list but an IPTV one), so a plain list pays nothing for them.
    pub attributes: Option<Box<Attributes>>,
}

/// What an extended M3U's `#EXTINF` line says about an entry beyond its
/// length and title.
///
/// # Why they are kept now
///
/// An IPTV list files every channel under a `group-title`, and names its logo,
/// its programme-guide id, its country and its language as attributes of the
/// `#EXTINF` line. They were read past and dropped while a list was only
/// something to queue; the player's catalogue sorts channels BY them (a group
/// is a category, the logo a poster, the country and language are filters),
/// so they are kept exactly as written, and what they mean is the reader's.
///
/// Two more ride along because a stream fails without them: the
/// `#EXTVLCOPT:http-user-agent=` and `http-referrer=` lines some lists write
/// above an entry, which the hosts that check them demand.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attributes {
    /// `group-title`, as written: several groups are separated by `;`
    /// (iptv-org writes `Animation;Kids`), and splitting them is the
    /// reader's. An `#EXTGRP:` line gives it when the `#EXTINF` did not.
    pub group: Option<String>,
    /// `tvg-logo`: the address of the channel's picture.
    pub logo: Option<String>,
    /// `tvg-id`: the programme guide's id of the channel (`BBCNews.uk`).
    pub guide_id: Option<String>,
    /// `tvg-country`, as written (`US`, `us;ca`, a name).
    pub country: Option<String>,
    /// `tvg-language`, as written (`English`, `eng;spa`).
    pub language: Option<String>,
    /// The `User-Agent` an `#EXTVLCOPT:http-user-agent=` line asks for.
    pub user_agent: Option<String>,
    /// The `Referer` an `#EXTVLCOPT:http-referrer=` line asks for.
    pub referrer: Option<String>,
}

impl Attributes {
    /// Whether it says nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// How much of a list is read: its bytes, and its entries.
///
/// The defaults ([`BYTE_CEILING`], [`ENTRY_CEILING`]) are for a list a person
/// opens and a panel draws. The player's catalogue reads an IPTV provider's
/// whole list — tens of thousands of channels, tens of megabytes — and asks
/// with bounds of its own ([`parse_text_within`]); a bound is still a bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// The most bytes of the text read; the rest is not.
    pub bytes: usize,
    /// The most entries kept; the list says it was cut past them.
    pub entries: usize,
}

impl Bounds {
    /// The panel's bounds: [`BYTE_CEILING`] and [`ENTRY_CEILING`].
    pub const PANEL: Self = Self {
        bytes: BYTE_CEILING as usize,
        entries: ENTRY_CEILING,
    };
}

/// A playlist, read.
#[derive(Debug, Clone, PartialEq)]
pub struct List {
    /// Which spelling it was.
    pub kind: Kind,
    /// What it names, in the order it named them.
    pub entries: Vec<Entry>,
    /// There was more than this, and the panel says so on its last row.
    pub cut: bool,
}

impl List {
    /// The entries that are on this machine and do not say how long they run.
    ///
    /// What the host fills in. It is answered HERE — rather than the host
    /// walking the entries itself — because the ceiling and the rule ("local,
    /// and undeclared") belong with the list, and this module stays pure: it
    /// returns which ones to ask about and never asks.
    pub fn untimed(&self) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.seconds.is_none() && matches!(e.at, Where::Local(_)))
            .map(|(n, _)| n)
            .take(TIMED_CEILING)
            .collect()
    }

    /// The entries something could actually be played from.
    ///
    /// One place, because the queue walks these and the rows are drawn from
    /// the same list — and a drawn row number that did not match a queue
    /// position would put the playing mark on the wrong line.
    pub fn playable(&self) -> impl Iterator<Item = usize> + '_ {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e.at, Where::Local(_)))
            .map(|(n, _)| n)
    }
}

/// Why a file that names itself a playlist gave no list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trouble {
    /// It could not be opened or read.
    Unreadable,
    /// It was read, and named nothing.
    Empty,
}

/// Which spelling a file's NAME says it is.
///
/// For the player, which is handed paths and not media types. The extension
/// is the one question it can answer without a MIME database, and a file
/// whose name lies about its spelling gets `Trouble::Empty` from the parser,
/// never a list read with the wrong grammar.
#[must_use]
pub fn kind_by_name(path: &Path) -> Option<Kind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "m3u" | "m3u8" => Some(Kind::M3u),
        "pls" => Some(Kind::Pls),
        "xspf" => Some(Kind::Xspf),
        "asx" | "wax" | "wvx" => Some(Kind::Asx),
        _ => None,
    }
}

/// The whole of it, once the spelling is settled.
///
/// # Errors
///
/// [`Trouble::Unreadable`] when the file cannot be read, and
/// [`Trouble::Empty`] when it names nothing.
pub fn read(path: &Path, kind: Kind) -> Result<List, Trouble> {
    read_titled(path, kind).map(|(list, _)| list)
}

/// [`read`], and the name the list gives ITSELF ([`title_of`]).
///
/// For the player, which makes a playlist node of a list and names the node
/// after it — an XSPF's `<title>`, an M3U's `#PLAYLIST:` — rather than after
/// the file.
///
/// # Errors
///
/// As [`read`].
pub fn read_titled(path: &Path, kind: Kind) -> Result<(List, Option<String>), Trouble> {
    if kind == Kind::Own {
        // OURS ARE JSON with a ceiling of their own, and the reader is `own`'s.
        let read = own::read(path).map_err(|_| Trouble::Unreadable)?;
        return list_of_own(&read).map(|list| (list, None));
    }
    let text = read_text(path)?;
    let folder = path.parent().unwrap_or(Path::new("."));
    let list = parse_text(&text, kind, Base::Folder(folder))?;
    Ok((list, title_of(&text, kind)))
}

/// One of our files, read, as the flat [`List`] the preview draws: a set is
/// one row, a queue what it plays ([`own::Own::listing`]).
///
/// # Errors
///
/// [`Trouble::Empty`] when it plays nothing.
pub fn list_of_own(read: &own::Own) -> Result<List, Trouble> {
    let mut entries = read.listing();
    if entries.is_empty() {
        return Err(Trouble::Empty);
    }
    let cut = entries.len() >= ENTRY_CEILING;
    entries.truncate(ENTRY_CEILING);
    Ok(List {
        kind: Kind::Own,
        entries,
        cut,
    })
}

/// At most [`BYTE_CEILING`] of a list on disk, as text.
fn read_text(path: &Path) -> Result<String, Trouble> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path).map_err(|_| Trouble::Unreadable)?;
    let mut raw = Vec::new();
    file.by_ref()
        .take(BYTE_CEILING)
        .read_to_end(&mut raw)
        .map_err(|_| Trouble::Unreadable)?;
    // LOSSY, because a playlist is a list of NAMES and a name is bytes: a
    // stray Latin-1 accent in one line should not lose the other forty.
    Ok(String::from_utf8_lossy(&raw).into_owned())
}

/// The name a list gives itself, when it gives one.
///
/// An XSPF's `<title>` and an ASX's `<title>` (children of the list itself,
/// never a track's), and the extended M3U's `#PLAYLIST:` line. A PLS has no
/// such field. Trimmed; an empty one is none.
#[must_use]
pub fn title_of(text: &str, kind: Kind) -> Option<String> {
    let text = within_the_ceiling(text);
    let found = match kind {
        Kind::M3u => text.lines().find_map(|line| {
            line.trim_end_matches('\r')
                .trim()
                .strip_prefix("#PLAYLIST:")
                .map(ToOwned::to_owned)
        }),
        Kind::Xspf | Kind::Asx => {
            // Through the depth guard, as every list's entries are.
            let doc = ronoctua_xml::parse(text, ronoctua_xml::roxmltree::ParsingOptions::default())
                .ok()?;
            let root = doc.root_element();
            root.children()
                .find(|c| c.is_element() && c.tag_name().name().eq_ignore_ascii_case("title"))
                .and_then(|c| c.text())
                .map(ToOwned::to_owned)
        }
        Kind::Pls | Kind::Own => None,
    };
    found.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

/// What a relative name in a list is relative TO.
///
/// # Two answers, because a list comes from two places
///
/// A list on this disk names files beside it, and `01.flac` means
/// `<its folder>/01.flac`. A list fetched from a radio's site names addresses
/// beside IT — `stream.mp3` in `https://radio.example/listen.pls` is
/// `https://radio.example/stream.mp3` — and must never name a file on this
/// machine: a `file://` in a list somebody else's server wrote is refused, or a
/// web page could make the player open `~/.ssh/id_ed25519` as audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base<'a> {
    /// The folder of a list read from this disk.
    Folder(&'a Path),
    /// The address a list was fetched from.
    Address(&'a str),
}

impl<'a> From<&'a Path> for Base<'a> {
    fn from(folder: &'a Path) -> Self {
        Self::Folder(folder)
    }
}

/// A list that is already text, read with the grammar `kind` names.
///
/// For the daemon, which fetches a radio's `.pls` itself (bounded, through
/// the one HTTP crate) and hands the text here: the network stays out of this
/// crate, and the four grammars stay in it. [`read`] is this, after the file
/// is read.
///
/// Past [`BYTE_CEILING`] the text is not read — the same bound a file gets.
///
/// # Errors
///
/// [`Trouble::Empty`] when it names nothing.
pub fn parse_text(text: &str, kind: Kind, base: Base<'_>) -> Result<List, Trouble> {
    parse_text_within(text, kind, base, Bounds::PANEL)
}

/// [`parse_text`], within bounds of the caller's.
///
/// For the player's catalogue, which reads a provider's whole list; an M3U
/// stops reading at one entry past the bound, so a list ten times too long
/// costs the bound and not the list.
///
/// # Errors
///
/// [`Trouble::Empty`] when it names nothing.
pub fn parse_text_within(
    text: &str,
    kind: Kind,
    base: Base<'_>,
    bounds: Bounds,
) -> Result<List, Trouble> {
    let text = within(text, bounds.bytes);
    let most = bounds.entries;
    let mut found = match kind {
        Kind::M3u => from_m3u_up_to(text, base, most.saturating_add(1)),
        Kind::Pls => from_pls(text, base),
        Kind::Xspf => from_xspf(text, base),
        Kind::Asx => from_asx(text, base),
        // OURS, from this disk only: a list fetched from somebody else's
        // server never names one of our files here.
        Kind::Own => match base {
            Base::Folder(folder) => own::parse(text.as_bytes(), folder, &|_| true)
                .map(|own| own.listing())
                .unwrap_or_default(),
            Base::Address(_) => Vec::new(),
        },
    };
    let cut = found.len() > most;
    found.truncate(most);
    if found.is_empty() {
        // NOTHING IN IT is a sentence the panel already has, and the true one:
        // a file of comments is a file with nothing inside.
        return Err(Trouble::Empty);
    }
    Ok(List {
        kind,
        entries: found,
        cut,
    })
}

/// The first [`BYTE_CEILING`] bytes, cut where a character ends.
fn within_the_ceiling(text: &str) -> &str {
    within(text, Bounds::PANEL.bytes)
}

/// The first `most` bytes, cut where a character ends.
fn within(text: &str, most: usize) -> &str {
    if text.len() <= most {
        return text;
    }
    let mut end = most;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Is this `.m3u8` a STREAM's own index rather than a list of things?
///
/// # Why the player has to ask
///
/// The two share an extension. An HLS playlist is what a live channel or a
/// segmented video IS — its lines name two-second pieces of one medium, and
/// opening it as a list would queue a thousand fragments. libav's HLS reader
/// is what plays it, as one medium. Every HLS file carries `#EXT-X-` tags
/// (`#EXT-X-TARGETDURATION` in a media playlist, `#EXT-X-STREAM-INF` in a
/// master one), and a list of songs has none, so their presence is the
/// answer.
#[must_use]
pub fn is_hls(text: &str) -> bool {
    within_the_ceiling(text)
        .lines()
        .any(|line| line.trim_start().starts_with("#EXT-X-"))
}

/// Where a name in a playlist points.
///
/// # `..` and absolute paths are ALLOWED here, and refused in glTF
///
/// The difference is who opens what. A glTF's buffer URI is a resource the
/// RENDERER opens behind the person's back, mid-parse, so `..` there is a file
/// read nobody asked for. A playlist entry is SHOWN first and played on
/// request — it is a listing row, and the same file is one `Enter` away in the
/// listing already.
///
/// Refusing them would also break the feature outright: a `.m3u` exported by
/// any music player is absolute, and `../Album/01.flac` is ordinary.
///
/// What they must not do is arrive silently, which is why [`Entry::written`]
/// keeps what the file said and the row shows that.
fn points_at(written: &str, base: Base<'_>) -> Where {
    let written = written.trim();
    if written.is_empty() {
        return Where::Refused;
    }
    // A SCHEME IS A SCHEME. `file://` is a local path spelled long and is
    // decoded; everything else is somewhere this crate does not go.
    if let Some(scheme) = scheme_of(written) {
        if scheme.eq_ignore_ascii_case("file") {
            let Base::Folder(folder) = base else {
                // A LIST FROM SOMEBODY ELSE'S SERVER NAMES NOTHING HERE.
                return Where::Refused;
            };
            let rest = &written[scheme.len() + 3..];
            let decoded = percent_decoded(rest);
            return local(&decoded, folder);
        }
        if written.contains(['\0', '\n', '\r']) {
            return Where::Refused;
        }
        return Where::Remote(written.to_owned());
    }
    match base {
        Base::Folder(folder) => local(&percent_decoded(written), folder),
        Base::Address(address) => beside_the_address(written, address),
    }
}

/// The scheme of `text` when it is written `<scheme>://…`.
pub(crate) fn scheme_of(text: &str) -> Option<&str> {
    let (scheme, _) = text.split_once("://")?;
    (!scheme.is_empty()
        && scheme.chars().next().is_some_and(char::is_alphabetic)
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')))
    .then_some(scheme)
}

/// A relative name in a list fetched from `address`, as an address.
///
/// String arithmetic and not a URL parser: the three spellings a list uses
/// are `//host/x` (the list's own scheme), `/x` (the list's own host) and `x`
/// (the list's own folder), and each is one cut of the address. `..` is left
/// in, for the server to answer, as a browser does.
fn beside_the_address(written: &str, address: &str) -> Where {
    if written.contains(['\0', '\n', '\r']) {
        return Where::Refused;
    }
    let Some(scheme) = scheme_of(address) else {
        return Where::Refused;
    };
    let after_scheme = scheme.len() + 3;
    // THE HOST ends where the path, the query or the fragment starts.
    let host_end = address[after_scheme..]
        .find(['/', '?', '#'])
        .map_or(address.len(), |n| after_scheme + n);
    if written.starts_with("//") {
        return Where::Remote(format!("{scheme}:{written}"));
    }
    if written.starts_with('/') {
        return Where::Remote(format!("{}{written}", &address[..host_end]));
    }
    // THE FOLDER: the path without its query and its last piece.
    let path_end = address[host_end..]
        .find(['?', '#'])
        .map_or(address.len(), |n| host_end + n);
    let folder = match address[host_end..path_end].rfind('/') {
        Some(n) => &address[..=host_end + n],
        None => &address[..host_end],
    };
    if folder.ends_with('/') {
        Where::Remote(format!("{folder}{written}"))
    } else {
        Where::Remote(format!("{folder}/{written}"))
    }
}

/// A name that is not a URL, resolved against the playlist's own folder.
fn local(name: &str, folder: &Path) -> Where {
    // A NAME NO FILESYSTEM ACCEPTS IS NOT A NAME. A newline would also split
    // one row into two on screen, which is a list saying something the file
    // does not.
    if name.contains('\0') || name.contains('\n') || name.contains('\r') {
        return Where::Refused;
    }
    let as_path = Path::new(name);
    Where::Local(if as_path.is_absolute() {
        as_path.to_owned()
    } else {
        folder.join(as_path)
    })
}

/// `%20` becomes a space, and a broken escape stays as it was written.
pub(crate) fn percent_decoded(text: &str) -> String {
    if !text.contains('%') {
        return text.to_owned();
    }
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut n = 0;
    while n < bytes.len() {
        if bytes[n] == b'%'
            && n + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (
                (bytes[n + 1] as char).to_digit(16),
                (bytes[n + 2] as char).to_digit(16),
            )
        {
            #[allow(clippy::cast_possible_truncation)]
            out.push((hi * 16 + lo) as u8);
            n += 3;
            continue;
        }
        out.push(bytes[n]);
        n += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A duration a playlist declares, where `-1` is its way of saying "unknown".
fn declared(text: &str) -> Option<f64> {
    let n: f64 = text.trim().parse().ok()?;
    (n.is_finite() && n > 0.0).then_some(n)
}

/// The plain list: a name a line, with an optional `#EXTINF:` above each —
/// within the panel's bound, as the tests read it.
#[cfg(test)]
fn from_m3u(text: &str, base: Base<'_>) -> Vec<Entry> {
    from_m3u_up_to(text, base, ENTRY_CEILING.saturating_add(1))
}

/// [`from_m3u`], stopping once `most` entries are read.
fn from_m3u_up_to(text: &str, base: Base<'_>, most: usize) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut title = None;
    let mut seconds = None;
    let mut said = Attributes::default();
    for line in text.lines() {
        if out.len() >= most {
            break;
        }
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            let read = extinf(rest);
            seconds = declared(read.length);
            title = read.title.map(ToOwned::to_owned);
            // A NEW `#EXTINF` STARTS A NEW ENTRY'S WORDS: what an earlier one
            // said, if nothing followed it, is not this one's.
            said = read.attributes;
            continue;
        }
        if let Some(group) = line.strip_prefix("#EXTGRP:") {
            // THE `#EXTINF`'S OWN GROUP WINS: `#EXTGRP` is the older spelling,
            // and a list that writes both means the attribute.
            if said.group.is_none() {
                said.group = kept(group);
            }
            continue;
        }
        if let Some(option) = line.strip_prefix("#EXTVLCOPT:") {
            if let Some((name, value)) = option.split_once('=') {
                match name.trim().to_ascii_lowercase().as_str() {
                    "http-user-agent" => said.user_agent = kept(value),
                    // BOTH SPELLINGS: HTTP's own header is `Referer`, and lists
                    // write the option either way.
                    "http-referrer" | "http-referer" => said.referrer = kept(value),
                    _ => {}
                }
            }
            continue;
        }
        // EVERY OTHER `#` LINE IS A DIRECTIVE, and the HLS ones (`#EXT-X-…`)
        // are directives this reader has no use for.
        if line.starts_with('#') {
            continue;
        }
        let attributes = std::mem::take(&mut said);
        out.push(Entry {
            at: points_at(line, base),
            title: title.take(),
            seconds: seconds.take(),
            written: line.to_owned(),
            attributes: (!attributes.is_empty()).then(|| Box::new(attributes)),
        });
    }
    out
}

/// A value an attribute or an option gave: trimmed, and `None` when blank.
fn kept(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// What an `#EXTINF:` line says, after its colon.
struct Extinf<'a> {
    /// The length, as written (`-1` is "unknown").
    length: &'a str,
    /// The title: everything after the first comma that is not inside quotes.
    title: Option<&'a str>,
    /// The attributes kept.
    attributes: Attributes,
}

/// An `#EXTINF:` line, read: `<length> key="value" key=value…,<title>`.
///
/// THE FIRST COMMA OUTSIDE QUOTES ends the attributes, and everything after
/// it is the title, commas and all: *"Michael Jackson - Billie Jean, single
/// version"* is ordinary, and so is `group-title="News, Weather"`. An
/// attribute whose name is not one of the five kept is read past, so its
/// value — quoted, or up to a space — never leaks into the title.
fn extinf(rest: &str) -> Extinf<'_> {
    let bytes = rest.as_bytes();
    let mut at = 0;
    // THE LENGTH: up to a space or the comma.
    while at < bytes.len() && !matches!(bytes[at], b' ' | b'\t' | b',') {
        at += 1;
    }
    let length = &rest[..at];
    let mut attributes = Attributes::default();
    loop {
        while at < bytes.len() && matches!(bytes[at], b' ' | b'\t') {
            at += 1;
        }
        if at >= bytes.len() {
            return Extinf {
                length,
                title: None,
                attributes,
            };
        }
        if bytes[at] == b',' {
            let title = rest[at + 1..].trim();
            return Extinf {
                length,
                title: (!title.is_empty()).then_some(title),
                attributes,
            };
        }
        // A NAME: up to `=`, a space or the comma.
        let name_from = at;
        while at < bytes.len() && !matches!(bytes[at], b'=' | b' ' | b'\t' | b',') {
            at += 1;
        }
        let name = &rest[name_from..at];
        if at >= bytes.len() || bytes[at] != b'=' {
            // A WORD WITH NO VALUE is read past, like any attribute not kept.
            continue;
        }
        at += 1;
        let value = if bytes.get(at) == Some(&b'"') {
            let from = at + 1;
            let to = rest[from..].find('"').map_or(rest.len(), |n| from + n);
            at = (to + 1).min(rest.len());
            &rest[from..to]
        } else {
            let from = at;
            while at < bytes.len() && !matches!(bytes[at], b' ' | b'\t' | b',') {
                at += 1;
            }
            &rest[from..at]
        };
        let slot = match name.to_ascii_lowercase().as_str() {
            "group-title" => &mut attributes.group,
            "tvg-logo" => &mut attributes.logo,
            "tvg-id" => &mut attributes.guide_id,
            "tvg-country" => &mut attributes.country,
            "tvg-language" => &mut attributes.language,
            _ => continue,
        };
        *slot = kept(value);
    }
}

/// The ini list, whose fields are joined by a NUMBER rather than by order.
fn from_pls(text: &str, base: Base<'_>) -> Vec<Entry> {
    // BY THE NUMBER IN THE KEY, because `File2=` may be written before
    // `File1=` and a `Title3=` may have no `File3=` at all.
    let mut found: Vec<(u32, String, Option<String>, Option<f64>)> = Vec::new();
    fn slot_for(found: &mut Vec<(u32, String, Option<String>, Option<f64>)>, n: u32) -> usize {
        if let Some(seen) = found.iter().position(|(m, ..)| *m == n) {
            return seen;
        }
        found.push((n, String::new(), None, None));
        found.len() - 1
    }
    for line in text.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let digits: String = key.chars().filter(char::is_ascii_digit).collect();
        let Ok(n) = digits.parse::<u32>() else {
            continue;
        };
        let word: String = key.chars().filter(|c| !c.is_ascii_digit()).collect();
        // CASE-INSENSITIVE, because a dozen programs write this format and
        // they do not agree on it.
        if word.eq_ignore_ascii_case("File") {
            let slot = slot_for(&mut found, n);
            found[slot].1 = value.to_owned();
        } else if word.eq_ignore_ascii_case("Title") {
            let slot = slot_for(&mut found, n);
            found[slot].2 = (!value.is_empty()).then(|| value.to_owned());
        } else if word.eq_ignore_ascii_case("Length") {
            let slot = slot_for(&mut found, n);
            found[slot].3 = declared(value);
        }
    }
    found.sort_by_key(|(n, ..)| *n);
    found
        .into_iter()
        .filter(|(_, file, ..)| !file.is_empty())
        .map(|(_, file, title, seconds)| Entry {
            at: points_at(&file, base),
            title,
            seconds,
            written: file,
            attributes: None,
        })
        .collect()
}

/// The XML list, whose durations are in MILLISECONDS.
fn from_xspf(text: &str, base: Base<'_>) -> Vec<Entry> {
    // Through the depth guard: `roxmltree` recurses once per level, and a
    // list a radio's server sends is a text anyone could have written.
    let Ok(doc) = ronoctua_xml::parse(text, ronoctua_xml::roxmltree::ParsingOptions::default())
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for track in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "track")
    {
        let child = |name: &str| {
            track
                .children()
                .find(|c| c.is_element() && c.tag_name().name() == name)
                .and_then(|c| c.text())
                .map(str::trim)
                .filter(|t| !t.is_empty())
        };
        let Some(location) = child("location") else {
            continue;
        };
        out.push(Entry {
            at: points_at(location, base),
            title: child("title").map(ToOwned::to_owned),
            // MILLISECONDS, which is this format's one unit trap: everything
            // else in this file speaks seconds.
            seconds: child("duration")
                .and_then(|d| d.trim().parse::<f64>().ok())
                .filter(|d| d.is_finite() && *d > 0.0)
                .map(|d| d / 1000.0),
            written: location.to_owned(),
            attributes: None,
        });
    }
    out
}

/// Microsoft's XML list.
///
/// # Two things it does not share with the XSPF above
///
/// Its tag and attribute names are CASE-INSENSITIVE — `<ASX>`, `<Entry>` and
/// `<REF HREF=…>` are all written in the wild, and a reader that matched
/// `"entry"` exactly would open half the files it claims. And a duration is a
/// CLOCK (`00:03:45.6`), not a number of seconds or milliseconds.
///
/// What it deliberately does not read is `<entryref href=…>`, which names
/// another playlist by URL: fetching it is the network, and there is none
/// here. It is skipped the way an unreadable line of an `.m3u` is.
fn from_asx(text: &str, base: Base<'_>) -> Vec<Entry> {
    let Ok(doc) = ronoctua_xml::parse(text, ronoctua_xml::roxmltree::ParsingOptions::default())
    else {
        return Vec::new();
    };
    let named = |n: &ronoctua_xml::roxmltree::Node, name: &str| {
        n.tag_name().name().eq_ignore_ascii_case(name)
    };
    let mut out = Vec::new();
    for entry in doc
        .descendants()
        .filter(|n| n.is_element() && named(n, "entry"))
    {
        let child = |name: &str| entry.children().find(|c| c.is_element() && named(c, name));
        let Some(href) = child("ref").and_then(|r| {
            r.attributes()
                .find(|a| a.name().eq_ignore_ascii_case("href"))
                .map(|a| a.value())
        }) else {
            continue;
        };
        let href = href.trim();
        if href.is_empty() {
            continue;
        }
        out.push(Entry {
            at: points_at(href, base),
            title: child("title")
                .and_then(|t| t.text())
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(ToOwned::to_owned),
            seconds: child("duration")
                .and_then(|d| {
                    d.attributes()
                        .find(|a| a.name().eq_ignore_ascii_case("value"))
                        .map(|a| a.value())
                })
                .and_then(on_the_clock),
            written: href.to_owned(),
            attributes: None,
        });
    }
    out
}

/// `hh:mm:ss.f`, `mm:ss` or `ss` as seconds.
///
/// A separate function because it is the only place in this module that reads
/// a time written as a clock, and because getting it wrong is silent: a
/// duration that does not parse becomes "unknown", which looks exactly like a
/// file whose length nobody asked for.
fn on_the_clock(value: &str) -> Option<f64> {
    let mut seconds = 0.0_f64;
    let mut parts = 0_u32;
    for piece in value.trim().split(':') {
        let n: f64 = piece.parse().ok()?;
        if !n.is_finite() || n < 0.0 {
            return None;
        }
        seconds = seconds * 60.0 + n;
        parts += 1;
        if parts > 3 {
            return None;
        }
    }
    (seconds > 0.0).then_some(seconds)
}

#[cfg(test)]
mod tests {
    use super::{
        Attributes, Base, Bounds, Entry, Kind, Trouble, Where, from_asx, from_m3u, from_pls,
        from_xspf, is_hls, kind_by_name, on_the_clock, parse_text, parse_text_within,
    };
    use std::path::{Path, PathBuf};

    /// The folder every fixture resolves against.
    fn folder() -> &'static Path {
        Path::new("/music/album")
    }

    fn at(entry: &Entry) -> Option<&PathBuf> {
        match &entry.at {
            Where::Local(p) => Some(p),
            _ => None,
        }
    }

    /// FAILS IF: the plain list stops reading like one.
    ///
    /// Three traps live in these six lines: an `#EXTINF` title may contain a
    /// COMMA, `-1` is the format's way of saying "I do not know", and every
    /// other `#` line is a directive — including the HLS ones, which name
    /// nothing.
    #[test]
    fn the_plain_list_reads_its_directives_and_its_commas() {
        let text = "#EXTM3U\r\n\
                    #EXT-X-TARGETDURATION:10\r\n\
                    #EXTINF:132,sample (mp3)\r\n\
                    sample-aud.mp3\r\n\
                    #EXTINF:293,Michael Jackson - Billie Jean, single version\n\
                    Billie Jean - Michael Jackson.flac\n\
                    #EXTINF:-1,length unknown\n\
                    sample-aud.ogg\n\
                    \n\
                    /elsewhere/absolute.wav\n";
        let out = from_m3u(text, folder().into());
        assert_eq!(out.len(), 4, "{out:#?}");

        assert_eq!(out[0].title.as_deref(), Some("sample (mp3)"));
        assert_eq!(out[0].seconds, Some(132.0));
        assert_eq!(
            at(&out[0]),
            Some(&PathBuf::from("/music/album/sample-aud.mp3"))
        );

        // THE FIRST COMMA ONLY — the rest of the title is part of the title.
        assert_eq!(
            out[1].title.as_deref(),
            Some("Michael Jackson - Billie Jean, single version")
        );
        assert_eq!(
            at(&out[1]),
            Some(&PathBuf::from(
                "/music/album/Billie Jean - Michael Jackson.flac"
            ))
        );

        // `-1` IS NOT A DURATION.
        assert_eq!(out[2].seconds, None, "-1 became a length");
        assert_eq!(out[2].title.as_deref(), Some("length unknown"));

        // AN ABSOLUTE NAME IS TAKEN AS WRITTEN, and it has no `#EXTINF` above
        // it, so it carries neither a title nor a length.
        assert_eq!(at(&out[3]), Some(&PathBuf::from("/elsewhere/absolute.wav")));
        assert_eq!(out[3].title, None);
        assert_eq!(out[3].seconds, None);

        // AND THE CARRIAGE RETURNS ARE GONE, or every other path would end in
        // one and no file would ever be found.
        assert!(out.iter().all(|e| !e.written.contains('\r')));
    }

    /// FAILS IF: the ini list is read by ORDER rather than by its numbers.
    ///
    /// Which is the whole of this format: `File2=` may be written above
    /// `File1=`, and a `Title` may arrive before the `File` it belongs to.
    #[test]
    fn the_ini_list_joins_its_fields_by_number() {
        let text = "[playlist]\n\
                    NumberOfEntries=2\n\
                    Title2=second, with a comma\n\
                    File2=/elsewhere/two.wav\n\
                    Length2=29\n\
                    file1=sample-aud.mp3\n\
                    TITLE1=first\n\
                    length1=132\n\
                    Version=2\n";
        let out = from_pls(text, folder().into());
        assert_eq!(out.len(), 2, "{out:#?}");
        // SORTED BY THE NUMBER, not by where the lines sat.
        assert_eq!(out[0].title.as_deref(), Some("first"));
        assert_eq!(out[0].seconds, Some(132.0));
        assert_eq!(
            at(&out[0]),
            Some(&PathBuf::from("/music/album/sample-aud.mp3"))
        );
        assert_eq!(out[1].title.as_deref(), Some("second, with a comma"));
        assert_eq!(at(&out[1]), Some(&PathBuf::from("/elsewhere/two.wav")));

        // A `Title` WITH NO `File` IS NOT AN ENTRY.
        let orphan = from_pls("[playlist]\nTitle7=nothing points here\n", folder().into());
        assert!(orphan.is_empty(), "{orphan:#?}");
    }

    /// FAILS IF: XSPF's milliseconds are read as seconds, or its escapes are
    /// left in the path.
    #[test]
    fn the_xml_list_speaks_milliseconds_and_percent_escapes() {
        let text = r#"<?xml version="1.0" encoding="UTF-8"?>
            <playlist version="1" xmlns="http://xspf.org/ns/0/">
              <trackList>
                <track>
                  <location>Billie%20Jean%20-%20Michael%20Jackson.flac</location>
                  <title>Billie Jean</title>
                  <duration>293800</duration>
                </track>
                <track>
                  <location>file:///elsewhere/two.wav</location>
                </track>
                <track>
                  <location>https://example.invalid/stream.mp3</location>
                  <title>a stream</title>
                </track>
                <track><title>no location at all</title></track>
              </trackList>
            </playlist>"#;
        let out = from_xspf(text, folder().into());
        assert_eq!(out.len(), 3, "a track with no location became an entry");

        // MILLISECONDS. Every other format here says seconds.
        assert_eq!(out[0].seconds, Some(293.8));
        assert_eq!(
            at(&out[0]),
            Some(&PathBuf::from(
                "/music/album/Billie Jean - Michael Jackson.flac"
            ))
        );

        // `file://` IS A LOCAL PATH SPELLED LONG.
        assert_eq!(at(&out[1]), Some(&PathBuf::from("/elsewhere/two.wav")));

        // AND ANY OTHER SCHEME IS SOMEWHERE THIS CRATE DOES NOT GO.
        assert_eq!(
            out[2].at,
            Where::Remote("https://example.invalid/stream.mp3".to_owned())
        );
    }

    /// FAILS IF: a name that is not a name becomes a path, or a remote entry
    /// becomes a local one.
    #[test]
    fn a_name_that_is_not_a_name_is_refused_and_a_url_is_never_opened() {
        let out = from_m3u(
            "ordinary.mp3\n\
             ../other album/one.flac\n\
             /absolute/two.wav\n\
             http://192.0.2.1/three.mp3\n\
             ftp://host/four.mp3\n\
             file:///five.wav\n",
            folder().into(),
        );
        assert_eq!(
            at(&out[0]),
            Some(&PathBuf::from("/music/album/ordinary.mp3"))
        );
        // `..` IS ALLOWED, and shown as it was written — a playlist any music
        // player exports is full of them.
        assert_eq!(
            at(&out[1]),
            Some(&PathBuf::from("/music/album/../other album/one.flac"))
        );
        assert_eq!(out[1].written, "../other album/one.flac");
        assert_eq!(at(&out[2]), Some(&PathBuf::from("/absolute/two.wav")));
        // TWO SCHEMES THIS CRATE DOES NOT GO TO.
        assert!(matches!(out[3].at, Where::Remote(_)));
        assert!(matches!(out[4].at, Where::Remote(_)));
        // AND ONE THAT IS LOCAL ALL ALONG.
        assert_eq!(at(&out[5]), Some(&PathBuf::from("/five.wav")));

        // A NUL IS NOT A NAME.
        let bad = from_m3u("one\u{0}two.mp3\n", folder().into());
        assert_eq!(bad[0].at, Where::Refused);
    }

    /// FAILS IF: an ASX stops being read, or is read as if its tag names were
    /// case-sensitive.
    ///
    /// THE OWNER ASKED ABOUT THIS FILE BY NAME — *"não sei se o arquivo
    /// 'playlist.asx' foi criado com a intenção de ser visto no preview, pois
    /// ocorre que ele não abre"* — and it was refused as a whole format over
    /// one element of it, `<entryref>`, which names another list by URL.
    #[test]
    fn microsoft_s_list_is_read_whatever_case_it_is_written_in() {
        let text = concat!(
            "<ASX version=\"3.0\">\n",
            "  <Entry><REF HREF=\"one.mp3\"/><TITLE>First</TITLE>",
            "<DURATION VALUE=\"00:03:45\"/></Entry>\n",
            "  <entry><ref href=\"http://192.0.2.1/two.mp3\"/></entry>\n",
            // NAMES ANOTHER LIST BY URL, which is the network: skipped, and
            // the entries around it are not lost with it.
            "  <entryref href=\"http://192.0.2.1/more.asx\"/>\n",
            "  <entry><ref/></entry>\n",
            "</ASX>\n",
        );
        let out = from_asx(text, folder().into());
        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!(out[0].at, Where::Local(folder().join("one.mp3")));
        assert_eq!(out[0].title.as_deref(), Some("First"));
        assert_eq!(out[0].seconds, Some(225.0));
        assert_eq!(
            out[1].at,
            Where::Remote("http://192.0.2.1/two.mp3".to_owned())
        );
        assert_eq!(out[1].seconds, None);
    }

    /// FAILS IF: an IPTV list's attributes are dropped, a quoted comma ends
    /// the attributes, or one entry's words land on the next.
    ///
    /// A hand-written fixture in the shape iptv-org and the IPTV providers
    /// publish (`#EXTINF:-1 tvg-id=… tvg-logo=… group-title=…,Name`, with
    /// `#EXTVLCOPT` headers and an `#EXTGRP` line).
    #[test]
    fn an_iptv_list_keeps_its_group_logo_guide_country_and_language() {
        let text = "#EXTM3U x-tvg-url=\"https://guide.example/epg.xml\"\n\
            #EXTINF:-1 tvg-id=\"BBCNews.uk\" tvg-country=\"UK\" tvg-language=\"English\" \
            tvg-logo=\"https://logo.example/bbc.png\" group-title=\"News, Weather;Documentary\",BBC News, HD\n\
            #EXTVLCOPT:http-user-agent=Mozilla/5.0 (X11; Linux x86_64)\n\
            #EXTVLCOPT:http-referrer=https://tv.example/\n\
            https://stream.example/bbc/index.m3u8\n\
            #EXTINF:0 catchup=\"default\" tvg-ID=unquoted radio=true,Plain\n\
            #EXTGRP:Radio\n\
            http://radio.example/live.mp3\n\
            #EXTINF:-1,No attributes at all\n\
            https://stream.example/plain.ts\n";
        let out = from_m3u(text, Base::Address("https://lists.example/all.m3u"));
        assert_eq!(out.len(), 3, "{out:#?}");

        let bbc = out[0]
            .attributes
            .as_deref()
            .expect("the attributes are kept");
        assert_eq!(bbc.guide_id.as_deref(), Some("BBCNews.uk"));
        assert_eq!(
            bbc.country.as_deref(),
            Some("UK"),
            "as written: the reader maps it"
        );
        assert_eq!(bbc.language.as_deref(), Some("English"));
        assert_eq!(bbc.logo.as_deref(), Some("https://logo.example/bbc.png"));
        // A COMMA INSIDE QUOTES does not end the attributes.
        assert_eq!(bbc.group.as_deref(), Some("News, Weather;Documentary"));
        assert_eq!(out[0].title.as_deref(), Some("BBC News, HD"));
        assert_eq!(out[0].seconds, None, "-1 is not a length");
        assert_eq!(
            bbc.user_agent.as_deref(),
            Some("Mozilla/5.0 (X11; Linux x86_64)"),
            "an option's value keeps its spaces"
        );
        assert_eq!(bbc.referrer.as_deref(), Some("https://tv.example/"));

        // AN UNQUOTED VALUE, A NAME IN OTHER CASE, AN ATTRIBUTE NOT KEPT, and
        // the older `#EXTGRP` giving the group.
        let plain = out[1].attributes.as_deref().expect("kept");
        assert_eq!(plain.guide_id.as_deref(), Some("unquoted"));
        assert_eq!(plain.group.as_deref(), Some("Radio"));
        assert_eq!(
            plain.user_agent, None,
            "the first entry's header is its own"
        );
        assert_eq!(out[1].title.as_deref(), Some("Plain"));

        // AND A PLAIN LINE PAYS NOTHING.
        assert_eq!(out[2].attributes, None);
        assert_eq!(out[2].title.as_deref(), Some("No attributes at all"));
        assert!(Attributes::default().is_empty());
    }

    /// FAILS IF: a caller's bounds are not the ones applied, or a list past
    /// them does not say it was cut.
    #[test]
    fn a_list_read_within_bounds_stops_at_them_and_says_so() {
        let long: String = (0..5000)
            .map(|n| {
                format!("#EXTINF:-1 group-title=\"G\",Channel {n}\nhttps://s.example/{n}.ts\n")
            })
            .collect();
        let base = Base::Address("https://lists.example/all.m3u");
        // THE PANEL'S BOUND: two thousand.
        let panel = parse_text(&long, Kind::M3u, base).unwrap();
        assert_eq!(panel.entries.len(), super::ENTRY_CEILING);
        assert!(panel.cut);
        // A CATALOGUE'S: all of them, and not cut.
        let wide = Bounds {
            bytes: 16 << 20,
            entries: 10_000,
        };
        let all = parse_text_within(&long, Kind::M3u, base, wide).unwrap();
        assert_eq!(all.entries.len(), 5000);
        assert!(!all.cut);
        // AND A BYTE BOUND CUTS THE TEXT, not the entry count.
        let few = parse_text_within(
            &long,
            Kind::M3u,
            base,
            Bounds {
                bytes: 200,
                entries: 10_000,
            },
        )
        .unwrap();
        assert!(few.entries.len() < 10, "{}", few.entries.len());
    }

    /// FAILS IF: a clock is read as a count of seconds.
    #[test]
    fn a_duration_written_as_a_clock_is_seconds() {
        assert_eq!(on_the_clock("00:03:45"), Some(225.0));
        assert_eq!(on_the_clock("3:45"), Some(225.0));
        assert_eq!(on_the_clock("12"), Some(12.0));
        assert_eq!(on_the_clock("00:00:01.5"), Some(1.5));
        // NOT A CLOCK, and a wrong number is worse than none.
        assert_eq!(on_the_clock(""), None);
        assert_eq!(on_the_clock("1:2:3:4"), None);
        assert_eq!(on_the_clock("nope"), None);
        assert_eq!(on_the_clock("0"), None);
    }

    /// FAILS IF: the host is asked to probe something it should not, or is not
    /// asked about something it should be.
    ///
    /// A bare list has no lengths and still needs a timeline; a remote entry
    /// must never be opened; and a list of a thousand names must not cost a
    /// thousand child processes for a cursor resting on it.
    #[test]
    fn what_the_list_did_not_declare_is_asked_about_within_a_bound() {
        let bare = super::List {
            kind: Kind::M3u,
            entries: from_m3u(
                "one.mp3\nhttp://192.0.2.1/two.mp3\nthree.flac\n",
                folder().into(),
            ),
            cut: false,
        };
        // THE LOCAL ONES, and never the remote one.
        assert_eq!(bare.untimed(), vec![0, 2]);

        // AND WHAT THE LIST ALREADY SAID IS NOT ASKED AGAIN.
        let told = super::List {
            kind: Kind::M3u,
            entries: from_m3u("#EXTINF:132,a\none.mp3\ntwo.flac\n", folder().into()),
            cut: false,
        };
        assert_eq!(told.untimed(), vec![1], "a declared length was asked about");

        // AND A LONG LIST COSTS A BOUNDED NUMBER OF THEM.
        let many = super::List {
            kind: Kind::M3u,
            entries: from_m3u(&"x.mp3\n".repeat(500), folder().into()),
            cut: false,
        };
        assert_eq!(many.untimed().len(), super::TIMED_CEILING);
    }

    /// FAILS IF: the queue and the drawn rows stop agreeing about which entry
    /// is which.
    #[test]
    fn only_the_local_entries_are_playable_and_they_keep_their_row() {
        let out = from_m3u(
            "one.mp3\nhttp://192.0.2.1/two.mp3\nthree.flac\n",
            folder().into(),
        );
        let list = super::List {
            kind: Kind::M3u,
            entries: out,
            cut: false,
        };
        // THE INDICES ARE THE DRAWN ROWS', so a mark put on `playable[1]` lands
        // on row two and not on row one.
        assert_eq!(list.playable().collect::<Vec<_>>(), vec![0, 2]);
    }

    /// FAILS IF: the player stops recognising a playlist by its name, or
    /// starts taking a sound file for one.
    #[test]
    fn a_name_says_which_spelling() {
        assert_eq!(kind_by_name(Path::new("/a/b.M3U8")), Some(Kind::M3u));
        assert_eq!(kind_by_name(Path::new("b.pls")), Some(Kind::Pls));
        assert_eq!(kind_by_name(Path::new("b.xspf")), Some(Kind::Xspf));
        assert_eq!(kind_by_name(Path::new("b.asx")), Some(Kind::Asx));
        assert_eq!(kind_by_name(Path::new("b.flac")), None);
        assert_eq!(kind_by_name(Path::new("xspf")), None);
    }

    /// FAILS IF: a list fetched from a radio's site stops resolving its
    /// relative names against the ADDRESS, or starts naming this machine's
    /// files.
    ///
    /// The three relative spellings a server writes are each one cut of the
    /// address; `file://` from somebody else's server is refused, or a web
    /// page could make the player open a private file as audio.
    #[test]
    fn a_fetched_list_names_addresses_beside_its_own_and_never_a_file_here() {
        let text = "[playlist]\n\
                    File1=http://ice.example/live.mp3\n\
                    File2=backup.aac\n\
                    File3=/other/path.ogg\n\
                    File4=//cdn.example/x.mp3\n\
                    File5=file:///home/somebody/.ssh/id_ed25519\n";
        let list = parse_text(
            text,
            Kind::Pls,
            Base::Address("https://radio.example/listen/tune.pls?session=9#top"),
        )
        .expect("a list");
        let at: Vec<&Where> = list.entries.iter().map(|e| &e.at).collect();
        assert_eq!(
            at,
            vec![
                &Where::Remote("http://ice.example/live.mp3".to_owned()),
                &Where::Remote("https://radio.example/listen/backup.aac".to_owned()),
                &Where::Remote("https://radio.example/other/path.ogg".to_owned()),
                &Where::Remote("https://cdn.example/x.mp3".to_owned()),
                &Where::Refused,
            ]
        );
        // A HOST WITH NO PATH still has a folder: its root.
        let bare = parse_text(
            "one.mp3\n",
            Kind::M3u,
            Base::Address("http://radio.example"),
        )
        .expect("a list");
        assert_eq!(
            bare.entries[0].at,
            Where::Remote("http://radio.example/one.mp3".to_owned())
        );
    }

    /// FAILS IF: an XML list nested past the depth guard reaches the parser.
    ///
    /// `roxmltree` recurses once per level: a hundred thousand `<a>` fit
    /// under the byte ceiling and, parsed, abort the process (the player
    /// daemon for a radio's list, the shell for a preview) — on this test's
    /// own small stack too.
    #[test]
    fn a_list_nested_past_the_guard_is_nothing_and_not_a_crash() {
        let depth = 100_000;
        for (kind, head, tail) in [
            (
                Kind::Xspf,
                "<playlist><trackList><track><location>one.ogg</location></track>",
                "</trackList></playlist>",
            ),
            (
                Kind::Asx,
                "<asx><entry><ref href=\"one.ogg\"/></entry>",
                "</asx>",
            ),
        ] {
            let mut text = String::from(head);
            text.push_str(&"<a>".repeat(depth));
            text.push_str(&"</a>".repeat(depth));
            text.push_str(tail);
            assert_eq!(
                parse_text(&text, kind, Base::Address("https://radio.example/l")),
                Err(Trouble::Empty),
                "{kind:?}"
            );
            let shallow = format!("{head}{tail}");
            assert_eq!(
                parse_text(&shallow, kind, Base::Address("https://radio.example/l"))
                    .map(|l| l.entries.len()),
                Ok(1),
                "{kind:?} at a sane depth"
            );
        }
    }

    /// FAILS IF: text handed in is read differently from the same text in a
    /// file, or a list of comments stops being "nothing in it".
    #[test]
    fn text_is_read_the_way_the_file_is() {
        let list = parse_text(
            "#EXTINF:12,one\none.mp3\n",
            Kind::M3u,
            Base::Folder(folder()),
        )
        .expect("a list");
        assert_eq!(list.entries[0].at, Where::Local(folder().join("one.mp3")));
        assert_eq!(list.entries[0].seconds, Some(12.0));
        assert_eq!(
            parse_text("#EXTM3U\n# nothing\n", Kind::M3u, Base::Folder(folder())),
            Err(Trouble::Empty)
        );
    }

    /// FAILS IF: a stream's own index is taken for a list of songs, or a
    /// list of songs for a stream.
    ///
    /// An HLS index names two-second pieces of ONE medium; queued as a list
    /// it would be a thousand fragments.
    /// FAILS IF: a list's own name is lost, a track's title is taken for
    /// the list's, or an empty name is kept as a name.
    #[test]
    fn a_list_names_itself_and_a_track_does_not_name_it() {
        use super::title_of;
        let xspf = "<playlist version=\"1\" xmlns=\"http://xspf.org/ns/0/\">\
                    <title> Road trip </title><trackList>\
                    <track><title>One</title><location>a.flac</location></track>\
                    </trackList></playlist>";
        assert_eq!(title_of(xspf, Kind::Xspf).as_deref(), Some("Road trip"));
        let untitled = "<playlist xmlns=\"http://xspf.org/ns/0/\"><trackList>\
                        <track><title>One</title><location>a.flac</location></track>\
                        </trackList></playlist>";
        assert_eq!(title_of(untitled, Kind::Xspf), None);
        let asx = "<ASX version=\"3.0\"><TITLE>Evening</TITLE>\
                   <ENTRY><TITLE>x</TITLE><REF HREF=\"a.mp3\"/></ENTRY></ASX>";
        assert_eq!(title_of(asx, Kind::Asx).as_deref(), Some("Evening"));
        let m3u = "#EXTM3U\r\n#PLAYLIST:Morning\r\n#EXTINF:1,x\r\na.flac\r\n";
        assert_eq!(title_of(m3u, Kind::M3u).as_deref(), Some("Morning"));
        assert_eq!(title_of("#EXTM3U\n#PLAYLIST:  \na.flac\n", Kind::M3u), None);
        assert_eq!(title_of("[playlist]\nFile1=a.mp3\n", Kind::Pls), None);
    }

    /// FAILS IF: one of our files is listed from a list somebody else's
    /// server sent, or not listed from one on this disk.
    #[test]
    fn our_own_files_are_listed_from_this_disk_only() {
        let queue = r#"{"format":"ronoctua-queue","version":1,"entries":[
            {"single":{"location":"a.flac"}},{"stream":{"url":"https://r.example/x"}}]}"#;
        let list = parse_text(queue, Kind::Own, Base::Folder(folder())).expect("listed");
        assert_eq!(list.kind, Kind::Own);
        assert_eq!(list.entries.len(), 2);
        assert_eq!(
            at(&list.entries[0]),
            Some(&PathBuf::from("/music/album/a.flac"))
        );
        assert_eq!(
            parse_text(
                queue,
                Kind::Own,
                Base::Address("https://x.example/q.rqueue")
            ),
            Err(Trouble::Empty)
        );
    }

    #[test]
    fn a_stream_index_is_told_from_a_list_of_songs() {
        assert!(is_hls(
            "#EXTM3U\n#EXT-X-TARGETDURATION:10\n#EXTINF:9.9,\nseg0.ts\n"
        ));
        assert!(is_hls(
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=800000\nlow/index.m3u8\n"
        ));
        assert!(!is_hls("#EXTM3U\n#EXTINF:132,a song\none.mp3\n"));
        assert!(!is_hls(""));
    }
}
