//! `ronoctua-tags` — a media file's tags, read or written, as a CHILD.
//!
//! ```text
//! ronoctua-tags sheet <path>
//! ronoctua-tags apply <path> --stamp <json>   (the edits, as JSON, on stdin)
//! ```
//!
//! One [`Answer`] as JSON on stdout, and exit status 0 whenever there is one —
//! a refusal included, because a refusal is a sentence the panel says. A
//! status other than 0 is a program that could not answer at all (a word it
//! does not know, stdin that is not JSON), and it says why on stderr.
//!
//! # Why this exists
//!
//! So lofty never runs inside the footer. The file manager's info panel shows
//! and edits a song's tags, and lofty parses whatever the file holds; a panic
//! or a runaway there, in the process that IS the desktop, would take the
//! desktop down. Here it takes down one short-lived child, and the panel says
//! so. The player's daemon keeps calling the library directly: it is its own
//! process already.
//!
//! The programs this crate runs (ffprobe, exiftool, the Matroska three) are
//! found the way the daemon finds them, from `RONOCTUA_*` in the environment
//! the parent passes on.

use std::io::Read as _;
use std::path::PathBuf;
use std::process::ExitCode;

use ronoctua_tags::{Answer, Edit, Stamp, Tools};

/// The most edits stdin may carry, in bytes: every field's longest value
/// twice over, and nothing a person could type.
const STDIN_MOST_BYTES: u64 = 8 * 1024 * 1024;

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    match answered(&words) {
        Ok(answer) => {
            // ONE WRITE of the whole line, the rule for every JSON line this
            // tree puts on a pipe: a reader that stops for an instant must not
            // be handed half a document.
            let mut line = match serde_json::to_vec(&answer) {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("ronoctua-tags: the answer could not be written: {e}");
                    return ExitCode::FAILURE;
                }
            };
            line.push(b'\n');
            use std::io::Write as _;
            let mut out = std::io::stdout().lock();
            if out.write_all(&line).and_then(|()| out.flush()).is_err() {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("ronoctua-tags: {why}");
            eprintln!("usage: ronoctua-tags sheet <path>");
            eprintln!("       ronoctua-tags apply <path> --stamp <json>  (edits as JSON on stdin)");
            ExitCode::from(2)
        }
    }
}

/// The answer to one command line, or why there is none.
fn answered(words: &[String]) -> Result<Answer, String> {
    let tools = Tools::from_env();
    match words {
        [verb, path] if verb == "sheet" => Ok(
            match ronoctua_tags::read::sheet(&PathBuf::from(path), &tools) {
                Ok(sheet) => Answer::Sheet {
                    sheet: Box::new(sheet),
                },
                Err(why) => Answer::Refused { why },
            },
        ),
        [verb, path, flag, stamp] if verb == "apply" && flag == "--stamp" => {
            let stamp: Stamp = serde_json::from_str(stamp)
                .map_err(|e| format!("`--stamp {stamp}` is not a stamp: {e}"))?;
            let mut raw = Vec::new();
            std::io::stdin()
                .take(STDIN_MOST_BYTES)
                .read_to_end(&mut raw)
                .map_err(|e| format!("stdin could not be read: {e}"))?;
            let edits: Vec<Edit> = serde_json::from_slice(&raw)
                .map_err(|e| format!("stdin is not a list of edits: {e}"))?;
            Ok(
                match ronoctua_tags::write::apply(&PathBuf::from(path), stamp, &edits, &tools) {
                    Ok(stamp) => Answer::Written { stamp },
                    Err(why) => Answer::Refused { why },
                },
            )
        }
        _ => Err(format!(
            "`{}` is not a command this program knows",
            words.join(" ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::answered;
    use ronoctua_tags::{Answer, Why};

    #[test]
    fn a_word_it_does_not_know_is_refused_and_not_guessed_at() {
        for words in [
            vec![],
            vec!["sheet".to_owned()],
            vec!["read".to_owned(), "/x".to_owned()],
            vec!["apply".to_owned(), "/x".to_owned()],
            vec![
                "apply".to_owned(),
                "/x".to_owned(),
                "--stamp".to_owned(),
                "not json".to_owned(),
            ],
        ] {
            assert!(answered(&words).is_err(), "{words:?}");
        }
    }

    #[test]
    fn a_file_that_is_not_there_is_an_answer_and_not_a_failure() {
        let answer =
            answered(&["sheet".to_owned(), "/nowhere/at/all.mp3".to_owned()]).expect("an answer");
        assert_eq!(answer, Answer::Refused { why: Why::Gone });
        // And it crosses as JSON the shell reads back as the same value.
        let line = serde_json::to_string(&answer).unwrap();
        assert_eq!(serde_json::from_str::<Answer>(&line).unwrap(), answer);
    }
}
