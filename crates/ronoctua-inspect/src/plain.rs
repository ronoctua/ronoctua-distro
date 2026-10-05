//! What a file SAYS, as plain text — for the super index, not the panel.
//!
//! # The same readers, without the panel
//!
//! The preview reads a text file through [`crate::text`] and an office file or
//! an e-book through [`crate::document`]; this asks the SAME two readings
//! ([`crate::text::decoded`], [`crate::document::said`]), so a file is found by
//! exactly the words its preview shows. What it leaves out is the panel's:
//! no highlighting (the index wants words, not colours), no cut at two
//! thousand lines or five hundred bytes a line (a panel's ceilings, not a
//! text's), no tabs opened into spaces.
//!
//! The ceiling is the CALLER's (`searchContentKiB`, `assets/cache.nix`): the
//! index keeps that much text of a file, and says it was cut.
//!
//! # What it does not read
//!
//! A zip that is only a zip (its member names are not what it says), and a
//! picture, a sound or a film even when the database calls it text — an SVG,
//! an EPS, a playlist: their "text" is markup or paths.

use std::path::Path;

use crate::{Refusal, document, text};

/// What a file says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plain {
    /// The text, one line per line or paragraph, at most the ceiling.
    pub text: String,
    /// There was more than the ceiling.
    pub cut: bool,
}

/// Whether this type is read at all.
#[must_use]
pub fn reads(mime: &str) -> bool {
    if ["image/", "audio/", "video/"]
        .iter()
        .any(|p| mime.starts_with(p))
    {
        return false;
    }
    match document::kind_of(mime) {
        Some(document::Kind::Archive) => false,
        Some(_) => true,
        None => text::is_text(mime),
    }
}

/// What `path` says, at most `ceiling` bytes of it.
///
/// `None` — this type is not read ([`reads`]). `Some(Err)` — it is, and here
/// is why there is no text. `Some(Ok)` — the text.
#[must_use]
pub fn of(path: &Path, mime: &str, ceiling: usize) -> Option<Result<Plain, Refusal>> {
    if !reads(mime) {
        return None;
    }
    Some(match document::kind_of(mime) {
        Some(kind) => document::said(path, kind).and_then(|lines| joined(&lines, ceiling)),
        None => text::decoded(path, ceiling as u64).map(|(text, cut)| Plain { text, cut }),
    })
}

/// A document's lines as one text, trailing blanks off, cut on a character
/// boundary at the ceiling.
fn joined(lines: &[String], ceiling: usize) -> Result<Plain, Refusal> {
    let mut text = String::new();
    let mut cut = false;
    for line in lines.iter().map(|l| l.trim_end()).filter(|l| !l.is_empty()) {
        if !text.is_empty() {
            text.push('\n');
        }
        if text.len() + line.len() > ceiling {
            let mut at = ceiling.saturating_sub(text.len());
            while at > 0 && !line.is_char_boundary(at) {
                at -= 1;
            }
            text.push_str(&line[..at]);
            cut = true;
            break;
        }
        text.push_str(line);
    }
    if text.trim().is_empty() {
        return Err(Refusal::Empty);
    }
    Ok(Plain { text, cut })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-plain-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_text_file_is_read_whole_with_no_panel_cuts() {
        let dir = dir("text");
        let long_line = "word ".repeat(300);
        let body = format!("Résumé\n\t{long_line}\n");
        let path = dir.join("note.txt");
        std::fs::write(&path, &body).unwrap();
        let read = of(&path, "text/plain", 1 << 20).unwrap().unwrap();
        // The tab stays a tab and the 1600-byte line stays whole.
        assert_eq!(read.text, body);
        assert!(!read.cut);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_ceiling_cuts_on_a_character_and_says_so() {
        let dir = dir("cut");
        let path = dir.join("long.md");
        // "ção" is five bytes; a ceiling of six lands inside the second "ç".
        std::fs::write(&path, "çãoçãoçãoção").unwrap();
        let read = of(&path, "text/markdown", 6).unwrap().unwrap();
        assert_eq!(read.text, "ção");
        assert!(read.cut);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bytes_that_are_not_text_are_refused_and_pictures_are_not_read() {
        let dir = dir("binary");
        let path = dir.join("blob.txt");
        std::fs::write(&path, b"abc\0def").unwrap();
        assert_eq!(of(&path, "text/plain", 1024), Some(Err(Refusal::NotText)));
        let empty = dir.join("empty.txt");
        std::fs::write(&empty, b"").unwrap();
        assert_eq!(of(&empty, "text/plain", 1024), Some(Err(Refusal::Empty)));
        assert!(!reads("image/svg+xml"));
        assert!(!reads("audio/x-mpegurl"));
        assert!(!reads("application/zip"));
        assert!(!reads("application/octet-stream"));
        assert!(reads("application/vnd.oasis.opendocument.text"));
        assert!(reads("application/json"));
        assert!(of(&path, "image/png", 1024).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rich_text_document_says_its_words_not_its_markup() {
        let dir = dir("rtf");
        let path = dir.join("letter.rtf");
        std::fs::write(
            &path,
            r"{\rtf1\ansi\deff0{\fonttbl{\f0 Arial;}}\f0\pard Dear Ana,\par thank you for the book.\par}",
        )
        .unwrap();
        let read = of(&path, "application/rtf", 1 << 16).unwrap().unwrap();
        assert!(read.text.contains("Dear Ana"), "{:?}", read.text);
        assert!(
            read.text.contains("thank you for the book"),
            "{:?}",
            read.text
        );
        assert!(!read.text.contains("fonttbl"), "{:?}", read.text);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_document_is_joined_line_by_line_under_the_ceiling() {
        let lines = vec![
            "one  ".to_owned(),
            String::new(),
            "two".to_owned(),
            "three".to_owned(),
        ];
        let all = joined(&lines, 100).unwrap();
        assert_eq!(all.text, "one\ntwo\nthree");
        assert!(!all.cut);
        let some = joined(&lines, 9).unwrap();
        assert_eq!(some.text, "one\ntwo\nt");
        assert!(some.cut);
        assert_eq!(joined(&[" ".to_owned()], 9), Err(Refusal::Empty));
    }
}
