//! The player's own look at the filesystem: which folders and files it lists,
//! and whether a folder has anything to play.
//!
//! # What "a file the player plays" means, in one place
//!
//! The browser's listing asks it, and whether a folder has anything to play
//! before it is sent — [`glyph_if_played`] is that answer: a media type under
//! `audio/` or `video/` (which covers the types `assets/player-types.nix`
//! invents for the decoders' families), the handful of media types that live
//! under `application/`, a playlist, or a name one of the decoders claims.
//!
//! # It is the DAEMON that opens a folder
//!
//! A folder is handed to the player as ONE location, and the player makes it
//! a playlist named after the folder (its `engine/resolve.rs`): the queue is a
//! tree, and a folder flattened into its files here lost its name. What it
//! plays of the folder is the player's walk, which leaves pictures, text and
//! lists out, as this listing does.

use std::path::{Path, PathBuf};

/// The media types outside `audio/` and `video/` that are sound or pictures.
///
/// The same seven `nix/modules/player.nix` puts in the desktop entry beside
/// every `audio/` and `video/` type; a type here and not there would be listed
/// by the browser and not offered by the file manager, and the other way round.
pub const ALSO_MEDIA: [&str; 7] = [
    "application/ogg",
    "application/vnd.apple.mpegurl",
    "application/xspf+xml",
    "application/vnd.rn-realmedia",
    "application/vnd.ms-asf",
    "application/mxf",
    "application/x-matroska",
];

/// How deep a folder is walked when it is played whole.
///
/// A music folder is artist, album, disc: three levels, and a few more for
/// whoever sorts by year. Past this, a link or a mount the walk should never
/// have entered is the likelier explanation.
pub const DEPTH_CEILING: usize = 8;

/// The most files one folder puts in the queue.
///
/// `ronoctua_playlist::ENTRY_CEILING`'s number and its argument: past a couple
/// of thousand nobody is reading the queue, and every one is a row the doors
/// draw.
pub const FILE_CEILING: usize = ronoctua_inspect::playlist::ENTRY_CEILING;

/// The glyph a file the player plays is drawn with, or `None` for one it
/// does not play.
///
/// ONE ANSWER TO TWO QUESTIONS, because both are the same lookup: the type
/// the name resolves to. That lookup is the expensive part of a listing —
/// `mime::by_name` walks every pattern of the database, measured at 1.2 ms a
/// name in a debug build — and asking it again for the glyph on every drawn
/// frame is what froze the owner's `~/main/temp` while a song played: the
/// rows are built several times a frame, and again on every position the
/// player reports. So it is asked once, when the folder is read.
#[must_use]
pub fn glyph_if_played(path: &Path) -> Option<char> {
    played(path, true)
}

/// [`glyph_if_played`]'s answer, with the lists left out when `lists` is
/// `false` — ONE lookup of the type either way.
fn played(path: &Path, lists: bool) -> Option<char> {
    let name = path.file_name().and_then(|n| n.to_str())?;
    if let Some(mime) = ronoctua_files::mime::by_name(name) {
        let mime = ronoctua_files::mime::canonical(mime);
        let list = ronoctua_inspect::playlist::is_a_playlist(mime);
        if list && !lists {
            return None;
        }
        if mime.starts_with("audio/")
            || mime.starts_with("video/")
            || ALSO_MEDIA.contains(&mime)
            || list
        {
            return Some(ronoctua_files::mime::icon_of(mime));
        }
    }
    ronoctua_player::decoders::for_path(path).map(|_| ronoctua_tokens::icon::MUSIC)
}

/// What a page of the files lists, besides the folders — the purpose it was
/// opened for (`player::Purpose`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wanted {
    /// What the player plays, lists included: into the queue.
    Media,
    /// What plays TOGETHER with what plays — a lane: pictures, sounds and
    /// subtitles, which play over a picture and never alone in the queue. A
    /// list is no lane, and is left out.
    Lanes,
    /// The player's own files, a set (`.rset`) or a queue (`.rqueue`), to
    /// open.
    Saved,
    /// A SUBTITLE to lay over the picture: every file a subtitle's name says
    /// it is (`subtitles::claims`), and the ones whose name says nothing more
    /// often than a subtitle (`subtitles::GENERIC_EXTENSIONS` — a `.txt` of
    /// TMPlayer) — the person picking one is what makes it a subtitle.
    Subtitles,
    /// A PICTURE, for a cover: every file whose type is an image.
    Images,
}

/// The glyph of a file a page of `wanted` lists, or `None` for one it leaves
/// out.
#[must_use]
pub fn glyph_if_wanted(path: &Path, wanted: Wanted) -> Option<char> {
    use ronoctua_player::own::{OwnKind, kind_by_name};
    match wanted {
        Wanted::Media => glyph_if_played(path),
        Wanted::Images => {
            let name = path.file_name().and_then(|n| n.to_str())?;
            let mime = ronoctua_files::mime::canonical(ronoctua_files::mime::by_name(name)?);
            mime.starts_with("image/")
                .then(|| ronoctua_files::mime::icon_of(mime))
        }
        Wanted::Saved => kind_by_name(path).map(|kind| match kind {
            OwnKind::Set => ronoctua_tokens::icon::PLAYER_SET,
            OwnKind::Queue => ronoctua_tokens::icon::PLAYER_QUEUE,
        }),
        Wanted::Subtitles => {
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            (ronoctua_player::subtitles::claims(&extension)
                || ronoctua_player::subtitles::GENERIC_EXTENSIONS.contains(&extension.as_str()))
            .then_some(ronoctua_tokens::icon::PLAYER_SUBTITLES)
        }
        Wanted::Lanes => {
            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ronoctua_player::subtitles::claims(extension) {
                return Some(ronoctua_tokens::icon::PLAYER_SUBTITLES);
            }
            if kind_by_name(path).is_some() {
                return None;
            }
            played(path, false)
        }
    }
}

/// One folder, read for the browser.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    /// The folder.
    pub at: PathBuf,
    /// Its folders, sorted.
    pub folders: Vec<PathBuf>,
    /// The files in it the player plays, sorted.
    pub files: Vec<PathBuf>,
    /// The glyph of each of [`Listing::files`], by the same index — found
    /// with them, so drawing a row never asks the type database again.
    pub glyphs: Vec<char>,
    /// It could not be read, and why — a listing that came back empty and one
    /// that was refused must not look alike.
    pub refused: Option<String>,
}

/// What is in a folder: the folders under it and the files the player plays.
///
/// Hidden entries are left out, as the file manager leaves them out by
/// default. A link to a folder is listed as a folder — entering it is what a
/// person asked for — and only the WALK in [`gather`] refuses to follow one.
#[must_use]
pub fn list(at: &Path) -> Listing {
    list_for(at, Wanted::Media)
}

/// What is in a folder for a page of `wanted`: the folders under it and the
/// files that page lists ([`glyph_if_wanted`]).
#[must_use]
pub fn list_for(at: &Path, wanted: Wanted) -> Listing {
    let mut out = Listing {
        at: at.to_path_buf(),
        ..Listing::default()
    };
    let entries = match std::fs::read_dir(at) {
        Ok(e) => e,
        Err(e) => {
            out.refused = Some(e.to_string());
            return out;
        }
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            out.folders.push(path);
        } else if let Some(glyph) = glyph_if_wanted(&path, wanted) {
            files.push((path, glyph));
        }
    }
    out.folders.sort_by_key(|p| sort_key(p));
    files.sort_by_key(|(p, _)| sort_key(p));
    (out.files, out.glyphs) = files.into_iter().unzip();
    out
}

/// Every file a folder plays, walked down, in the order a listing shows them:
/// a folder's own files first, then each folder under it.
///
/// A link to a folder is not followed: a link back up the tree would make the
/// walk endless, and one out of it would queue somebody else's library.
#[must_use]
pub fn gather(at: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(at, 0, &mut out);
    out
}

fn walk(at: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > DEPTH_CEILING || out.len() >= FILE_CEILING {
        return;
    }
    let here = list(at);
    for f in here.files {
        if out.len() >= FILE_CEILING {
            return;
        }
        out.push(f);
    }
    for folder in here.folders {
        let linked = std::fs::symlink_metadata(&folder).is_ok_and(|m| m.file_type().is_symlink());
        if !linked {
            walk(&folder, depth + 1, out);
        }
    }
}

/// Case folded, so `b.flac` does not sort after `Z.flac`.
fn sort_key(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// What kind of lane a file or an address becomes when it is played together
/// with what plays, by its name: subtitles for one the player reads as
/// subtitles, a sound for a sound, and a picture for anything else — a film,
/// or an address that names no kind, which the player reads as one.
#[must_use]
pub fn lane_kind_of(location: &str) -> ronoctua_player::own::LaneKind {
    use ronoctua_player::own::LaneKind;
    let named = location.split(['?', '#']).next().unwrap_or(location);
    let path = Path::new(named);
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ronoctua_player::subtitles::claims(extension) {
        return LaneKind::Subtitle;
    }
    let sound = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(ronoctua_files::mime::by_name)
        .is_some_and(|m| ronoctua_files::mime::canonical(m).starts_with("audio/"));
    if sound {
        LaneKind::Audio
    } else {
        LaneKind::Video
    }
}

/// The places the browser starts from: music, videos and music videos, the
/// three user directories a player has something to do in.
///
/// MUSIC VIDEOS IS OURS, and a machine that is not this distro declares no
/// such directory: there it is the MUSIC directory the machine declares —
/// `XDG_MUSIC_DIR`, which need not be `~/Music` — and only with nothing
/// declared at all is it `~/Music`, the name the specification falls back to.
#[must_use]
pub fn places() -> [(&'static str, PathBuf); 3] {
    use ronoctua_i18n::key;
    let music = ronoctua_xdg::user_dir("XDG_MUSIC_DIR", MUSIC);
    [
        (key::PLAYER_PLACE_MUSIC, music.clone()),
        (
            key::PLAYER_PLACE_VIDEOS,
            ronoctua_xdg::user_dir("XDG_VIDEOS_DIR", VIDEOS),
        ),
        (
            key::PLAYER_PLACE_MUSIC_VIDEOS,
            ronoctua_xdg::user_dir_declared("XDG_MUSIC_VIDEO_DIR").unwrap_or(music),
        ),
    ]
}

/// The music folder's name when a machine declares none.
const MUSIC: &str = "Music";

/// The videos folder's name when a machine declares none.
const VIDEOS: &str = "Videos";

/// The places of a home that DECLARES NOTHING — [`places`]' own answer
/// there, music videos in the music folder — for `home` instead of this
/// machine's: what a photograph browses (`crate::photograph`), the same
/// tree on every machine.
#[must_use]
pub fn places_in(home: &std::path::Path) -> [(&'static str, PathBuf); 3] {
    use ronoctua_i18n::key;
    [
        (key::PLAYER_PLACE_MUSIC, home.join(MUSIC)),
        (key::PLAYER_PLACE_VIDEOS, home.join(VIDEOS)),
        (key::PLAYER_PLACE_MUSIC_VIDEOS, home.join(MUSIC)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-browse-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        for (name, bytes) in [
            ("B.flac", &b""[..]),
            ("a.mp3", b""),
            ("cover.jpg", b""),
            ("notes.txt", b""),
            (".hidden.mp3", b""),
            ("tune.d00", b""),
            ("Album/01.ogg", b""),
            ("Album/02.opus", b""),
            ("Album/Disc 2/01.mkv", b""),
            ("empty/readme.md", b""),
        ] {
            let p = root.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, bytes).unwrap();
        }
        root
    }

    /// FAILS IF: the browser lists what the player cannot play, or leaves out
    /// a sound only one of the decoders reads.
    #[test]
    fn a_folder_lists_its_folders_and_the_files_it_plays() {
        let root = tree();
        let l = list(&root);
        let names = |v: &[PathBuf]| -> Vec<String> {
            v.iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(names(&l.folders), ["Album", "empty"]);
        assert_eq!(names(&l.files), ["a.mp3", "B.flac", "tune.d00"]);
        assert!(l.refused.is_none());
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: the page that picks something to play together leaves out
    /// a subtitle file, or offers a list as a lane; or the page that opens a
    /// set or a queue lists anything but the player's own two.
    #[test]
    fn each_page_of_the_files_lists_what_it_is_for() {
        let root = tree();
        for (name, bytes) in [
            ("film.en.srt", &b""[..]),
            ("list.m3u", b""),
            ("night.rset", b"{}"),
            ("week.RQUEUE", b"{}"),
        ] {
            std::fs::write(root.join(name), bytes).unwrap();
        }
        let names = |l: &Listing| -> Vec<String> {
            l.files
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        let lanes = list_for(&root, Wanted::Lanes);
        assert!(
            names(&lanes).contains(&"film.en.srt".to_owned()),
            "{:?}",
            names(&lanes)
        );
        assert!(!names(&lanes).contains(&"list.m3u".to_owned()));
        assert!(!names(&lanes).contains(&"night.rset".to_owned()));
        assert!(names(&lanes).contains(&"a.mp3".to_owned()));
        let saved = list_for(&root, Wanted::Saved);
        assert_eq!(names(&saved), ["night.rset", "week.RQUEUE"]);
        assert_eq!(
            saved.glyphs,
            [
                ronoctua_tokens::icon::PLAYER_SET,
                ronoctua_tokens::icon::PLAYER_QUEUE
            ]
        );
        assert_eq!(saved.folders, lanes.folders, "the folders are every page's");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: a folder played whole skips a level, or queues a picture.
    #[test]
    fn a_folder_plays_everything_under_it_its_own_files_first() {
        let root = tree();
        let got: Vec<String> = gather(&root)
            .iter()
            .map(|p| {
                p.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            got,
            [
                "a.mp3",
                "B.flac",
                "tune.d00",
                "Album/01.ogg",
                "Album/02.opus",
                "Album/Disc 2/01.mkv"
            ]
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: a file played together becomes a lane of another kind than
    /// it is — a subtitle file a picture, a song a picture, a film a sound —
    /// or an address is taken for a file of the kind its name ends in.
    #[test]
    fn a_file_played_together_is_a_lane_of_its_kind() {
        use ronoctua_player::own::LaneKind;
        assert_eq!(lane_kind_of("/v/film.en.srt"), LaneKind::Subtitle);
        assert_eq!(lane_kind_of("/v/film.ass"), LaneKind::Subtitle);
        assert_eq!(lane_kind_of("/m/song.flac"), LaneKind::Audio);
        assert_eq!(lane_kind_of("/v/film.mkv"), LaneKind::Video);
        assert_eq!(
            lane_kind_of("https://example.org/a.flac?x=1"),
            LaneKind::Audio
        );
        assert_eq!(
            lane_kind_of("https://example.org/watch?v=1"),
            LaneKind::Video,
            "an address that names no kind is read by the player as a picture"
        );
    }

    /// FAILS IF: a folder that cannot be read looks like an empty one.
    #[test]
    fn a_folder_that_cannot_be_read_says_so() {
        let l = list(Path::new("/nonexistent/ronoctua"));
        assert!(l.refused.is_some());
    }
}
