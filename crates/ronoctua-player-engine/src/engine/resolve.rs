//! What a door asks to put, turned into entries of the queue.
//!
//! | Asked | Becomes |
//! |---|---|
//! | a file | a file |
//! | a folder | a playlist named after the folder: its media, its folders |
//! | a local `.m3u`/`.pls`/`.xspf`/`.asx` | a playlist named as the list names itself, else after the file |
//! | a local `.m3u8` with `#EXT-X-` | one medium, which libav's HLS reader plays |
//! | a `.rset` | a set |
//! | a subtitle file | a lane over the film it is named after, else over what plays, else over the film beside it (`subtitles.rs`) |
//! | a `.rqueue` | a playlist of it — or the queue itself, when it starts the queue over |
//! | a radio's `.pls`/`.m3u`/`.asx`/`.xspf` on a server | a stream, its relays the addresses after the first |
//! | a YouTube list | a playlist of its videos, listed flat by yt-dlp |
//! | a film of the Internet Archive, whole (a directory's row) | its files: one entry, or a playlist of them named as the film |
//! | any other address | one medium |
//!
//! # Two speeds
//!
//! What is on this disk is read at once, by whoever asked, BEFORE the
//! engine's lock is taken (`serve_one`): a folder is a walk of the disk, and
//! the lock is every door's. What is on the network is given a placeholder —
//! the address itself, shown and not yet playable — and read on a
//! `player-resolve` thread; the placeholder then becomes what was read,
//! keeping its id ([`Engine::resolved`](super::Engine::resolved)).
//!
//! A list inside a list is left as it is written: when it plays, mpv opens it
//! and the engine adopts what mpv found (`queue.rs`, `adopt_expansion`).
//!
//! # A file is put by its canonical path
//!
//! A link, `..` and a second spelling of one file are one place, and "is it
//! in the queue already?" (`Ask::Put`'s `once`) is answered under the
//! engine's lock by comparing text alone (`queue::Queue::holding`): so a file
//! is put by the path the disk calls it — here, outside the lock — and the
//! place being put is worked out here too ([`places_of`]).
//!
//! # An address is named when its title comes
//!
//! One video of a site yt-dlp reads is playable at once, put by its address,
//! and named on a thread of its own ([`spawn_titles`], `yt-dlp --print
//! title`) — only while nobody named it meanwhile.

use std::{
    io::{Read as _, Write as _},
    net::{TcpStream, ToSocketAddrs as _},
    path::Path,
    time::Duration,
};

use ronoctua_player::{
    decoders,
    find::Directory,
    limit::{Limit, TooMany},
    own::{self, Item, Stream},
    queue::{self, EntryId},
    wire::{Thing, Trouble},
};
use ronoctua_playlist::{Base, Kind, Where};

use super::Shared;

/// What one thing asked to be put became.
#[derive(Debug)]
pub enum Resolved {
    /// An entry, now.
    Now(Item),
    /// An entry now — an address, playable at once — and its title asked on
    /// the side ([`spawn_titles`]), for the leaf nobody named.
    Titling {
        /// The entry.
        item: Item,
        /// Its address, as the leaf keeps it.
        url: String,
    },
    /// A `.rqueue`: a playlist of it, and its entries — which are the whole
    /// queue when it is opened to start the queue over.
    Queue {
        /// It, as one playlist.
        item: Item,
        /// What it holds.
        entries: Vec<Item>,
        /// The file as it was when it was read, for "changed on the disk
        /// since".
        stamp: Option<super::queue::FileStamp>,
        /// Where its files ARE, for those written otherwise ([`aliases_of`]).
        aliases: Vec<(String, String)>,
    },
    /// A subtitle file: not an entry of its own, but a lane over a film
    /// (`Engine::open_subtitle`).
    Subtitle(String),
    /// A placeholder now, and the reading that replaces it.
    Later {
        /// What the queue shows meanwhile: the address.
        placeholder: Item,
        /// What is to be read.
        job: Job,
    },
}

/// A reading from the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    /// A radio's list on a server: one broadcast and its relays.
    List {
        /// Its address.
        url: String,
        /// Its spelling, from its name.
        kind: Kind,
    },
    /// A YouTube list, listed flat.
    YouTube {
        /// Its address.
        url: String,
    },
    /// A film of the Internet Archive, opened into the files of it that
    /// play.
    Archive {
        /// Its identifier.
        identifier: String,
        /// Its page's address — where it came from.
        url: String,
    },
    /// A video of the NASA Image and Video Library, kept WHOLE by the
    /// catalogue (its asset list), opened into the best of its files that
    /// plays (`ronoctua_broadcast::nasa::open`).
    Nasa {
        /// Its NASA id.
        nasa_id: String,
        /// Its title, kept on the stream that plays.
        title: Option<String>,
    },
}

/// The most of a radio's list that is read: a list of relays is a few
/// hundred bytes, and a server that answers more is not answering a list.
const LIST_MOST: u64 = 64 * 1024;

/// The most of an answer's head read over plain HTTP.
const HEAD_MOST: u64 = 16 * 1024;

/// How long a server is given for a list.
const PATIENCE: Duration = Duration::from_secs(10);

/// How this player names itself to a server.
const USER_AGENT: &str = "ronoctua-player";

/// Every thing, resolved as far as this disk allows.
#[must_use]
pub fn resolve_all(what: Vec<Thing>) -> Vec<Result<Resolved, Trouble>> {
    what.into_iter().map(resolve).collect()
}

/// One thing, resolved as far as this disk allows.
///
/// # Errors
///
/// A file that is not there or cannot be read, a list or a folder with
/// nothing to play in it, a folder past the queue's ceiling, one of our files
/// that refuses, a set that is not sound.
pub fn resolve(thing: Thing) -> Result<Resolved, Trouble> {
    match thing {
        Thing::Location(written) => location(&written),
        Thing::Stream { stream } => {
            // THE CONSENT AT THE MOMENT OF PUTTING, for the one stream that
            // asks the directory again: a whole film of the archive.
            let archive = ronoctua_broadcast::archive::whole_item(&stream).is_some()
                && ronoctua_player::prefs::store()
                    .read()
                    .allows_directory(Directory::Archive);
            stream_of_directory(stream, archive)
        }
        // A THING THAT ARRIVES TITLED is its location, called by its title
        // from the start.
        Thing::Named {
            location: at,
            title,
        } => location(&at).map(|r| named(r, title)),
        Thing::Set { set } => {
            set.check()
                .map_err(|r| Trouble::of_own(r, set.name.as_deref().unwrap_or_default()))?;
            Ok(Resolved::Now(Item::Set { set, from: None }))
        }
    }
}

/// What `location` resolved to, called `title` — a single file or address,
/// now or as its placeholder; a folder, a list or one of our files keep
/// their own names.
fn named(resolved: Resolved, title: String) -> Resolved {
    let titled = |item: Item| match item {
        Item::Single {
            location, seconds, ..
        } => Item::Single {
            location,
            title: Some(title.clone()),
            seconds,
        },
        Item::Stream(mut stream) => {
            stream.title = Some(title.clone());
            Item::Stream(stream)
        }
        other => other,
    };
    match resolved {
        // NAMED ALREADY: nothing is asked for a title.
        Resolved::Now(item) | Resolved::Titling { item, .. } => Resolved::Now(titled(item)),
        Resolved::Later { placeholder, job } => Resolved::Later {
            placeholder: titled(placeholder),
            job,
        },
        other => other,
    }
}

/// A stream a door put: as it is — or, for a WHOLE film of the archive (its
/// page, as a directory's row makes it), a placeholder now and its files
/// read on the resolving thread; refused, naming the host, when the archive
/// is not let (`archive_let`).
fn stream_of_directory(stream: Stream, archive_let: bool) -> Result<Resolved, Trouble> {
    // A VIDEO OF THE NASA LIBRARY, kept whole by the catalogue: its asset
    // list read on the resolving thread, on the person's consent for its
    // host there — refused then, naming it, when it is not let.
    if let Some(nasa_id) = ronoctua_broadcast::nasa::whole_item(&stream) {
        return Ok(Resolved::Later {
            placeholder: Item::Single {
                location: stream.url.clone(),
                title: stream.title.clone(),
                seconds: None,
            },
            job: Job::Nasa {
                nasa_id,
                title: stream.title,
            },
        });
    }
    let Some(identifier) = ronoctua_broadcast::archive::whole_item(&stream).map(str::to_owned)
    else {
        return Ok(Resolved::Now(Item::Stream(stream)));
    };
    if !archive_let {
        return Err(Trouble::Off {
            host: Directory::Archive.host().to_owned(),
        });
    }
    Ok(Resolved::Later {
        placeholder: Item::Single {
            location: stream.url.clone(),
            title: stream.title,
            seconds: None,
        },
        job: Job::Archive {
            identifier,
            url: stream.url,
        },
    })
}

/// A path or an address.
fn location(written: &str) -> Result<Resolved, Trouble> {
    let written = written.trim();
    if written.starts_with("file://") {
        return local(&canonical(Path::new(&path_of_uri(written))));
    }
    if written.contains("://") {
        return Ok(remote(written));
    }
    local(&canonical(Path::new(written)))
}

/// A path as the disk calls it — a link followed, `..` resolved — or as it
/// is written when the disk cannot say (it is not there: its refusal names
/// it as the person wrote it).
fn canonical(path: &Path) -> std::path::PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Where each thing being put IS, for "is it in the queue already?": a file
/// by its canonical path, a YouTube video by its id, anything else by its
/// text (`queue::place_of`) — worked out HERE, off the engine's lock. A set
/// is many places and none of them one: it is not asked about.
#[must_use]
pub fn places_of(what: &[Thing]) -> Vec<String> {
    what.iter()
        .filter_map(|thing| match thing {
            Thing::Location(written)
            | Thing::Named {
                location: written, ..
            } => {
                let written = written.trim();
                Some(if written.starts_with("file://") {
                    queue::place_of(&path_of_uri(written))
                } else {
                    queue::place_of(written)
                })
            }
            Thing::Stream { stream } => Some(queue::place_of(&stream.url)),
            Thing::Set { .. } => None,
        })
        .collect()
}

/// Where each file of `entries` IS, when that is not how it is written — a
/// link, `..`, a folder reached through a link: `(written, canonical)`, for
/// "is it in the queue already?".
///
/// # Why a `.rqueue` keeps its spelling
///
/// Its files are not put by their canonical paths, as every other file is:
/// saving the queue again would write another text into the person's file.
/// So the engine keeps where they are BESIDE them, and
/// [`not_queued_in`](super::queue::not_queued_in) reads both — worked out
/// here, off the engine's lock, since each is a question for the disk. A set
/// is not asked about, and an address is its own place. Asks the disk: call
/// it off the lock.
#[must_use]
pub fn aliases_of(entries: &[Item]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut walk = vec![entries];
    while let Some(items) = walk.pop() {
        for item in items {
            match item {
                Item::Single { location, .. } if !location.contains("://") => {
                    let place = queue::place_of(location);
                    if place != *location {
                        out.push((location.clone(), place));
                    }
                }
                Item::Playlist { entries, .. } => walk.push(entries),
                Item::Single { .. } | Item::Set { .. } | Item::Stream(_) => {}
            }
        }
    }
    out
}

/// A `file://` address, back to the path it names — `%20` and its kin
/// decoded, since a listener sends a URI and the engine opens paths; any
/// other address as it is.
#[must_use]
pub fn path_of_uri(uri: &str) -> String {
    let Some(rest) = uri.strip_prefix("file://") else {
        return uri.to_owned();
    };
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(Some(h)), Some(Some(l))) = (
                bytes.get(i + 1).map(|b| hex(*b)),
                bytes.get(i + 2).map(|b| hex(*b)),
            )
        {
            out.push(u8::try_from(h * 16 + l).unwrap_or(b'?'));
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// An address: a list the network is asked for, or one medium.
fn remote(url: &str) -> Resolved {
    let placeholder = Item::Single {
        location: url.to_owned(),
        title: None,
        seconds: None,
    };
    if is_youtube_list(url) {
        return Resolved::Later {
            placeholder,
            job: Job::YouTube {
                url: url.to_owned(),
            },
        };
    }
    match remote_list_kind(url) {
        Some(kind) => Resolved::Later {
            placeholder,
            job: Job::List {
                url: url.to_owned(),
                kind,
            },
        },
        None if names_itself_elsewhere(url) => Resolved::Titling {
            item: placeholder,
            url: url.to_owned(),
        },
        None => Resolved::Now(placeholder),
    }
}

/// Whether an address is a PAGE whose video has a title only yt-dlp can
/// read — a YouTube video, a page of any site it knows — rather than a file
/// on a server, named by its own last piece. Pure.
///
/// An address ending in a media name (`.mp3`, `.mkv`, `.m3u8`, a list's) is
/// a file, and a radio's stream is one too: asked of yt-dlp it would be
/// downloaded to be named. So only a YouTube video by its id, or an address
/// whose last piece names no media, is asked.
fn names_itself_elsewhere(url: &str) -> bool {
    if ronoctua_ytdlp::video_id(url).is_some() {
        return true;
    }
    let Some((scheme, after)) = url.split_once("://") else {
        return false;
    };
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return false;
    }
    let path = after.split(['?', '#']).next().unwrap_or(after);
    let Some((_, path)) = path.split_once('/') else {
        // A SITE'S OWN ADDRESS is no video.
        return false;
    };
    let last = path.rsplit('/').find(|p| !p.is_empty()).unwrap_or_default();
    if last.is_empty() {
        return false;
    }
    let extension = last
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    !(MEDIA.contains(&extension.as_str())
        || matches!(
            extension.as_str(),
            "m3u" | "m3u8" | "pls" | "xspf" | "asx" | "mpd" | "ism"
        ))
}

/// The spelling of a list on a server, by its name. Never `.m3u8`: on a
/// server that is an HLS stream's own index, one medium libav reads.
fn remote_list_kind(url: &str) -> Option<Kind> {
    let path = url.split(['?', '#']).next()?;
    let (_, after) = path.split_once("://")?;
    let (_, name) = after.split_once('/')?;
    let (_, extension) = name.rsplit('/').next()?.rsplit_once('.')?;
    match extension.to_ascii_lowercase().as_str() {
        "pls" => Some(Kind::Pls),
        "m3u" => Some(Kind::M3u),
        "xspf" => Some(Kind::Xspf),
        "asx" | "wax" | "wvx" => Some(Kind::Asx),
        _ => None,
    }
}

/// A YouTube list: its own page, or a watch page with one
/// (`ronoctua_ytdlp::listing_address` writes that one as the list).
fn is_youtube_list(url: &str) -> bool {
    if ronoctua_ytdlp::listing_address(url) != url {
        return true;
    }
    let Some((_, after)) = url.split_once("://") else {
        return false;
    };
    let (host, rest) = after.split_once('/').unwrap_or((after, ""));
    let host = host.to_ascii_lowercase();
    let youtube = [
        "youtube.com",
        "www.youtube.com",
        "m.youtube.com",
        "music.youtube.com",
    ];
    youtube.contains(&host.as_str())
        && rest.starts_with("playlist?")
        && rest.split(['?', '&']).any(|p| p.starts_with("list="))
}

/// A path on this disk.
fn local(path: &Path) -> Result<Resolved, Trouble> {
    let shown = path.to_string_lossy().into_owned();
    let unreadable = || Trouble::Unreadable {
        path: shown.clone(),
    };
    let meta = std::fs::metadata(path).map_err(|_| unreadable())?;
    if meta.is_dir() {
        let mut budget = queue::LEAVES_MOST;
        return walk(path, 1, &mut budget)?
            .map(Resolved::Now)
            .ok_or(Trouble::Empty { path: shown });
    }
    // A SUBTITLE FILE goes over a film, and is never played by itself. The
    // names that mean other things too (`.txt`, `.xml`) are not taken here:
    // a person adds one of those to a film on purpose.
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(ronoctua_player::subtitles::claims)
    {
        return Ok(Resolved::Subtitle(shown));
    }
    if own::kind_by_name(path).is_some() {
        return ours(path);
    }
    if let Some(kind) = ronoctua_playlist::kind_by_name(path) {
        return list(path, kind);
    }
    Ok(Resolved::Now(Item::Single {
        location: shown,
        title: None,
        seconds: None,
    }))
}

/// A file's name without its extension: what a list or a queue is called
/// when it does not say.
fn stem(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map_or_else(|| path.to_string_lossy(), |s| s.to_string_lossy())
        .into_owned()
}

/// One of our files: a set is a set, a queue a playlist of it.
fn ours(path: &Path) -> Result<Resolved, Trouble> {
    let shown = path.to_string_lossy().into_owned();
    // THE FILE AS IT IS READ, before it is read: a write that lands between
    // the two is one "changed on the disk" catches, never one it misses.
    let stamp = super::queue::FileStamp::of(path);
    match own::read(path) {
        Ok(own::Own::Set(set)) => Ok(Resolved::Now(Item::Set {
            set,
            from: Some(shown),
        })),
        Ok(own::Own::Queue(file)) => {
            if file.entries.is_empty() {
                return Err(Trouble::Empty { path: shown });
            }
            let name = file
                .name
                .clone()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| stem(path));
            Ok(Resolved::Queue {
                item: Item::Playlist {
                    name,
                    from: Some(shown),
                    entries: file.entries.clone(),
                },
                aliases: aliases_of(&file.entries),
                entries: file.entries,
                stamp,
            })
        }
        Err(refusal) => Err(Trouble::of_own(refusal, &shown)),
    }
}

/// A list on this disk: a playlist named as it names itself, else after its
/// file — or, for an HLS index, the one medium it is.
fn list(path: &Path, kind: Kind) -> Result<Resolved, Trouble> {
    let shown = path.to_string_lossy().into_owned();
    let hls = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("m3u8"))
        && std::fs::File::open(path).is_ok_and(|f| {
            let mut head = Vec::new();
            f.take(ronoctua_playlist::BYTE_CEILING)
                .read_to_end(&mut head)
                .is_ok()
                && ronoctua_playlist::is_hls(&String::from_utf8_lossy(&head))
        });
    if hls {
        return Ok(Resolved::Now(Item::Single {
            location: shown,
            title: None,
            seconds: None,
        }));
    }
    let (read, title) = ronoctua_playlist::read_titled(path, kind).map_err(|t| match t {
        ronoctua_playlist::Trouble::Unreadable => Trouble::Unreadable {
            path: shown.clone(),
        },
        ronoctua_playlist::Trouble::Empty => Trouble::Empty {
            path: shown.clone(),
        },
    })?;
    let entries: Vec<Item> = read
        .entries
        .into_iter()
        .filter_map(|e| {
            let location = match e.at {
                Where::Local(p) => canonical(&p).to_string_lossy().into_owned(),
                Where::Remote(u) => u,
                Where::Refused => return None,
            };
            Some(Item::Single {
                location,
                title: e.title,
                seconds: e.seconds,
            })
        })
        .collect();
    if entries.is_empty() {
        return Err(Trouble::Empty { path: shown });
    }
    Ok(Resolved::Now(Item::Playlist {
        name: title.unwrap_or_else(|| stem(path)),
        from: Some(shown),
        entries,
    }))
}

/// The names a folder's media go by: mpv 0.41's own `audio-exts` and
/// `video-exts` (what it would open from a folder itself), the music of
/// consoles libgme reads and the trackers' modules. Images, lists, cue
/// sheets, covers and notes are left in the folder.
const MEDIA: &[&str] = &[
    // audio
    "3gp", "aac", "ac3", "aif", "aiff", "amr", "ape", "au", "awb", "dsf", "dts", "dtshd", "eac3",
    "f4a", "flac", "lpcm", "m1a", "m2a", "m4a", "mk3d", "mka", "mp2", "mp3", "mpa", "mpc", "oga",
    "ogg", "ogm", "opus", "pcm", "rm", "tak", "tta", "thd", "truehd", "wav", "weba", "wma", "wv",
    "wvp", "mid", "midi", // video
    "3g2", "3ga", "3gp2", "3gpp", "amv", "asf", "avf", "avi", "bik", "dav", "divx", "f4v", "flv",
    "gifv", "h264", "h265", "hevc", "m2t", "m2ts", "m2v", "m4v", "mjpeg", "mjpg", "mkv", "mov",
    "mp4", "mp4v", "mpeg", "mpg", "mpv", "mts", "mxf", "nsv", "nuv", "ogv", "ogx", "qt", "rmvb",
    "ts", "vob", "webm", "wm", "wmv", "y4m",
    // the music of consoles (`super::GAME_MUSIC`) and trackers
    "vgm", "vgz", "nsf", "nsfe", "spc", "gbs", "hes", "kss", "ay", "sap", "gym", "mod", "xm", "s3m",
    "it", "mptm",
];

/// Whether a file in a folder is something to play.
fn playable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MEDIA.contains(&e.to_ascii_lowercase().as_str()))
        || decoders::for_path(path).is_some()
}

/// A folder as a playlist: its folders first, then its media, each by name;
/// hidden ones and folders reached through a link left out (a link back up
/// would be a walk without an end).
fn walk(dir: &Path, depth: usize, budget: &mut usize) -> Result<Option<Item>, Trouble> {
    let listing = std::fs::read_dir(dir).map_err(|_| Trouble::Unreadable {
        path: dir.to_string_lossy().into_owned(),
    })?;
    let mut folders = Vec::new();
    let mut files = Vec::new();
    for e in listing.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(kind) = e.file_type() else { continue };
        let path = e.path();
        if kind.is_dir() {
            if depth < queue::DEPTH_MOST {
                folders.push((name.to_lowercase(), path));
            }
        } else if (kind.is_file() || (kind.is_symlink() && path.is_file())) && playable(&path) {
            // A LINK IS PUT AS THE FILE IT NAMES: one file, one place.
            let path = if kind.is_symlink() {
                canonical(&path)
            } else {
                path
            };
            files.push((name.to_lowercase(), path));
        }
    }
    folders.sort();
    files.sort();
    let mut entries = Vec::new();
    for (_, folder) in folders {
        if let Some(inner) = walk(&folder, depth + 1, budget)? {
            entries.push(inner);
        }
    }
    for (_, file) in files {
        // PAST THE QUEUE'S CEILING is a refusal with the number, never a
        // folder silently cut.
        *budget = budget.checked_sub(1).ok_or(Trouble::TooMany(TooMany {
            what: Limit::Leaves,
            most: queue::LEAVES_MOST,
        }))?;
        entries.push(Item::Single {
            location: file.to_string_lossy().into_owned(),
            title: None,
            seconds: None,
        });
    }
    if entries.is_empty() {
        return Ok(None);
    }
    Ok(Some(Item::Playlist {
        name: dir
            .file_name()
            .map_or_else(|| dir.to_string_lossy(), |n| n.to_string_lossy())
            .into_owned(),
        from: Some(dir.to_string_lossy().into_owned()),
        entries,
    }))
}

/// Reads what the network is asked for, on a thread of its own, and hands
/// each answer to the engine as it comes.
pub fn spawn(jobs: Vec<(EntryId, Job)>, shared: Shared) {
    if jobs.is_empty() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("player-resolve".into())
        .spawn(move || {
            for (id, job) in jobs {
                let got = run(&job);
                if let Err(t) = &got {
                    tracing::info!(?job, trouble = ?t, "a list on the network was not read");
                }
                let (lock, bell) = &*shared;
                if let Ok(mut e) = lock.lock() {
                    e.resolved(id, got, &shared);
                    e.broadcast();
                }
                bell.notify_all();
            }
        });
}

/// Asks each address's title on a thread of its own, one after the other,
/// and names its leaf when it comes — if the leaf is still that address and
/// nobody named it meanwhile (`queue::name_if_unnamed`). Nothing is said
/// when no title comes: the leaf keeps its address's name, as it had.
pub fn spawn_titles(named: Vec<(EntryId, String)>, shared: Shared) {
    if named.is_empty() || !ronoctua_ytdlp::installed() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("player-title".into())
        .spawn(move || {
            let program = ronoctua_ytdlp::program();
            for (id, url) in named {
                match title_of(&program, &url) {
                    Ok(title) => {
                        let (lock, bell) = &*shared;
                        if let Ok(mut e) = lock.lock() {
                            e.adopt_title(id, &url, &title);
                            if e.dirty {
                                e.broadcast();
                            }
                        }
                        bell.notify_all();
                    }
                    Err(why) => tracing::info!(?why, "no title came for an address"),
                }
            }
        });
}

/// What `url` is called, asked of `program` (yt-dlp, or a test's stand-in)
/// with the PO token's flags. Blocks: call it on a thread.
///
/// # Errors
///
/// Every trouble of `ronoctua_ytdlp::title_by`.
pub(crate) fn title_of(program: &Path, url: &str) -> Result<String, ronoctua_ytdlp::Trouble> {
    ronoctua_ytdlp::title_by(program, url, &ronoctua_ytdlp::pot_flags())
}

/// One reading. Blocks: call it on a thread.
fn run(job: &Job) -> Result<Item, Trouble> {
    match job {
        Job::List { url, kind } => {
            let (at, text) = fetch_list(url)?;
            stream_of(&text, *kind, &at).ok_or_else(|| Trouble::Empty { path: url.clone() })
        }
        Job::YouTube { url } => {
            let flat = ronoctua_ytdlp::flat(url).map_err(|why| Trouble::Listing {
                what: url.clone(),
                why,
            })?;
            playlist_of_flat(url, flat).ok_or_else(|| Trouble::Empty { path: url.clone() })
        }
        Job::Archive { identifier, url } => {
            let opened = ronoctua_broadcast::archive::open(
                &ronoctua_broadcast::internet::Internet,
                identifier,
            )
            .map_err(Trouble::Directory)?;
            item_of_film(&opened, url)
        }
        Job::Nasa { nasa_id, title } => {
            let host = ronoctua_broadcast::nasa::ASSETS_HOST;
            let pass = ronoctua_consent::pass(host).map_err(|_| Trouble::Off {
                host: host.to_owned(),
            })?;
            let url = ronoctua_broadcast::nasa::open(
                &ronoctua_broadcast::internet::Internet,
                &pass,
                nasa_id,
            )
            .map_err(|why| Trouble::Source { why })?;
            let mut stream = Stream::typed(&url);
            stream.title.clone_from(title);
            Ok(Item::Stream(stream))
        }
    }
}

/// A film of the archive, opened: its one file, or a playlist of its files
/// named as the film. Pure.
fn item_of_film(opened: &ronoctua_broadcast::archive::Opened, url: &str) -> Result<Item, Trouble> {
    let mut streams = opened.streams();
    match streams.len() {
        0 => Err(Trouble::Empty {
            path: url.to_owned(),
        }),
        1 => Ok(Item::Stream(streams.remove(0))),
        _ => Ok(Item::Playlist {
            name: opened.title.clone(),
            from: Some(url.to_owned()),
            entries: streams.into_iter().map(Item::Stream).collect(),
        }),
    }
}

/// A radio's list, read: one stream, its relays after it, named as the list
/// names itself, else as its first entry does.
fn stream_of(text: &str, kind: Kind, at: &str) -> Option<Item> {
    let list = ronoctua_playlist::parse_text(text, kind, Base::Address(at)).ok()?;
    let mut stream = Stream::of_list(&list, true)?;
    stream.title = ronoctua_playlist::title_of(text, kind).or(stream.title);
    Some(Item::Stream(stream))
}

/// A YouTube list, listed: a playlist of watch addresses (never the ones
/// yt-dlp resolves, which expire), named as the list is, else after its
/// address.
fn playlist_of_flat(url: &str, flat: ronoctua_ytdlp::Flat) -> Option<Item> {
    if flat.entries.is_empty() {
        return None;
    }
    let name = Some(flat.title.trim().to_owned())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| queue::file_name(url).to_owned());
    Some(Item::Playlist {
        name,
        from: Some(url.to_owned()),
        entries: flat
            .entries
            .into_iter()
            .map(|e| Item::Single {
                location: e.url,
                title: Some(e.title.trim().to_owned()).filter(|t| !t.is_empty()),
                seconds: e.duration_seconds,
            })
            .collect(),
    })
}

/// A list on a server, at most [`LIST_MOST`]: where it ended up after its
/// redirects, and its text.
///
/// # Whose consent
///
/// The host of `url` — the address the person put — is asked under the
/// pass [`pass_put`] gives, before anything goes out, plain `http` or not.
/// Where that host sends the question is the host's answer and not a host
/// the person let: plain hops are followed as `ronoctua_http::web` follows
/// its own ([`hop_pass`] never writes the consent file), and a hop to another
/// host over `https` goes under that host's own pass when the person let it,
/// else is refused with its name ([`Trouble::Off`]).
fn fetch_list(url: &str) -> Result<(String, String), Trouble> {
    let unreachable = |why: String| Trouble::Unreachable {
        what: url.to_owned(),
        why,
    };
    let first = pass_put(
        host_of(url),
        answered(host_of(url)),
        |host| ronoctua_consent::let_hosts(&[host], true),
        ronoctua_consent::pass,
    )?;
    let mut at = url.to_owned();
    for _ in 0..=ronoctua_http::web::MOST_HOPS {
        if at.starts_with("https://") {
            let pass = hop_pass(&first, host_of(&at), ronoctua_consent::pass)?;
            let ask = ronoctua_http::web::Ask::get(&at, USER_AGENT)
                .most(LIST_MOST)
                .waiting(PATIENCE);
            let got =
                ronoctua_http::web::request_let(&pass, &ask).map_err(|e| unreachable(e.trouble))?;
            if !got.reply.went_through() {
                return Err(unreachable(format!(
                    "{} answered {}",
                    got.url, got.reply.status
                )));
            }
            return Ok((
                got.url,
                String::from_utf8_lossy(&got.reply.body).into_owned(),
            ));
        }
        // PLAIN HTTP, which most radios still serve their lists on: one hop
        // at a time, bounded, framed by the one framer.
        let reply = plain_get(&at).map_err(unreachable)?;
        if let Some(location) = ronoctua_http::web::redirect_of(&reply) {
            at = joined(&at, location)
                .ok_or_else(|| unreachable(format!("{at} redirected to `{location}`")))?;
            continue;
        }
        if !reply.went_through() {
            return Err(unreachable(format!("{at} answered {}", reply.status)));
        }
        let mut body = reply.body;
        body.truncate(usize::try_from(LIST_MOST).unwrap_or(usize::MAX));
        return Ok((at, String::from_utf8_lossy(&body).into_owned()));
    }
    Err(unreachable(format!(
        "more than {} redirects",
        ronoctua_http::web::MOST_HOPS
    )))
}

/// The host an address names: no scheme, no user, no port, no path. Empty
/// for text that names none — which no consent lets. Pure.
fn host_of(url: &str) -> &str {
    url.split_once("://")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .map(|authority| authority.rsplit('@').next().unwrap_or(authority))
        .map(|host| host.split(':').next().unwrap_or(host))
        .unwrap_or_default()
}

/// What the person's consent file says of `host` now: let (`Some(true)`),
/// refused (`Some(false)`), or never answered (`None`).
fn answered(host: &str) -> Option<bool> {
    let name = ronoctua_consent::normal(host)?;
    ronoctua_consent::store().read().hosts.get(&name).copied()
}

/// The pass for the host of the address the person PUT — `answered` being
/// what the consent file says of it now, `keep` how it is let and `mint` how
/// a pass is had (the person's file; a test's own).
///
/// # Why putting the address is the consent
///
/// A radio's list is read because the person put its address in the queue
/// (a file they opened, a row they pressed, an address they pasted): the
/// distro's rule counts a question the person configured as their own
/// consent, and RECORDS it like any other (`ronoctua-consent`) — ONCE, when
/// the file has no answer for the host. A host the person refused stays
/// refused ([`Trouble::Off`]: the file is not written over a "no"), and a
/// host already let is not written again. Nothing but the host put is ever
/// let here.
fn pass_put(
    host: &str,
    answered: Option<bool>,
    keep: impl FnOnce(&str) -> Result<(), ronoctua_consent::Unkept>,
    mint: impl FnOnce(&str) -> Result<ronoctua_consent::Pass, ronoctua_consent::NotLet>,
) -> Result<ronoctua_consent::Pass, Trouble> {
    let off = |host: &str| Trouble::Off {
        host: host.to_owned(),
    };
    match answered {
        Some(false) => return Err(off(host)),
        Some(true) => {}
        None => keep(host).map_err(|why| Trouble::Unreachable {
            what: host.to_owned(),
            why: format!("the consent file would not take it ({})", why.id()),
        })?,
    }
    mint(host).map_err(|n| off(&n.host))
}

/// The pass a hop to `host` goes under: the first one when it is the same
/// host, else that host's own when the person let it — never one this
/// writes — and the host's refusal otherwise.
fn hop_pass(
    first: &ronoctua_consent::Pass,
    host: &str,
    mint: impl FnOnce(&str) -> Result<ronoctua_consent::Pass, ronoctua_consent::NotLet>,
) -> Result<ronoctua_consent::Pass, Trouble> {
    if first.covers(host) {
        return Ok(first.clone());
    }
    mint(host).map_err(|n| Trouble::Off { host: n.host })
}

/// A redirect's `Location`, as an address: absolute as it is, `/x` on the
/// same host, anything else beside the address. `http` and `https` only.
fn joined(from: &str, location: &str) -> Option<String> {
    let location = location.trim();
    if location.starts_with("https://") || location.starts_with("http://") {
        return Some(location.to_owned());
    }
    let (scheme, after) = from.split_once("://")?;
    let host_end = after.find(['/', '?', '#']).unwrap_or(after.len());
    let origin = format!("{scheme}://{}", &after[..host_end]);
    if location.starts_with('/') {
        return Some(format!("{origin}{location}"));
    }
    let path = after[host_end..].split(['?', '#']).next().unwrap_or("");
    let folder = path.rsplit_once('/').map_or("", |(f, _)| f);
    Some(format!("{origin}{folder}/{location}"))
}

/// One plain `GET`, with its answer bounded.
fn plain_get(url: &str) -> Result<ronoctua_http::Reply, String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("{url}: not an http address"))?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() || authority.contains('@') {
        return Err(format!("{url}: no host, or credentials in it"));
    }
    let mut path = rest[end..].split('#').next().unwrap_or("").to_owned();
    if !path.starts_with('/') {
        path.insert(0, '/');
    }
    let with_port = if authority
        .rsplit_once(':')
        .is_some_and(|(_, p)| p.parse::<u16>().is_ok())
        && !authority.ends_with(']')
    {
        authority.to_owned()
    } else {
        format!("{authority}:80")
    };
    let address = with_port
        .to_socket_addrs()
        .map_err(|e| format!("{authority}: {e}"))?
        .next()
        .ok_or_else(|| format!("{authority}: no address"))?;
    let mut stream =
        TcpStream::connect_timeout(&address, PATIENCE).map_err(|e| format!("{authority}: {e}"))?;
    let _ = stream.set_read_timeout(Some(PATIENCE));
    let _ = stream.set_write_timeout(Some(PATIENCE));
    let headers = [("User-Agent", USER_AGENT), ("Accept", "*/*")];
    let call = ronoctua_http::Call::new("GET", &path, authority)
        .with_headers(&headers)
        .waiting(PATIENCE);
    stream
        .write_all(ronoctua_http::request_of(&call).as_bytes())
        .map_err(|e| format!("{authority}: {e}"))?;
    let mut raw = Vec::new();
    // BOUNDED: a server that streams its "list" for ever is a radio, not a
    // list, and is cut here.
    let _ = (&mut stream)
        .take(LIST_MOST + HEAD_MOST)
        .read_to_end(&mut raw);
    ronoctua_http::reply_of(&raw).map_err(|e| format!("{authority}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-resolve-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        dir
    }

    fn now(r: Result<Resolved, Trouble>) -> Item {
        match r {
            Ok(Resolved::Now(item)) => item,
            other => panic!("not resolved now: {other:?}"),
        }
    }

    /// FAILS IF: a thing that arrives titled loses its title — a file is
    /// called by its name again, an address waits to be asked for one — or
    /// a folder is renamed by it.
    #[test]
    fn a_named_thing_keeps_its_title() {
        let dir = folder("named");
        let file = dir.join("a.flac");
        std::fs::write(&file, b"x").expect("a file");
        let Item::Single { title, .. } = now(resolve(Thing::Named {
            location: file.to_string_lossy().into_owned(),
            title: "A song".into(),
        })) else {
            panic!("not a single");
        };
        assert_eq!(title.as_deref(), Some("A song"));
        let Item::Single { title, .. } = now(resolve(Thing::Named {
            location: "https://example.org/a.ogg".into(),
            title: "Far".into(),
        })) else {
            panic!("not a single");
        };
        assert_eq!(title.as_deref(), Some("Far"));
        std::fs::write(dir.join("b.flac"), b"y").expect("another");
        match now(resolve(Thing::Named {
            location: dir.to_string_lossy().into_owned(),
            title: "Not the folder's".into(),
        })) {
            Item::Playlist { name, .. } => assert_ne!(name, "Not the folder's"),
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a list on this disk is flattened again, loses the name it
    /// gives itself, or an HLS index on this disk is opened as a list of its
    /// own pieces.
    #[test]
    fn a_list_on_this_disk_is_a_named_playlist_and_an_hls_index_one_medium() {
        let dir = folder("lists");
        std::fs::write(
            dir.join("l.xspf"),
            "<playlist version=\"1\" xmlns=\"http://xspf.org/ns/0/\"><title>list</title><trackList>\
             <track><location>one.flac</location></track>\
             <track><location>http://192.0.2.1/two.ogg</location></track>\
             </trackList></playlist>",
        )
        .expect("written");
        let Item::Playlist {
            name,
            entries,
            from,
        } = now(resolve(Thing::Location(
            dir.join("l.xspf").to_string_lossy().into_owned(),
        )))
        else {
            panic!("not a playlist");
        };
        assert_eq!(name, "list");
        assert_eq!(
            from.as_deref(),
            Some(dir.join("l.xspf").to_string_lossy().as_ref())
        );
        let locations: Vec<String> = entries
            .iter()
            .map(|e| match e {
                Item::Single { location, .. } => location.clone(),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            locations,
            vec![
                dir.join("one.flac").to_string_lossy().into_owned(),
                "http://192.0.2.1/two.ogg".to_owned()
            ]
        );
        // NAMED AFTER ITS FILE when it does not name itself.
        std::fs::write(dir.join("Road trip.m3u"), "a.flac\nb.flac\n").expect("written");
        let Item::Playlist { name, .. } = now(resolve(Thing::Location(
            dir.join("Road trip.m3u").to_string_lossy().into_owned(),
        ))) else {
            panic!("not a playlist");
        };
        assert_eq!(name, "Road trip");
        std::fs::write(
            dir.join("live.m3u8"),
            "#EXTM3U\n#EXT-X-TARGETDURATION:2\n#EXTINF:2,\nseg1.ts\n",
        )
        .expect("written");
        assert!(matches!(
            now(resolve(Thing::Location(
                dir.join("live.m3u8").to_string_lossy().into_owned()
            ))),
            Item::Single { .. }
        ));
        // A LIST THAT NAMES NOTHING, and a path that is not there, are said.
        std::fs::write(dir.join("empty.pls"), "[playlist]\n").expect("written");
        assert!(matches!(
            resolve(Thing::Location(
                dir.join("empty.pls").to_string_lossy().into_owned()
            )),
            Err(Trouble::Empty { .. })
        ));
        assert!(matches!(
            resolve(Thing::Location(
                dir.join("gone.flac").to_string_lossy().into_owned()
            )),
            Err(Trouble::Unreadable { .. })
        ));
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    /// FAILS IF: a subtitle file is put in the queue to be played by itself
    /// instead of going over a film, or a name that means other things too
    /// (`.txt`) is taken for one.
    #[test]
    fn a_subtitle_file_is_not_an_entry_of_its_own() {
        let dir = folder("subtitles");
        for name in ["film.pt.srt", "film.sup", "notes.txt"] {
            std::fs::write(dir.join(name), "x").expect("written");
        }
        for name in ["film.pt.srt", "film.sup"] {
            let at = dir.join(name).to_string_lossy().into_owned();
            match resolve(Thing::Location(at.clone())) {
                Ok(Resolved::Subtitle(path)) => assert_eq!(path, at),
                other => panic!("{name}: {other:?}"),
            }
        }
        assert!(matches!(
            now(resolve(Thing::Location(
                dir.join("notes.txt").to_string_lossy().into_owned()
            ))),
            Item::Single { .. }
        ));
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    /// FAILS IF: a folder stops being a playlist named after it, takes in
    /// what is not media (a cover, a cue sheet, a hidden file), or is cut
    /// silently past the ceiling.
    #[test]
    fn a_folder_is_a_playlist_of_its_media_and_its_folders() {
        let dir = folder("walk");
        let album = dir.join("Album");
        std::fs::create_dir_all(album.join("CD 2")).expect("folders");
        for name in [
            "02 b.flac",
            "01 a.FLAC",
            "cover.jpg",
            "album.cue",
            ".hidden.mp3",
        ] {
            std::fs::write(album.join(name), b"").expect("written");
        }
        std::fs::write(album.join("CD 2").join("01 c.opus"), b"").expect("written");
        std::fs::create_dir_all(album.join("empty")).expect("folder");
        let Item::Playlist { name, entries, .. } = now(resolve(Thing::Location(
            album.to_string_lossy().into_owned(),
        ))) else {
            panic!("not a playlist");
        };
        assert_eq!(name, "Album");
        assert_eq!(entries.len(), 3, "{entries:#?}");
        assert!(
            matches!(&entries[0], Item::Playlist { name, entries, .. } if name == "CD 2" && entries.len() == 1)
        );
        assert!(
            matches!(&entries[1], Item::Single { location, .. } if location.ends_with("01 a.FLAC"))
        );
        assert!(
            matches!(&entries[2], Item::Single { location, .. } if location.ends_with("02 b.flac"))
        );
        // NOTHING TO PLAY in a folder is said.
        assert!(matches!(
            resolve(Thing::Location(
                album.join("empty").to_string_lossy().into_owned()
            )),
            Err(Trouble::Empty { .. })
        ));
        // PAST THE CEILING is refused with the number.
        let mut budget = 1;
        assert!(matches!(
            walk(&album, 1, &mut budget),
            Err(Trouble::TooMany(TooMany {
                what: Limit::Leaves,
                ..
            }))
        ));
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    /// FAILS IF: our own files stop resolving to what they are, or a queue
    /// opened to start over is not the queue itself.
    #[test]
    fn a_set_is_a_set_and_a_queue_is_a_playlist_or_the_queue() {
        let dir = folder("own");
        let queue = dir.join("Sunday.rqueue");
        std::fs::write(
            &queue,
            r#"{"format":"ronoctua-queue","version":1,"entries":[{"single":{"location":"a.flac"}},{"single":{"location":"b.flac"}}]}"#,
        )
        .expect("written");
        match resolve(Thing::Location(queue.to_string_lossy().into_owned())) {
            Ok(Resolved::Queue {
                item: Item::Playlist { name, .. },
                entries,
                stamp,
                ..
            }) => {
                assert!(stamp.is_some(), "the file as it was read");
                assert_eq!(name, "Sunday");
                assert_eq!(entries.len(), 2);
            }
            other => panic!("{other:?}"),
        }
        let set = dir.join("film.rset");
        std::fs::write(
            &set,
            r#"{"format":"ronoctua-set","version":1,"lanes":[{"id":1,"kind":"video","source":"film.mkv"}]}"#,
        )
        .expect("written");
        assert!(matches!(
            now(resolve(Thing::Location(set.to_string_lossy().into_owned()))),
            Item::Set { from: Some(_), .. }
        ));
        std::fs::write(
            dir.join("newer.rset"),
            r#"{"format":"ronoctua-set","version":9}"#,
        )
        .expect("written");
        assert!(matches!(
            resolve(Thing::Location(
                dir.join("newer.rset").to_string_lossy().into_owned()
            )),
            Err(Trouble::Newer { found: 9, .. })
        ));
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    /// FAILS IF: an address that names a list is played by mpv as one medium
    /// (which opens it into its own playlist), or a stream's index is taken
    /// for a list.
    #[test]
    fn an_address_that_names_a_list_is_read_by_us() {
        let job = |url: &str| match remote(url) {
            Resolved::Later { job, placeholder } => {
                assert!(matches!(placeholder, Item::Single { .. }));
                Some(job)
            }
            _ => None,
        };
        assert_eq!(
            job("http://radio.example/listen.pls?x=1"),
            Some(Job::List {
                url: "http://radio.example/listen.pls?x=1".into(),
                kind: Kind::Pls
            })
        );
        assert!(matches!(
            job("https://r.example/a/b.M3U"),
            Some(Job::List {
                kind: Kind::M3u,
                ..
            })
        ));
        assert_eq!(job("https://tv.example/live/index.m3u8"), None);
        assert_eq!(job("https://r.example/stream.mp3"), None);
        assert!(matches!(
            job("https://www.youtube.com/watch?v=abcdefghijk&list=PL0123456789"),
            Some(Job::YouTube { .. })
        ));
        assert!(matches!(
            job("https://www.youtube.com/playlist?list=PL0123456789"),
            Some(Job::YouTube { .. })
        ));
        assert_eq!(job("https://www.youtube.com/watch?v=abcdefghijk"), None);
    }

    /// FAILS IF: a radio's list stops being one stream with its relays, or
    /// loses the station's name.
    #[test]
    fn a_radios_list_is_one_stream_and_its_relays() {
        let pls = "[playlist]\nFile1=http://a.example:8000/live\nTitle1=Radio X\nFile2=/relay\nNumberOfEntries=2\n";
        let Some(Item::Stream(s)) = stream_of(pls, Kind::Pls, "http://radio.example/x.pls") else {
            panic!("not a stream");
        };
        assert_eq!(s.url, "http://a.example:8000/live");
        assert_eq!(s.alternates, vec!["http://radio.example/relay".to_owned()]);
        assert_eq!(s.title.as_deref(), Some("Radio X"));
        assert!(s.live);
        assert!(stream_of("[playlist]\n", Kind::Pls, "http://r.example/x.pls").is_none());
    }

    /// FAILS IF: a YouTube list loses its name or an entry with an empty
    /// title gets an empty one instead of none.
    #[test]
    fn a_youtube_list_is_a_named_playlist_of_watch_addresses() {
        let flat = ronoctua_ytdlp::Flat {
            title: " Mix ".into(),
            entries: vec![
                ronoctua_ytdlp::FlatEntry {
                    url: "https://www.youtube.com/watch?v=a".into(),
                    title: String::new(),
                    duration_seconds: Some(3.0),
                    thumbnail: None,
                    channel: None,
                },
                ronoctua_ytdlp::FlatEntry {
                    url: "https://www.youtube.com/watch?v=b".into(),
                    title: "B".into(),
                    duration_seconds: None,
                    thumbnail: None,
                    channel: None,
                },
            ],
        };
        let Some(Item::Playlist { name, entries, .. }) =
            playlist_of_flat("https://www.youtube.com/playlist?list=PLx", flat)
        else {
            panic!("not a playlist");
        };
        assert_eq!(name, "Mix");
        assert!(matches!(
            &entries[0],
            Item::Single {
                title: None,
                seconds: Some(_),
                ..
            }
        ));
        assert!(matches!(&entries[1], Item::Single { title: Some(t), .. } if t == "B"));
    }

    /// FAILS IF: an address that is a video's PAGE is not named by its own
    /// title, or a file on a server — a song, a radio, a list, a stream's
    /// index — is sent to yt-dlp to be named.
    #[test]
    fn only_a_page_is_asked_for_its_title() {
        for page in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ?t=3",
            "https://vimeo.com/76979871",
            "https://video.blender.org/w/1a2b3c",
        ] {
            assert!(
                matches!(remote(page), Resolved::Titling { .. }),
                "{page} is a page"
            );
        }
        for file in [
            "https://r.example/stream.mp3",
            "http://radio.example:8000/live.aac",
            "https://tv.example/live/index.m3u8",
            "https://example.org/",
            "https://example.org",
            "ftp://example.org/v/1",
        ] {
            assert!(
                matches!(remote(file), Resolved::Now(_)),
                "{file} is not a page"
            );
        }
        // A THING PUT TITLED asks nothing.
        match resolve(Thing::Named {
            location: "https://vimeo.com/76979871".into(),
            title: "Named".into(),
        }) {
            Ok(Resolved::Now(Item::Single { title, .. })) => {
                assert_eq!(title.as_deref(), Some("Named"));
            }
            other => panic!("{other:?}"),
        }
    }

    /// FAILS IF: the title job reads its answer anywhere but what the
    /// program printed, or a leaf is named by it while somebody else named
    /// it first — run against a stand-in for yt-dlp that prints a title.
    #[test]
    fn the_title_job_names_the_leaf_that_is_still_its_address() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = folder("title");
        let program = dir.join("yt-dlp");
        std::fs::write(
            &program,
            "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\n\
             case \"$last\" in *dQw4w9WgXcQ*) echo 'Never Gonna Give You Up';; *) exit 1;; esac\n",
        )
        .expect("a stand-in");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .expect("runnable");
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        let title = title_of(&program, url).expect("a title");
        assert_eq!(title, "Never Gonna Give You Up");
        assert_eq!(
            title_of(&program, "https://example.org/v/1"),
            Err(ronoctua_ytdlp::Trouble::Failed)
        );
        let mut q = ronoctua_player::queue::Queue::from_file(own::QueueFile {
            name: None,
            entries: vec![Item::Single {
                location: url.into(),
                title: None,
                seconds: None,
            }],
        })
        .expect("a queue");
        let leaf = q.leaves()[0];
        assert!(super::super::queue::name_if_unnamed(
            &mut q, leaf, url, &title
        ));
        assert_eq!(
            q.find(leaf).map(|e| e.body.name()),
            Some(queue::Name::Titled("Never Gonna Give You Up".into()))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the title job is not the INSTALLED yt-dlp's — the program
    /// `RONOCTUA_YT_DLP` names, asked with the title's flags — or its answer
    /// does not name the leaf under the engine's lock, names a leaf the
    /// person named, or turns a queue that was its file's into "not saved".
    #[test]
    fn the_title_job_is_the_installed_yt_dlp_s() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = folder("title-job");
        let program = dir.join("yt-dlp");
        let asked = dir.join("asked");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{}'\necho 'A song'\n",
                asked.display()
            ),
        )
        .expect("a stand-in");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .expect("runnable");
        super::super::alone::run(
            module_path!(),
            "the_title_job_alone",
            &[
                ("RONOCTUA_YT_DLP", program.as_os_str()),
                ("RONOCTUA_POT_HOME", std::ffi::OsStr::new("")),
                ("RONOCTUA_DENO", std::ffi::OsStr::new("")),
                ("RONOCTUA_POT_PLUGINS", std::ffi::OsStr::new("")),
            ],
        );
        let args = std::fs::read_to_string(&asked).expect("the stand-in was asked");
        let args: Vec<&str> = args.lines().collect();
        assert_eq!(
            args.last().copied(),
            Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        );
        assert!(
            args.windows(2).any(|w| w == ["--print", "title"]),
            "{args:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The half of [`the_title_job_is_the_installed_yt_dlp_s`] that runs
    /// alone: an engine in a home of its own, a queue that was its file's —
    /// one address nobody named, the same address a person named — and the
    /// title job, as a put starts it.
    #[test]
    fn the_title_job_alone() {
        use super::super::queue::{Origin, fingerprint};
        use std::sync::{Arc, Condvar, Mutex};
        if !super::super::alone::is_alone() {
            return;
        }
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        let shared: Shared = Arc::new((Mutex::new(super::super::Engine::new()), Condvar::new()));
        // WHERE NOTHING CAN BE WRITTEN: under a file, in this home of its own.
        let home = std::path::PathBuf::from(std::env::var_os("HOME").expect("a home"));
        std::fs::write(home.join("a-file"), b"").expect("a file");
        let nowhere = home.join("a-file").join("Sunday.rqueue");
        let (lock, bell) = &*shared;
        let (a, b) = {
            let mut e = lock.lock().expect("the engine");
            e.set_shared(&shared);
            e.queue = ronoctua_player::queue::Queue::from_file(own::QueueFile {
                name: None,
                entries: vec![
                    Item::Single {
                        location: url.into(),
                        title: None,
                        seconds: None,
                    },
                    Item::Single {
                        location: url.into(),
                        title: Some("Named by the person".into()),
                        seconds: None,
                    },
                ],
            })
            .expect("a queue");
            let print = fingerprint(&e.queue);
            e.origin = Some(Origin {
                path: nowhere.to_string_lossy().into_owned(),
                name: "Sunday".into(),
                fingerprint: print,
                stamp: None,
            });
            let leaves = e.queue.leaves();
            (leaves[0], leaves[1])
        };
        spawn_titles(vec![(a, url.into()), (b, url.into())], shared.clone());
        let named = |e: &super::super::Engine, id| e.queue.find(id).map(|x| x.body.name());
        let song = Some(queue::Name::Titled("A song".into()));
        let (e, waited) = bell
            .wait_timeout_while(
                lock.lock().expect("the engine"),
                Duration::from_secs(20),
                |e| named(e, a) != song,
            )
            .expect("the engine");
        assert!(!waited.timed_out(), "no title came");
        assert_eq!(named(&e, a), song);
        assert_eq!(
            named(&e, b),
            Some(queue::Name::Titled("Named by the person".into()))
        );
        assert!(
            matches!(e.queue_kept(), queue::QueueKept::Same { .. }),
            "a title learned made the queue \"not saved\""
        );
        // PUT ONCE, again: the asker's question, and no door's trouble.
        let mut e = e;
        let said = Trouble::NoOrigin;
        e.state.troubles = vec![said.clone()];
        let put = ronoctua_player::wire::Ask::Put {
            what: vec![Thing::Location("https://youtu.be/dQw4w9WgXcQ?t=3".into())],
            after: None,
            into: None,
            play: false,
            replace: false,
            once: true,
        };
        assert!(matches!(
            e.ask(put, &shared),
            ronoctua_player::wire::Say::Refused(Trouble::AlreadyQueued { .. })
        ));
        assert_eq!(
            e.state.troubles,
            vec![said],
            "one door's question told to all"
        );
        // ANY OTHER REFUSAL is every door's: saving again where nothing can
        // be written.
        let again = ronoctua_player::wire::Ask::SaveQueueAgain { force: false };
        assert!(matches!(
            e.ask(again, &shared),
            ronoctua_player::wire::Say::Refused(Trouble::Unwritable { .. })
        ));
        assert!(
            matches!(e.state.troubles.as_slice(), [Trouble::Unwritable { .. }]),
            "{:?}",
            e.state.troubles
        );
    }

    /// FAILS IF: "is it in the queue already?" is asked about a file by the
    /// way it was spelled — a link, `..` — instead of the file, or about a
    /// YouTube video by its address instead of its id; or a file is put by
    /// another spelling than the one that place names.
    #[test]
    fn a_file_is_put_and_asked_about_by_the_place_it_is() {
        let dir = folder("places");
        let file = dir.join("a.flac");
        std::fs::write(&file, b"x").expect("a file");
        std::os::unix::fs::symlink(&file, dir.join("link.flac")).expect("a link");
        std::fs::create_dir_all(dir.join("sub")).expect("a folder");
        let spelled = [
            dir.join("link.flac").to_string_lossy().into_owned(),
            dir.join("sub/../a.flac").to_string_lossy().into_owned(),
            format!("file://{}", file.to_string_lossy().replace(' ', "%20")),
        ];
        let canonical = std::fs::canonicalize(&file)
            .expect("there")
            .to_string_lossy()
            .into_owned();
        for written in &spelled {
            assert_eq!(
                places_of(&[Thing::Location(written.clone())]),
                std::slice::from_ref(&canonical),
                "{written}"
            );
            let Item::Single { location, .. } = now(resolve(Thing::Location(written.clone())))
            else {
                panic!("not a single");
            };
            assert_eq!(location, canonical, "{written} was put by another spelling");
        }
        assert_eq!(
            places_of(&[
                Thing::Location("https://youtu.be/dQw4w9WgXcQ?t=5".into()),
                Thing::Set {
                    set: own::Set::default()
                },
            ]),
            ["youtube:dQw4w9WgXcQ".to_owned()]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a redirect's `Location` is joined to the wrong place.
    #[test]
    fn a_redirect_is_joined_where_it_points() {
        let from = "http://r.example/a/list.pls?x=1";
        assert_eq!(
            joined(from, "https://s.example/y.pls").as_deref(),
            Some("https://s.example/y.pls")
        );
        assert_eq!(
            joined(from, "/z.pls").as_deref(),
            Some("http://r.example/z.pls")
        );
        assert_eq!(
            joined(from, "w.pls").as_deref(),
            Some("http://r.example/a/w.pls")
        );
    }

    /// FAILS IF: putting an address writes over the person's "no" for its
    /// host, writes a host already let again, or asks a host the file never
    /// answered for without recording that the person put it.
    #[test]
    fn the_host_put_is_let_once_and_a_refusal_stays() {
        let dir = folder("consent-put");
        let store = ronoctua_consent::store_at(Some(dir.clone()));
        let mint = |h: &str| ronoctua_consent::pass_in(&store, h);
        // NEVER ANSWERED: putting it is the consent, recorded.
        let pass = pass_put(
            "Radio.Example",
            None,
            |h| ronoctua_consent::let_hosts_in(&store, &[h], true),
            mint,
        )
        .expect("let by being put");
        assert!(pass.covers("radio.example"));
        assert!(ronoctua_consent::allows_in(&store, "radio.example"));
        // LET ALREADY: nothing is written.
        let written = |_: &str| -> Result<(), ronoctua_consent::Unkept> {
            panic!("a host already answered for was written again")
        };
        assert!(pass_put("radio.example", Some(true), written, mint).is_ok());
        // REFUSED: it stays refused, and the refusal names it.
        ronoctua_consent::let_hosts_in(&store, &["no.example"], false).expect("kept");
        assert_eq!(
            pass_put("no.example", Some(false), written, mint),
            Err(Trouble::Off {
                host: "no.example".into()
            })
        );
        assert!(!ronoctua_consent::allows_in(&store, "no.example"));
        // A FILE THAT WILL NOT TAKE IT is said, and nothing is asked.
        let unkept = |_: &str| Err(ronoctua_consent::Unkept::Unparseable);
        assert!(matches!(
            pass_put("new.example", None, unkept, mint),
            Err(Trouble::Unreachable { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a redirect to another host is let by the address the person
    /// put — asked without that host's own consent — or a hop to the same
    /// host needs one of its own.
    #[test]
    fn a_hop_to_another_host_needs_that_host_s_own_consent() {
        let dir = folder("consent-hop");
        let store = ronoctua_consent::store_at(Some(dir.clone()));
        ronoctua_consent::let_hosts_in(&store, &["radio.example", "cdn.example"], true)
            .expect("kept");
        let mint = |h: &str| ronoctua_consent::pass_in(&store, h);
        let first = mint("radio.example").expect("let");
        let never = |_: &str| -> Result<ronoctua_consent::Pass, ronoctua_consent::NotLet> {
            panic!("the same host asked for a pass of its own")
        };
        assert_eq!(hop_pass(&first, "RADIO.example", never), Ok(first.clone()));
        assert!(
            hop_pass(&first, "cdn.example", mint).is_ok_and(|p| p.covers("cdn.example")),
            "a host the person let is asked under its own pass"
        );
        assert_eq!(
            hop_pass(&first, "elsewhere.example", mint),
            Err(Trouble::Off {
                host: "elsewhere.example".into()
            })
        );
        assert!(
            !ronoctua_consent::allows_in(&store, "elsewhere.example"),
            "a hop lets nothing"
        );
        assert_eq!(
            host_of("http://u:p@Radio.example:8000/l.pls?x#y"),
            "Radio.example"
        );
        assert_eq!(host_of("no address"), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: an address from a listener reaches the engine still
    /// escaped, and names a file that does not exist.
    #[test]
    fn a_file_address_is_the_path_it_names() {
        assert_eq!(path_of_uri("file:///m/a%20b.flac"), "/m/a b.flac");
        assert_eq!(path_of_uri("https://x.org/a%20b"), "https://x.org/a%20b");
    }

    /// FAILS IF: a whole film of the archive is played as the address of its
    /// page instead of being opened into its files — or is opened while the
    /// archive is not let, or a file of it is opened again.
    #[test]
    fn a_film_of_the_archive_is_opened_into_what_plays_only_when_let() {
        let item = ronoctua_broadcast::archive::Item {
            identifier: "Night".to_owned(),
            title: "Night of the Living Dead".to_owned(),
            year: None,
            downloads: 0,
            license_url: None,
        };
        let whole = item.stream().expect("a stream");
        match stream_of_directory(whole.clone(), true) {
            Ok(Resolved::Later {
                placeholder:
                    Item::Single {
                        location, title, ..
                    },
                job: Job::Archive { identifier, url },
            }) => {
                assert_eq!(location, "https://archive.org/details/Night");
                assert_eq!(title.as_deref(), Some("Night of the Living Dead"));
                assert_eq!(identifier, "Night");
                assert_eq!(url, location);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            stream_of_directory(whole.clone(), false).err(),
            Some(Trouble::Off {
                host: "archive.org".to_owned()
            }),
            "nothing is asked of the archive while it is not let"
        );
        let file = Stream {
            url: "https://archive.org/download/Night/a.mp4".to_owned(),
            ..whole
        };
        assert!(matches!(
            stream_of_directory(file, false),
            Ok(Resolved::Now(Item::Stream(_)))
        ));
        let two = ronoctua_broadcast::archive::parse_metadata(
            "Night",
            br#"{"metadata":{"title":"Night"},"files":[
                {"name":"a.mp4","format":"h.264","source":"original"},
                {"name":"b.mp4","format":"h.264","source":"original"}]}"#,
        )
        .expect("an item");
        match item_of_film(&two, "https://archive.org/details/Night") {
            Ok(Item::Playlist { name, entries, .. }) => {
                assert_eq!(name, "Night");
                assert_eq!(entries.len(), 2);
            }
            other => panic!("{other:?}"),
        }
        let one = ronoctua_broadcast::archive::parse_metadata(
            "Night",
            br#"{"files":[{"name":"a.mp4","format":"h.264","source":"original"}]}"#,
        )
        .expect("an item");
        assert!(matches!(
            item_of_film(&one, "https://archive.org/details/Night"),
            Ok(Item::Stream(_))
        ));
    }
}
