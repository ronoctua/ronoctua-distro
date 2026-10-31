//! The other thing that is inside a file: its letters.
//!
//! # Why the preview reads text at all
//!
//! Because the panel is not an image panel. It answers "what is in this file",
//! and for most of the files on a developer's machine the answer is letters —
//! a `.nix`, a `.rs`, a unit file, a README. Until this module existed the
//! panel answered those with a sentence about pictures, which is a true
//! statement about a question nobody asked.
//!
//! # Why here and not in `ronoctua-files`
//!
//! That crate builds `ronoctua-files-root`, the binary that runs as **root**,
//! and `nix/rust.nix` extracts it precisely so what runs privileged does not
//! carry the desktop — `files-vm` asserts `wgpu`, `wasmtime`, `mesa` and the
//! font are absent from its closure. A highlighter and a table of thirty-seven
//! languages inside a root binary is the trap this crate's own manifest spends
//! a paragraph on.
//!
//! And not in the shell either: `preview_fields` runs inside the frame that
//! draws, once per line, once per frame. An `open(2)` there is the shape that
//! `systemd-run --scope` already cost this project once.
//!
//! # Everything expensive happens ONCE
//!
//! Reading, splitting, expanding tabs and computing one highlight state per
//! line all happen here, on the inspector's own thread, and the answer is
//! cached under `Stamp::key()` beside the pixels. What is left for the frame is
//! indexing a line and walking a handful of ranges.

use std::{fs::File, io::Read as _, path::Path};

use crate::Refusal;

/// The most this module reads off disk for one preview.
///
/// # A mebibyte, and the number is argued twice
///
/// It is a thirty-second of [`crate::tools::STDOUT_CEILING`]: a path that
/// spawns no process at all must not be allowed more room than a child
/// process gets. And a ceiling that is STATED is a limit, while an unbounded
/// read is a surprise — the sentence `ronoctua_files::mime::HEAD` already
/// carries, about a much smaller read.
pub const BYTE_CEILING: u64 = 1024 * 1024;

/// The most lines it keeps.
///
/// # Two thousand, and the argument is the SCROLLBAR
///
/// Not memory. `ronoctua_design::Scroll` floors the thumb at one line, so at
/// two thousand lines in a panel a dozen lines tall the thumb is eight per cent of
/// the track — small, and still something a pointer can catch. At a hundred
/// thousand it is a dot, and one detent of the wheel moves three lines out of
/// a hundred thousand. A bar that cannot be used is a bar that lies about
/// where you are, which is worse than no bar.
///
/// Two thousand lines is a large source file read whole; past it the panel
/// says so on its last row.
pub const LINE_CEILING: usize = 2_000;

/// The most bytes of ONE line it keeps.
///
/// # Five hundred and twelve, and this is the one that would have hurt
///
/// A minified `.js` is ONE line of two megabytes. Whoever draws elides with
/// `ronoctua_render::layout::elide_end`, which walks its whole argument by
/// grapheme cluster before cutting — so that two-megabyte `String` would be
/// walked once per frame, in the frame that draws. Five hundred and twelve is
/// ten times the widest panel this shell can produce, and the cut lands on a
/// `char` boundary.
pub const LINE_BYTES: usize = 512;

/// How much is read before deciding the bytes are not text at all.
const SNIFF: usize = 4096;

/// One line, already cut, already de-tabbed, already highlighted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// What it says.
    pub text: String,
    /// Byte ranges into `text`: ascending, non-overlapping, on `char`
    /// boundaries. Whoever draws slices by these INSIDE the frame that draws.
    pub runs: Vec<ronoctua_syntax::Range>,
}

/// What was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// The lines.
    pub lines: Vec<Line>,
    /// There was more than what is here, and the panel says so on its last row.
    ///
    /// NOT A [`Refusal`], and the difference is the whole reason it is a
    /// `bool`: a refusal means there is nothing on screen and here is why, and
    /// a cut file has two thousand lines on screen. Modelling it as a refusal
    /// would replace the content with a sentence about the content.
    pub cut: bool,
    /// What the ranges were produced for; `None` is prose, and prose is not
    /// coloured.
    pub language: Option<ronoctua_syntax::Language>,
}

/// Whether this type is text at all.
///
/// # One question, one answer, and freedesktop's
///
/// `is_a(mime, "text/plain")` and nothing else. There is no
/// `starts_with("text/")` clause: every `text/*` in the table descends from
/// `text/plain`, and a second rule written by hand is a second answer to a
/// question the database settles. That "every" is the SPEC's — its implicit
/// rule, which shared-mime-info 2.5.1 stopped spelling out on
/// `text/x-shellscript`, the AWK and sed scripts and `text/x-gradle` — and
/// `nix/scripts/gen-mimedb.py` writes it into the table as rows, so it is
/// still the database answering.
///
/// It reaches further than it looks. Some three hundred types reach
/// `text/plain` through the subclass DAG — `application/json` at depth three,
/// `application/xml`, `text/x-shellscript`, `application/toml`,
/// `application/yaml`, `application/x-desktop`, `application/sql`. None of
/// them needs naming here.
///
/// And the four descendants that are NOT text — `image/svg+xml`,
/// `image/x-eps`, `audio/x-mpegurl`, `video/vnd.mpegurl` — need no exception
/// list either, because something is always asked FIRST: the two images are
/// asked of the picture, and the two playlists are asked of
/// [`crate::playlist`].
///
/// The playlists were the exception for a while, and the sentence here said
/// *"an `.m3u` genuinely is text"* — which was true and is no longer the
/// point. It IS text, and showing it as text meant showing a person the paths
/// of nine tracks with the slashes syntax-coloured, rather than the nine
/// tracks. What decides is not what a file is made of but what it is FOR.
#[must_use]
pub fn is_text(mime: &str) -> bool {
    ronoctua_files::mime::is_a(mime, "text/plain")
}

/// The file as lines, when it is text.
///
/// `None` — it is not text, this module has no opinion, and the picture path's
/// own answer stands. `Some(Err)` — it IS text and here is why there is none.
/// `Some(Ok)` — the lines.
///
/// `tab` arrives as a PARAMETER and is never read here: it is
/// `space.files.tabWidth`, a token, and this crate does not receive token
/// values. Whoever calls holds the bundle and the tokens both.
#[must_use]
pub fn of(path: &Path, mime: &str, tab: usize) -> Option<Result<Read, Refusal>> {
    if !is_text(mime) {
        return None;
    }
    Some(read_it(path, mime, tab))
}

/// The whole of it, once the type has been settled.
fn read_it(path: &Path, mime: &str, tab: usize) -> Result<Read, Refusal> {
    let (text, mut cut) = decoded(path, BYTE_CEILING)?;
    let text = text.as_str();

    let language = ronoctua_syntax::of_mime(mime).or_else(|| ronoctua_syntax::guess(text));

    let mut lines: Vec<Line> = Vec::new();
    let mut state = ronoctua_syntax::State::fresh();
    let mut runs = Vec::new();
    for row in text.lines() {
        if lines.len() >= LINE_CEILING {
            cut = true;
            break;
        }
        // TABS FIRST, AND HIGHLIGHTING SECOND. The ranges are byte offsets
        // into the finished string, so expanding afterwards would move every
        // one of them.
        let flat = flatten(row, tab);
        match language {
            Some(l) => {
                state = ronoctua_syntax::line_into(&flat, l.grammar(), state, &mut runs);
            }
            None => runs.clear(),
        }
        lines.push(Line {
            text: flat,
            runs: runs.clone(),
        });
    }
    if lines.is_empty() {
        return Err(Refusal::Empty);
    }

    Ok(Read {
        lines,
        cut,
        language,
    })
}

/// The first `ceiling` bytes of a text file as a string, and whether there
/// was more — the half of reading that the panel and the index share
/// (`crate::plain` asks with the index's ceiling, and never highlights).
pub(crate) fn decoded(path: &Path, ceiling: u64) -> Result<(String, bool), Refusal> {
    let mut file = File::open(path).map_err(|_| Refusal::Unreadable)?;
    let mut raw = Vec::new();
    // TAKE, so a four-hundred-megabyte file does not allocate itself into the
    // inspector's thread before anything gets to check its size. The ceiling
    // is the read, not a test after one.
    file.by_ref()
        .take(ceiling)
        .read_to_end(&mut raw)
        .map_err(|_| Refusal::Unreadable)?;
    // Whether there was more: one more byte is all this needs to know.
    let mut over = [0u8; 1];
    let cut = matches!(file.read(&mut over), Ok(n) if n > 0);

    if raw.is_empty() {
        return Err(Refusal::Empty);
    }
    // A NUL IN THE HEAD IS WHAT SEPARATES a file that lies about being text
    // from a file in Latin-1, which is merely unreadable as UTF-8 and is still
    // a file of letters.
    if raw[..raw.len().min(SNIFF)].contains(&0) {
        return Err(Refusal::NotText);
    }

    match String::from_utf8(raw) {
        Ok(s) => Ok((s, cut)),
        Err(e) => {
            // A CHARACTER THE CEILING CUT IN HALF is not a broken file. It is
            // this module's own cut, and refusing here would make a large
            // valid file unreadable one time in four.
            let utf8 = e.utf8_error();
            let mut raw = e.into_bytes();
            let cut_by_us = utf8.error_len().is_none() && raw.len() as u64 == ceiling;
            if !cut_by_us {
                return Err(Refusal::NotText);
            }
            raw.truncate(utf8.valid_up_to());
            String::from_utf8(raw)
                .map(|s| (s, cut))
                .map_err(|_| Refusal::NotText)
        }
    }
}

/// One line, with its tabs opened out and its carriage return gone.
///
/// # The `\r` is not cosmetic
///
/// Without stripping it, every line of a file written on Windows ends in a
/// glyph that draws nothing and that `width_of` counts as a cell — so every
/// line is elided one cell early, in a panel where a cell is a twenty-fourth
/// of the width.
///
/// The tab expands to the NEXT multiple of `tab`, not to `tab` spaces: that is
/// what makes a second level of indentation line up under a first when the
/// first was reached by other characters.
///
/// # And it is the DOCUMENT reader's too
///
/// It was private, and `document::lines_of` built its lines without it — so a
/// `.rs` reached the panel de-tabbed and an `.rtf` did not. That is two rulers
/// again, and this one has teeth: the panel budgets in CELLS
/// (`ronoctua_protocol::cells`, which calls a tab ONE) and the shaper advances
/// it to the next stop, so a line with a tab in it was measured as fitting and
/// drawn four cells longer — over the column beside it. The owner photographed
/// exactly that, with `sample.rtf`'s bullet list running across the facts.
pub(crate) fn flatten(row: &str, tab: usize) -> String {
    let row = row.strip_suffix('\r').unwrap_or(row);
    let mut out = String::with_capacity(row.len());
    let mut column = 0usize;
    for c in row.chars() {
        if out.len() >= LINE_BYTES {
            break;
        }
        if c == '\t' {
            let step = if tab == 0 { 1 } else { tab - (column % tab) };
            for _ in 0..step {
                out.push(' ');
            }
            column += step;
            continue;
        }
        out.push(c);
        column += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::{LINE_BYTES, LINE_CEILING, Read, flatten, is_text, of};
    use crate::Refusal;

    fn scratch(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-text-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("create");
        f.write_all(bytes).expect("write");
        path
    }

    fn read(name: &str, bytes: &[u8], mime: &str) -> Option<Result<Read, Refusal>> {
        of(&scratch(name, bytes), mime, 4)
    }

    /// FAILS IF: somebody writes a second rule beside the database's. Every one
    /// of these is settled by the subclass DAG, and none of them is named in
    /// this crate.
    ///
    /// The shell script both ways: `text/x-shellscript`, its name since
    /// shared-mime-info 2.5.1, whose only explicit parent is
    /// `application/x-executable` (it is text by the spec's implicit rule,
    /// which the generator writes down), and `application/x-shellscript`, the
    /// old name, now an alias. And `text/x-gradle`, which 2.5.1 gives no parent
    /// at all.
    #[test]
    fn what_counts_as_text_comes_from_the_database() {
        for yes in [
            "text/plain",
            "text/markdown",
            "text/rust",
            "text/x-nix",
            "text/x-csrc",
            "text/html",
            "application/json",
            "application/xml",
            "application/toml",
            "application/yaml",
            "text/x-shellscript",
            "application/x-shellscript",
            "text/x-gradle",
            "application/x-desktop",
            "application/sql",
        ] {
            assert!(is_text(yes), "`{yes}` should be text");
        }
        for no in [
            "application/pdf",
            "image/png",
            "image/jpeg",
            "video/mp4",
            "application/zip",
            "inode/directory",
        ] {
            assert!(!is_text(no), "`{no}` should not be text");
        }
    }

    /// FAILS IF: a picture's bytes are offered to the text path.
    #[test]
    fn a_type_that_is_not_text_gets_no_opinion() {
        assert!(read("a.png", b"\x89PNG\r\n", "image/png").is_none());
    }

    /// FAILS IF: the NUL test goes, and a file that lies about being text
    /// arrives as a panel of replacement characters.
    #[test]
    fn a_nul_byte_means_it_is_not_text() {
        let out = read("b.txt", b"hello\0world", "text/plain");
        assert_eq!(out, Some(Err(Refusal::NotText)));
    }

    /// FAILS IF: `from_utf8_lossy` creeps in, and confetti replaces the truth.
    #[test]
    fn invalid_utf8_in_the_middle_means_it_is_not_text() {
        let out = read("c.txt", b"good\xC3\x28bad", "text/plain");
        assert_eq!(out, Some(Err(Refusal::NotText)));
    }

    /// FAILS IF: an empty file draws an empty panel, which looks broken.
    #[test]
    fn an_empty_file_says_so() {
        assert_eq!(read("d.txt", b"", "text/plain"), Some(Err(Refusal::Empty)));
    }

    /// FAILS IF: the `\r` survives. Every line would elide one cell early.
    #[test]
    fn a_windows_line_ending_leaves_nothing_behind() {
        let out = read("e.txt", b"one\r\ntwo\r\n", "text/plain")
            .unwrap()
            .unwrap();
        assert_eq!(out.lines[0].text, "one");
        assert_eq!(out.lines[1].text, "two");
    }

    /// FAILS IF: the tab stops being expanded, or is expanded to a fixed run
    /// instead of to the next stop.
    #[test]
    fn a_tab_reaches_the_next_stop() {
        assert_eq!(flatten("\tx", 4), "    x");
        assert_eq!(flatten("ab\tx", 4), "ab  x");
        assert_eq!(flatten("abcd\tx", 4), "abcd    x");
        assert_eq!(flatten("a\tb\tc", 4), "a   b   c");
    }

    /// FAILS IF: the line ceiling goes, and a two-megabyte single line is
    /// walked by grapheme cluster once per frame.
    #[test]
    fn one_very_long_line_is_cut_on_a_char_boundary() {
        let mut long = "é".repeat(4000);
        long.push('\n');
        let out = read("f.txt", long.as_bytes(), "text/plain")
            .unwrap()
            .unwrap();
        assert!(out.lines[0].text.len() <= LINE_BYTES + 4);
        assert!(std::str::from_utf8(out.lines[0].text.as_bytes()).is_ok());
    }

    /// FAILS IF: the file ceiling goes, and the bar becomes a dot.
    #[test]
    fn too_many_lines_are_cut_and_it_says_so() {
        let many = "x\n".repeat(LINE_CEILING + 500);
        let out = read("g.txt", many.as_bytes(), "text/plain")
            .unwrap()
            .unwrap();
        assert_eq!(out.lines.len(), LINE_CEILING);
        assert!(out.cut, "it was cut and did not say so");
    }

    /// FAILS IF: the language stops being taken from the type, and every file
    /// falls back to sniffing — which colours prose.
    #[test]
    fn the_type_names_the_language_and_prose_gets_none() {
        let out = read("h.nix", b"let x = 1;\n", "text/x-nix")
            .unwrap()
            .unwrap();
        assert_eq!(out.language, Some(ronoctua_syntax::Language::Nix));
        assert!(!out.lines[0].runs.is_empty(), "`let` was not coloured");

        let prose = read("i.txt", b"Just some words about a cat.\n", "text/plain")
            .unwrap()
            .unwrap();
        assert_eq!(prose.language, None);
        assert!(prose.lines[0].runs.is_empty(), "prose was coloured");
    }

    /// FAILS IF: highlighting stops carrying between lines, and scrolling into
    /// a docstring shows it uncoloured.
    #[test]
    fn a_comment_across_lines_reaches_the_lines_it_covers() {
        let out = read("j.rs", b"/* one\ntwo\n*/ let x = 1;\n", "text/rust")
            .unwrap()
            .unwrap();
        assert_eq!(out.lines.len(), 3);
        assert!(
            !out.lines[1].runs.is_empty(),
            "the middle of the comment came out bare"
        );
        assert_eq!(out.lines[1].runs[0].role, ronoctua_syntax::Role::Comment);
    }

    /// FAILS IF: the ranges stop being offsets into the FINAL text — which is
    /// what expanding tabs after highlighting would do.
    #[test]
    fn every_range_lands_inside_its_own_line() {
        let out = read(
            "k.rs",
            b"\tlet s = \"a\\nb\";\n\tpub fn f() {}\n",
            "text/rust",
        )
        .unwrap()
        .unwrap();
        for line in &out.lines {
            for r in &line.runs {
                assert!(r.start < r.end && r.end <= line.text.len(), "{r:?}");
                assert!(
                    line.text.is_char_boundary(r.start) && line.text.is_char_boundary(r.end),
                    "{r:?} in {:?}",
                    line.text
                );
            }
        }
        assert!(
            out.lines[0].text.starts_with("    let"),
            "the tab did not open"
        );
    }
}
