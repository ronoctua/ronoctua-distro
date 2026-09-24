//! The player's own look at the filesystem: which folders and files it lists,
//! and what a folder plays.
//!
//! # What "a file the player plays" means, in one place
//!
//! Three things ask it — the listing of the browser, a folder typed where a
//! file was asked, and `ronoctua-shell play` handed a folder — and the answer
//! has to be the same for all three, or a folder would play files its listing
//! never showed. [`glyph_if_played`] is that answer: a media type under `audio/` or
//! `video/` (which covers the types `assets/player-types.nix` invents for the
//! decoders' families), the handful of media types that live under
//! `application/`, a playlist, or a name one of the decoders claims.
//!
//! # It is the SHELL that opens a folder, not the daemon
//!
//! libmpv would take a directory itself, and would get two things wrong: it
//! would queue every file in it — pictures and text included — and a file only
//! a decoder of ours reads would reach it by its path instead of through the
//! decoder's protocol, and refuse. The type database is on this side of the
//! socket, so the folder is turned into the files it plays here and the daemon
//! is handed a list, which it already knows how to take.

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
    let name = path.file_name().and_then(|n| n.to_str())?;
    if let Some(mime) = ronoctua_files::mime::by_name(name) {
        let mime = ronoctua_files::mime::canonical(mime);
        if mime.starts_with("audio/")
            || mime.starts_with("video/")
            || ALSO_MEDIA.contains(&mime)
            || ronoctua_inspect::playlist::is_a_playlist(mime)
        {
            return Some(ronoctua_files::mime::icon_of(mime));
        }
    }
    ronoctua_player::decoders::for_path(path).map(|_| ronoctua_tokens::icon::MUSIC)
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
        } else if let Some(glyph) = glyph_if_played(&path) {
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

/// What an order to play names, with a folder turned into its files.
///
/// An address and a file go through as they are; a folder becomes every file
/// under it the player plays, and a folder with none becomes nothing — which
/// the caller says, rather than handing the daemon an empty order.
#[must_use]
pub fn items_of(said: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for item in said {
        let path = Path::new(item);
        if !item.contains("://") && path.is_dir() {
            out.extend(
                gather(path)
                    .into_iter()
                    .map(|p| p.to_string_lossy().into_owned()),
            );
        } else {
            out.push(item.clone());
        }
    }
    out
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
    let music = ronoctua_xdg::user_dir("XDG_MUSIC_DIR", "Music");
    [
        (key::PLAYER_PLACE_MUSIC, music.clone()),
        (
            key::PLAYER_PLACE_VIDEOS,
            ronoctua_xdg::user_dir("XDG_VIDEOS_DIR", "Videos"),
        ),
        (
            key::PLAYER_PLACE_MUSIC_VIDEOS,
            ronoctua_xdg::user_dir_declared("XDG_MUSIC_VIDEO_DIR").unwrap_or(music),
        ),
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

    /// FAILS IF: a folder typed where a file was asked reaches the daemon as a
    /// folder, or an address is taken for one.
    #[test]
    fn an_order_turns_a_folder_into_its_files_and_nothing_else() {
        let root = tree();
        let album = root.join("Album").to_string_lossy().into_owned();
        let one = root.join("a.mp3").to_string_lossy().into_owned();
        let got = items_of(&[album, one.clone(), "https://example.org/x".to_owned()]);
        assert_eq!(got.len(), 5, "{got:?}");
        assert!(got[0].ends_with("Album/01.ogg"));
        assert_eq!(got[3], one);
        assert_eq!(got[4], "https://example.org/x");
        assert!(items_of(&[root.join("empty").to_string_lossy().into_owned()]).is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: a folder that cannot be read looks like an empty one.
    #[test]
    fn a_folder_that_cannot_be_read_says_so() {
        let l = list(Path::new("/nonexistent/ronoctua"));
        assert!(l.refused.is_some());
    }
}
