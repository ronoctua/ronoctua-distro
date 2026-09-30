//! A list of things to play, read out of the file that names them.
//!
//! # What lives here, and what lives in `ronoctua-playlist`
//!
//! The PARSERS moved to their own crate, because the player daemon reads
//! playlists too and must not link this one. What stays is the part that is
//! about MEDIA TYPES — which ones are a playlist, which spelling each one is,
//! and the refusal the preview draws — because the three functions that must
//! agree about what a playlist IS (`pixels::how`, `text::is_text` and
//! `facts::needs`) are all in this crate, and the table and the dispatch
//! belong beside them.

use std::path::Path;

pub use ronoctua_playlist::{BYTE_CEILING, ENTRY_CEILING, Entry, Kind, List, TIMED_CEILING, Where};

use crate::Refusal;
use crate::key;

/// Which types are a list of things to play rather than one of them.
///
/// The five the pinned database knows, `application/xspf+xml`, which lives
/// outside the `audio` branch and is a playlist all the same, and the
/// player's own two (`assets/player-types.nix`, `own`): a `.rset` — media
/// played together — and a `.rqueue`, a queue. Both are JSON underneath, and
/// what a person wants to see of one is what it plays, not its braces.
pub const TYPES: [&str; 8] = [
    "audio/x-mpegurl",
    "application/vnd.apple.mpegurl",
    "audio/x-scpls",
    "application/xspf+xml",
    "audio/x-ms-asx",
    "audio/x-iriver-pla",
    "application/x-ronoctua-set+json",
    "application/x-ronoctua-queue+json",
];

/// The two spellings this reader claims and does NOT open.
///
/// Declared rather than left as "whatever `kind_of` did not match", because
/// the two are different things: one is a decision, the other is an omission,
/// and only the first should reach a person as a sentence. With this list,
/// `playlist-formats-are-total` can tell them apart — a seventh type added to
/// [`TYPES`] and to neither side fails the check instead of quietly getting a
/// refusal written for somebody else's format.
pub const REFUSED: [&str; 1] = ["audio/x-iriver-pla"];

/// Whether this type names other files rather than carrying sound.
#[must_use]
pub fn is_a_playlist(mime: &str) -> bool {
    let mime = ronoctua_files::mime::canonical(mime);
    TYPES.contains(&mime)
}

/// Which parser a type gets, or `None` for one that is refused.
fn kind_of(mime: &str) -> Option<Kind> {
    match ronoctua_files::mime::canonical(mime) {
        "audio/x-mpegurl" | "application/vnd.apple.mpegurl" => Some(Kind::M3u),
        "audio/x-scpls" => Some(Kind::Pls),
        "application/xspf+xml" => Some(Kind::Xspf),
        "audio/x-ms-asx" => Some(Kind::Asx),
        "application/x-ronoctua-set+json" | "application/x-ronoctua-queue+json" => Some(Kind::Own),
        _ => None,
    }
}

/// The list a file holds, if it holds one.
///
/// Three answers, and they are `text::of`'s three: `None` is "this is not a
/// playlist and I have no opinion", `Some(Ok)` is the list, and `Some(Err)` is
/// a sentence saying why there is none.
#[must_use]
pub fn of(path: &Path, mime: &str) -> Option<Result<List, Refusal>> {
    if !is_a_playlist(mime) {
        return None;
    }
    // ONE SPELLING THIS READER DOES NOT OPEN, and it says so rather than
    // showing an empty list.
    //
    // PLA is iRiver's, a binary format with no published specification:
    // writing a parser from a reverse-engineering note and never running it
    // against a real file is what `mesh/` refuses to do per format, and the
    // same answer applies.
    //
    // ASX USED TO BE HERE, and the owner asked about it by name: *"não sei se
    // o arquivo 'playlist.asx' foi criado com a intenção de ser visto no
    // preview, pois ocorre que ele não abre"*. It was refused on the grounds
    // that its `<entryref href=…>` is a playlist inside a playlist BY URL and
    // there is no network here — which is an argument about ONE of its
    // elements, not about the format. An `<entry><ref href=…/></entry>` names
    // a file exactly the way an `.m3u` line does, and that is what his own
    // file holds. The element that would need the network is simply not read,
    // which is what every other reader here does with what it cannot answer.
    let Some(kind) = kind_of(mime) else {
        debug_assert!(
            REFUSED.contains(&ronoctua_files::mime::canonical(mime)),
            "a claimed type with no reader and no place among the refused"
        );
        return Some(Err(Refusal::Unsupported(key::PREVIEW_PLAYLIST_UNREAD)));
    };
    if kind == Kind::Own {
        return Some(own(path));
    }
    Some(
        ronoctua_playlist::read(path, kind).map_err(|trouble| match trouble {
            ronoctua_playlist::Trouble::Unreadable => Refusal::Unreadable,
            ronoctua_playlist::Trouble::Empty => Refusal::Empty,
        }),
    )
}

/// One of the player's own files, listed: a set as one row, a queue as what
/// it plays, flat.
///
/// Its refusals are its own, because they are different facts: a file that
/// is not one of ours (or is ours and broken) may be damaged, one a NEWER
/// player wrote is intact and this reader is too old for it.
fn own(path: &Path) -> Result<List, Refusal> {
    use ronoctua_playlist::own::{self, OWN_BYTE_CEILING};
    let read = own::read(path).map_err(|refusal| match refusal {
        own::Refusal::Unreadable => Refusal::Unreadable,
        own::Refusal::TooLarge => Refusal::TooBig(OWN_BYTE_CEILING),
        own::Refusal::NotOurs | own::Refusal::Damaged => Refusal::Failed,
        own::Refusal::Newer { .. } => Refusal::Unsupported(key::PREVIEW_OWN_NEWER),
    })?;
    ronoctua_playlist::list_of_own(&read).map_err(|_| Refusal::Empty)
}

#[cfg(test)]
mod tests {
    use super::{is_a_playlist, of};
    use std::path::Path;

    /// FAILS IF: the player's own files stop being listed as what they play,
    /// or a queue from a newer player is called damaged.
    #[test]
    fn the_players_own_files_are_listed_and_a_newer_one_says_so() {
        let dir = std::env::temp_dir().join(format!("ronoctua-own-preview-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let queue = dir.join("evening.rqueue");
        std::fs::write(
            &queue,
            r#"{"format":"ronoctua-queue","version":1,"entries":[
                {"playlist":{"name":"P","entries":[{"single":{"location":"a.flac"}},
                {"single":{"location":"b.flac"}}]}}]}"#,
        )
        .expect("written");
        let listed = of(&queue, "application/x-ronoctua-queue+json")
            .expect("claimed")
            .expect("listed");
        assert_eq!(listed.entries.len(), 2);
        let newer = dir.join("later.rqueue");
        std::fs::write(&newer, r#"{"format":"ronoctua-queue","version":99}"#).expect("written");
        assert!(matches!(
            of(&newer, "application/x-ronoctua-queue+json"),
            Some(Err(crate::Refusal::Unsupported(
                crate::key::PREVIEW_OWN_NEWER
            )))
        ));
        let not_ours = dir.join("x.rset");
        std::fs::write(&not_ours, "{}").expect("written");
        assert!(matches!(
            of(&not_ours, "application/x-ronoctua-set+json"),
            Some(Err(crate::Refusal::Failed))
        ));
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    /// FAILS IF: a type this reader cannot open is answered with an empty list
    /// rather than a sentence.
    #[test]
    fn the_spelling_this_reader_does_not_open_says_so() {
        let nowhere = Path::new("/nonexistent.pla");
        for kind in super::REFUSED {
            assert!(is_a_playlist(kind));
            match of(nowhere, kind) {
                Some(Err(crate::Refusal::Unsupported(key))) => {
                    assert_eq!(key, crate::key::PREVIEW_PLAYLIST_UNREAD);
                }
                other => panic!("{kind} answered {other:?}"),
            }
        }
        // AND A FILE THAT IS NOT A PLAYLIST GETS NO OPINION AT ALL.
        assert!(of(nowhere, "audio/mpeg").is_none());
        assert!(of(nowhere, "image/png").is_none());
        // THE ONES THAT ARE READ reach the reader, and fail on the file
        // rather than on the spelling.
        for kind in [
            "audio/x-mpegurl",
            "audio/x-scpls",
            "application/xspf+xml",
            "audio/x-ms-asx",
            "application/x-ronoctua-set+json",
            "application/x-ronoctua-queue+json",
        ] {
            assert!(matches!(
                of(nowhere, kind),
                Some(Err(crate::Refusal::Unreadable))
            ));
        }
    }
}
