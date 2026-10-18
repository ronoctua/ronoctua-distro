//! The KINDS of file the player takes: which files it plays, which it plays
//! together, which are its own, and whether a folder has anything to play.
//!
//! The player browses nothing of its own: a file is chosen in the one picker
//! of the system (`crate::file_picker`), and what the picker shows of a folder
//! for each of the player's purposes is a [`Filter`] from here.
//!
//! # What "a file the player plays" means, in one place
//!
//! The picker's filter asks it, and whether a folder has anything to play
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

use crate::file_picker::Filter;

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
/// player reports. So the picker asks it once, when the folder is read, and
/// hands the answer to the filters ([`Filter::keeps`]'s second argument).
#[must_use]
pub fn glyph_if_played(path: &Path) -> Option<char> {
    let mime = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(ronoctua_files::mime::by_name)
        .map(ronoctua_files::mime::canonical);
    played(path, mime, true)
}

/// [`glyph_if_played`]'s answer for a file whose type is `mime`, with the
/// lists left out when `lists` is `false`.
fn played(path: &Path, mime: Option<&'static str>, lists: bool) -> Option<char> {
    if let Some(mime) = mime {
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

/// What the player PLAYS, lists included: into the queue.
pub const MEDIA: Filter = Filter {
    label: crate::key::FILE_PICKER_FILTER_MEDIA,
    keeps: |path, mime| played(path, mime, true),
};

/// What plays TOGETHER with what plays — a lane: pictures, sounds and
/// subtitles, which play over a picture and never alone in the queue. A list
/// is no lane, and is left out, and so is a set or a queue of the player's.
pub const LANES: Filter = Filter {
    label: crate::key::FILE_PICKER_FILTER_LANES,
    keeps: |path, mime| {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ronoctua_player::subtitles::claims(extension) {
            return Some(ronoctua_tokens::icon::PLAYER_SUBTITLES);
        }
        if ronoctua_player::own::kind_by_name(path).is_some() {
            return None;
        }
        played(path, mime, false)
    },
};

/// The player's own files, a set (`.rset`) or a queue (`.rqueue`), to open.
pub const SAVED: Filter = Filter {
    label: crate::key::FILE_PICKER_FILTER_SAVED,
    keeps: |path, _| {
        use ronoctua_player::own::{OwnKind, kind_by_name};
        kind_by_name(path).map(|kind| match kind {
            OwnKind::Set => ronoctua_tokens::icon::PLAYER_SET,
            OwnKind::Queue => ronoctua_tokens::icon::PLAYER_QUEUE,
        })
    },
};

/// A SUBTITLE to lay over the picture: every file a subtitle's name says it
/// is (`subtitles::claims`), and the ones whose name says nothing more often
/// than a subtitle (`subtitles::GENERIC_EXTENSIONS` — a `.txt` of TMPlayer):
/// the person picking one is what makes it a subtitle.
pub const SUBTITLES: Filter = Filter {
    label: crate::key::FILE_PICKER_FILTER_SUBTITLES,
    keeps: |path, _| {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        (ronoctua_player::subtitles::claims(&extension)
            || ronoctua_player::subtitles::GENERIC_EXTENSIONS.contains(&extension.as_str()))
        .then_some(ronoctua_tokens::icon::PLAYER_SUBTITLES)
    },
};

/// The ring of the queue's picker.
pub const FOR_THE_QUEUE: &[Filter] = &[MEDIA, Filter::ANY];
/// The ring of what plays together.
pub const FOR_A_LANE: &[Filter] = &[LANES, Filter::ANY];
/// The ring of a set or a queue to open.
pub const FOR_OPENING: &[Filter] = &[SAVED, Filter::ANY];
/// The ring of a subtitle file.
pub const FOR_A_SUBTITLE: &[Filter] = &[SUBTITLES, Filter::ANY];
/// The ring of a picture for a cover.
pub const FOR_A_COVER: &[Filter] = crate::file_picker::PICTURES;
/// The ring of where a set or the queue is saved: its own files, which a
/// name typed may be about to write over.
pub const FOR_SAVING: &[Filter] = &[SAVED, Filter::ANY];

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
    use ronoctua_files::Kind;
    if depth > DEPTH_CEILING || out.len() >= FILE_CEILING {
        return;
    }
    // THE FILE MANAGER'S OWN READING and order — names folded, folders
    // first — walked here and never shown: what a folder played whole puts
    // in the queue. Hidden entries stay out, as the picker leaves them out.
    let Ok(entries) =
        ronoctua_files::listing::read_dir_at(at, ronoctua_files::listing::Sort::default())
    else {
        return;
    };
    let mut folders = Vec::new();
    for e in entries
        .iter()
        .filter(|e| !e.is_parent() && !e.name.starts_with('.'))
    {
        let path = at.join(&e.name);
        if e.opens_as() == Kind::Directory {
            if e.kind != Kind::Symlink {
                folders.push(path);
            }
            continue;
        }
        if out.len() >= FILE_CEILING {
            return;
        }
        if glyph_if_played(&path).is_some() {
            out.push(path);
        }
    }
    for folder in folders {
        walk(&folder, depth + 1, out);
    }
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

    /// FAILS IF: a filter of the player's keeps what another purpose is
    /// for — a list as a lane, a set among what plays, a picture as a
    /// subtitle — or leaves out a sound only one of the decoders reads.
    #[test]
    fn each_filter_keeps_what_its_purpose_is_for() {
        let keeps = |f: Filter, name: &str| {
            let path = Path::new("/m").join(name);
            let mime = ronoctua_files::mime::by_name(name).map(ronoctua_files::mime::canonical);
            (f.keeps)(&path, mime).is_some()
        };
        for played in ["a.mp3", "B.flac", "tune.d00", "list.m3u"] {
            assert!(keeps(MEDIA, played), "{played} plays");
        }
        for not in ["cover.jpg", "notes.txt"] {
            assert!(!keeps(MEDIA, not), "{not} does not play");
        }
        assert!(keeps(LANES, "film.en.srt"));
        assert!(keeps(LANES, "a.mp3"));
        assert!(!keeps(LANES, "list.m3u"), "a list is no lane");
        assert!(!keeps(LANES, "night.rset"), "a set is no lane");
        assert!(keeps(SAVED, "night.rset"));
        assert!(keeps(SAVED, "week.RQUEUE"));
        assert!(!keeps(SAVED, "a.mp3"));
        assert!(keeps(SUBTITLES, "film.ass"));
        assert!(
            keeps(SUBTITLES, "film.txt"),
            "a name that says nothing is the person's to call"
        );
        assert!(!keeps(SUBTITLES, "film.mkv"));
        for ring in [
            FOR_THE_QUEUE,
            FOR_A_LANE,
            FOR_OPENING,
            FOR_A_SUBTITLE,
            FOR_A_COVER,
            FOR_SAVING,
        ] {
            assert_eq!(
                ring.last().map(|f| f.label),
                Some(Filter::ANY.label),
                "every ring ends with every file"
            );
        }
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

    /// FAILS IF: a folder that cannot be read is walked into anything.
    #[test]
    fn a_folder_that_cannot_be_read_plays_nothing() {
        assert!(gather(Path::new("/nonexistent/ronoctua")).is_empty());
    }
}
