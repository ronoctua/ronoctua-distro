//! What one video is CALLED, asked without playing or downloading it.
//!
//! The player puts an address in its queue at once — it plays before
//! anything is known of it — and asks this on a thread of its own, so the
//! leaf is named by the video's own title and not by the last piece of its
//! address (`ronoctua-player-engine`, `engine/resolve.rs`, `Job::Title`).
//!
//! `--print title`: one line of plain text. yt-dlp still has to read the
//! page — the title is one of the fields of the video — so this goes out with
//! the PO token's flags ([`crate::pot_flags`]) like every request that reads
//! a video, and runs under the listings' deadline and runner ([`crate::run`]:
//! killed past the deadline, never left behind).
//!
//! # A page that is a list is named as the list
//!
//! An album's page, a channel's: `--print title` alone would read every
//! entry of it — each a request, until the deadline killed it — and print
//! the first one's title, naming the leaf after its first track.
//! `--playlist-items 0` takes none of them, and `--print playlist:title`
//! prints the list's own name once, at its end; a single video ignores both.
//! And `--ignore-no-formats-error`: a name is not a format, and a video whose
//! formats yt-dlp cannot pick is still called something. Measured with the
//! pinned yt-dlp on a page of two `<video>`s: the list's `<title>`, where the
//! flags before printed nothing and failed on its entries.

use std::path::Path;

use crate::address::is_address;
use crate::{DEADLINE, Trouble, run};

/// The longest title kept, in characters: a title longer than a line of
/// the widest door is a description somebody pasted into it.
pub const TITLE_MOST_CHARS: usize = 300;

/// The most bytes the answer may be: one line, and a margin.
const MOST_BYTES: usize = 16 * 1024;

/// How long yt-dlp waits on one socket before giving up on it.
const SOCKET_SECONDS: u32 = 10;

/// The arguments that print what `url` is called, with `pot` (the PO
/// token's flags, [`crate::pot_flags`]) among them. Pure.
///
/// # Errors
///
/// [`Trouble::NotAnAddress`] for anything but a plain web address.
pub fn title_arguments(url: &str, pot: &[String]) -> Result<Vec<String>, Trouble> {
    if !is_address(url) {
        return Err(Trouble::NotAnAddress);
    }
    let mut args: Vec<String> = [
        // NOT THE PERSON'S CONFIG: an `--output` or a `--format` somebody
        // keeps for their own downloads is not this question's.
        "--ignore-config",
        "--no-warnings",
        "--skip-download",
        // ONE VIDEO: a watch address inside a list is that video, not the
        // list's first title.
        "--no-playlist",
        // A PAGE THAT IS A LIST: none of its entries read, and its own name.
        "--playlist-items",
        "0",
        "--ignore-no-formats-error",
        "--print",
        "title",
        "--print",
        "playlist:title",
        "--socket-timeout",
        &SOCKET_SECONDS.to_string(),
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    args.extend(pot.iter().cloned());
    // THE ADDRESS AFTER `--`, so nothing in it is ever read as a flag.
    args.push("--".to_owned());
    args.push(url.to_owned());
    Ok(args)
}

/// What `url` is called, asked of this installation's yt-dlp. Blocks for up
/// to [`DEADLINE`]: call it on a thread.
///
/// # Errors
///
/// [`Trouble::Absent`] where this installation has no yt-dlp, and every
/// trouble of [`title_by`].
pub fn title(url: &str) -> Result<String, Trouble> {
    if !crate::installed() {
        return Err(Trouble::Absent);
    }
    title_by(&crate::program(), url, &crate::pot_flags())
}

/// [`title`], asked of `program` — the one a test hands a stand-in for.
///
/// # Errors
///
/// [`Trouble::NotAnAddress`], every trouble of a run ([`Trouble::Slow`],
/// [`Trouble::Failed`], …), and [`Trouble::Garbled`] for an answer that names
/// nothing.
pub fn title_by(program: &Path, url: &str, pot: &[String]) -> Result<String, Trouble> {
    let args = title_arguments(url, pot)?;
    let bytes = run::json(program, &args, DEADLINE, MOST_BYTES)?;
    parse_title(&bytes)
}

/// The title out of what `--print title` wrote: its first line that says
/// something, trimmed and cut at [`TITLE_MOST_CHARS`]. Pure.
///
/// # Errors
///
/// [`Trouble::Garbled`] when it said nothing, or yt-dlp's own `NA` — what it
/// prints for a field the video does not have.
pub fn parse_title(bytes: &[u8]) -> Result<String, Trouble> {
    let text = String::from_utf8_lossy(bytes);
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .ok_or(Trouble::Garbled)?;
    if line == "NA" {
        return Err(Trouble::Garbled);
    }
    Ok(line.chars().take(TITLE_MOST_CHARS).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the address can be read as a flag, the PO token's flags
    /// stay behind, the question could download or walk a list, or a page
    /// that is a list is not asked for the list's own name.
    #[test]
    fn a_title_is_asked_of_one_video_with_the_minter_s_flags() {
        let pot = vec!["--plugin-dirs".to_owned(), "/x/plugin".to_owned()];
        let args = title_arguments("https://youtu.be/dQw4w9WgXcQ", &pot).expect("an address");
        assert_eq!(
            args.last().map(String::as_str),
            Some("https://youtu.be/dQw4w9WgXcQ")
        );
        assert_eq!(args[args.len() - 2], "--");
        for flag in [
            "--skip-download",
            "--no-playlist",
            "--ignore-config",
            "--ignore-no-formats-error",
            "--plugin-dirs",
        ] {
            assert!(args.iter().any(|a| a == flag), "{flag} missing: {args:?}");
        }
        let printed: Vec<&str> = args
            .windows(2)
            .filter(|w| w[0] == "--print")
            .map(|w| w[1].as_str())
            .collect();
        assert_eq!(printed, ["title", "playlist:title"]);
        let items = args
            .iter()
            .position(|a| a == "--playlist-items")
            .expect("--playlist-items");
        assert_eq!(args[items + 1], "0", "a list's entries are never read");
        assert_eq!(
            title_arguments("-o /tmp/x", &[]),
            Err(Trouble::NotAnAddress),
            "a flag is never taken for an address"
        );
    }

    /// FAILS IF: yt-dlp's `NA` or an empty answer becomes a name, a title
    /// keeps its line break, or a pasted essay is kept whole.
    #[test]
    fn a_title_is_its_first_line_and_never_na() {
        assert_eq!(
            parse_title(b"\n  A song \nsecond\n"),
            Ok("A song".to_owned())
        );
        assert_eq!(parse_title(b"NA\n"), Err(Trouble::Garbled));
        assert_eq!(parse_title(b"  \n"), Err(Trouble::Garbled));
        let long = "é".repeat(TITLE_MOST_CHARS + 20);
        assert_eq!(
            parse_title(long.as_bytes()).map(|t| t.chars().count()),
            Ok(TITLE_MOST_CHARS)
        );
    }

    /// FAILS IF: the title is not read from what the program printed, or a
    /// program that fails is taken for an answer — run against a stand-in
    /// for yt-dlp, a script that prints what it was asked.
    #[test]
    fn a_stand_in_for_yt_dlp_is_asked_and_answers() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = std::env::temp_dir().join(format!("ronoctua-ytdlp-title-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        let program = dir.join("yt-dlp");
        // THE LAST ARGUMENT is the address: the stand-in names the video by it.
        std::fs::write(
            &program,
            "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\nprintf 'Video at %s\\n' \"$last\"\n",
        )
        .expect("a stand-in");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .expect("runnable");
        assert_eq!(
            title_by(&program, "https://example.org/v/1", &[]),
            Ok("Video at https://example.org/v/1".to_owned())
        );
        let failing = dir.join("failing");
        std::fs::write(&failing, "#!/bin/sh\necho 'ERROR: no' >&2\nexit 1\n").expect("a stand-in");
        std::fs::set_permissions(&failing, std::fs::Permissions::from_mode(0o755))
            .expect("runnable");
        assert_eq!(
            title_by(&failing, "https://example.org/v/1", &[]),
            Err(Trouble::Failed)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
