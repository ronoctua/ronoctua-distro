//! What Claude Code calls a session — the title it writes into the session's
//! transcript (`{"type":"ai-title","aiTitle":…}`), read from the file's end.
//!
//! # Why the transcript
//!
//! Because it is the one place the title is for an agent outside our
//! terminals: the hook's events carry no title, and only inside our terminal
//! does Claude Code also set it as the terminal's (OSC 0/2). Claude Code
//! writes the record again as the work moves, so the LAST one is the title
//! now, and it is near the end — read from the last [`TAIL_BYTES`] only, a
//! transcript being as long as the session.
//!
//! The title is DATA (invariant 8), like a terminal's title: drawn as it is,
//! on one line, cut to [`TITLE_MOST`] characters.

use std::io::{Read as _, Seek as _};
use std::path::Path;

/// How much of a transcript's end is read for its title.
const TAIL_BYTES: u64 = 256 * 1024;

/// The longest title kept, in characters.
pub const TITLE_MOST: usize = 120;

/// The record that names the session, as Claude Code writes it.
#[derive(serde::Deserialize)]
struct Titled {
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "aiTitle")]
    title: String,
}

/// The session's title as its transcript last said it — `None` for a file
/// that cannot be read, or with no title in its end.
#[must_use]
pub fn title(transcript: &Path) -> Option<String> {
    title_in(&tail(transcript).ok()?)
}

/// The last title in a transcript's text.
#[must_use]
pub fn title_in(text: &[u8]) -> Option<String> {
    text.split(|b| *b == b'\n')
        .rev()
        .filter(|line| contains(line, b"\"ai-title\""))
        .filter_map(|line| serde_json::from_slice::<Titled>(line).ok())
        .find(|t| t.kind == "ai-title")
        .map(|t| one_line(&t.title))
        .filter(|t| !t.is_empty())
}

/// A title on one line — its breaks and runs of space as one space — and
/// no longer than [`TITLE_MOST`].
fn one_line(title: &str) -> String {
    let joined = title.split_whitespace().collect::<Vec<_>>().join(" ");
    match joined.char_indices().nth(TITLE_MOST) {
        Some((at, _)) => format!("{}…", &joined[..at]),
        None => joined,
    }
}

/// Whether `needle` is in `hay`.
fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// The last [`TAIL_BYTES`] of a file, from its first whole line.
fn tail(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let from = file.metadata()?.len().saturating_sub(TAIL_BYTES);
    file.seek(std::io::SeekFrom::Start(from))?;
    let mut bytes = Vec::new();
    file.take(TAIL_BYTES).read_to_end(&mut bytes)?;
    if from > 0 {
        // BEGUN IN THE MIDDLE OF A LINE: that line is not whole.
        let first = bytes
            .iter()
            .position(|b| *b == b'\n')
            .map_or(bytes.len(), |n| n + 1);
        bytes.drain(..first);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The LAST title is the session's now, a record that is not one is
    /// passed over, and a title is kept on one line.
    #[test]
    fn the_last_title_is_the_sessions() {
        let text = concat!(
            r#"{"type":"ai-title","aiTitle":"First idea","sessionId":"s"}"#,
            "\n",
            r#"{"type":"user","message":"about the \"ai-title\""}"#,
            "\n",
            r#"{"type":"ai-title","aiTitle":"Minibuffer,\n modeline   e file manager","sessionId":"s"}"#,
            "\n",
            r#"{"type":"assistant"}"#,
            "\n",
        );
        assert_eq!(
            title_in(text.as_bytes()).as_deref(),
            Some("Minibuffer, modeline e file manager")
        );
        assert_eq!(title_in(b"{\"type\":\"user\"}\n"), None);
        assert_eq!(
            title_in(b"{\"type\":\"ai-title\",\"aiTitle\":\"  \"}"),
            None
        );
    }

    /// A title longer than the room is cut, at a character.
    #[test]
    fn a_long_title_is_cut() {
        let long = "é".repeat(TITLE_MOST + 5);
        let line = format!(r#"{{"type":"ai-title","aiTitle":"{long}"}}"#);
        let cut = title_in(line.as_bytes()).unwrap();
        assert_eq!(cut.chars().count(), TITLE_MOST + 1);
        assert!(cut.ends_with('…'));
    }

    /// Only the file's end is read, and a line cut at its start is not one.
    #[test]
    fn a_long_transcript_is_read_from_its_end() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-llm-transcript-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.jsonl");
        let mut text = String::from(r#"{"type":"ai-title","aiTitle":"too far back"}"#);
        text.push('\n');
        let filler = format!("{{\"type\":\"user\",\"text\":\"{}\"}}\n", "x".repeat(1000));
        while text.len() < usize::try_from(TAIL_BYTES).unwrap() + 4096 {
            text.push_str(&filler);
        }
        std::fs::write(&path, &text).unwrap();
        assert_eq!(title(&path), None, "a title past the tail was read");
        text.push_str(r#"{"type":"ai-title","aiTitle":"near the end"}"#);
        text.push('\n');
        std::fs::write(&path, &text).unwrap();
        assert_eq!(title(&path).as_deref(), Some("near the end"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
