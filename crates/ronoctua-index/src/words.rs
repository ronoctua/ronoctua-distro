//! How a text becomes the words the index keeps — and a typed word the words
//! it asks for.
//!
//! # One ruler, both sides
//!
//! The index and the query cut with the same rules here, so a name is found
//! by what a person types. Folding is [`ronoctua_apps::fold`] — the one accent
//! and case folding of the tree (`Crème Brûlée` is `creme brulee` in the
//! launcher, the file manager's sort and here) — and whatever it leaves in
//! another case (Cyrillic, Greek) is lowered after it.
//!
//! # Where a name is cut
//!
//! A file name is not prose: `IMG_20230415.jpg`, `reportFinal-v2.docx`,
//! `XMLParser.java`. It is cut at every character that is not a letter or a
//! digit (the CHUNKS: `IMG`, `20230415`, `jpg`), and inside a chunk at a
//! camelCase edge (`report|Final`, `XML|Parser`) and at a letter–digit edge
//! (`v|2`). A chunk cut that way is ALSO kept whole (`reportfinal`), so typing
//! the word run together finds it too.
//!
//! CJK ideographs and kana carry no spaces between words, so each one is a
//! word of its own: a query for one character finds every name holding it.
//!
//! # And a typed word
//!
//! [`typed`] folds and cuts at punctuation only — a person types `report2026`
//! or `reportfinal` and the whole-chunk words above answer; cutting the typed
//! word at its digits would ask for two words where the name has one chunk.

use std::ops::Range;

/// The longest word kept, in bytes. A longer run of letters is a hash, a key
/// or base64 — nothing a person types — and the term dictionary would pay for
/// it in every segment.
pub const WORD_BYTES: usize = 64;

/// One word of an indexed text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// The folded word.
    pub text: String,
    /// Its position: the chunk's order in the text (a whole chunk and its parts
    /// share one).
    pub position: usize,
    /// Where it came from, in bytes of the text.
    pub offset: Range<usize>,
}

/// Whether `c` is a word of its own (an ideograph, a kana, a Hangul syllable).
#[must_use]
pub fn stands_alone(c: char) -> bool {
    matches!(u32::from(c),
        0x3040..=0x30FF      // Hiragana and Katakana
        | 0x3400..=0x4DBF    // CJK extension A
        | 0x4E00..=0x9FFF    // CJK unified ideographs
        | 0xAC00..=0xD7AF    // Hangul syllables
        | 0xF900..=0xFAFF    // CJK compatibility ideographs
        | 0x20000..=0x2FFFF) // the supplementary ideographic planes
}

/// A word folded the index's way: accents off, every case lowered.
#[must_use]
pub fn folded(word: &str) -> String {
    let once = ronoctua_apps::fold::fold(word);
    if once.is_ascii() {
        once
    } else {
        once.to_lowercase()
    }
}

/// Which class a character is, for the cuts inside a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Lower,
    Upper,
    Digit,
    Other,
}

fn class(c: char) -> Class {
    if c.is_numeric() {
        Class::Digit
    } else if c.is_uppercase() {
        Class::Upper
    } else if c.is_lowercase() {
        Class::Lower
    } else {
        Class::Other
    }
}

/// The byte offsets inside `chunk` where a part begins (always `0` first).
fn cuts(chunk: &str) -> Vec<usize> {
    let chars: Vec<(usize, char)> = chunk.char_indices().collect();
    let mut out = vec![0];
    for k in 1..chars.len() {
        let (at, c) = chars[k];
        let before = class(chars[k - 1].1);
        let now = class(c);
        let next = chars.get(k + 1).map(|&(_, n)| class(n));
        let edge = match (before, now) {
            // `reportFinal`: a capital after a small letter.
            (Class::Lower, Class::Upper) => true,
            // `XMLParser`: the last capital of a run begins the next word when
            // a small letter follows it.
            (Class::Upper, Class::Upper) => next == Some(Class::Lower),
            // `v2`, `2026report`: letters and digits part.
            (Class::Digit, Class::Lower | Class::Upper | Class::Other)
            | (Class::Lower | Class::Upper | Class::Other, Class::Digit) => true,
            _ => false,
        };
        if edge {
            out.push(at);
        }
    }
    out
}

/// Every word of `text` the index keeps, in order.
#[must_use]
pub fn indexed(text: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut position = 0usize;
    let mut start: Option<usize> = None;
    let flush = |from: usize, to: usize, out: &mut Vec<Piece>, position: &mut usize| {
        let chunk = &text[from..to];
        let parts = cuts(chunk);
        let mut kept = 0usize;
        for (k, &at) in parts.iter().enumerate() {
            let end = parts.get(k + 1).copied().unwrap_or(chunk.len());
            let word = folded(&chunk[at..end]);
            if !word.is_empty() && word.len() <= WORD_BYTES {
                out.push(Piece {
                    text: word,
                    position: *position,
                    offset: from + at..from + end,
                });
                kept += 1;
            }
        }
        if parts.len() > 1 {
            let whole = folded(chunk);
            if whole.len() <= WORD_BYTES {
                out.push(Piece {
                    text: whole,
                    position: *position,
                    offset: from..to,
                });
                kept += 1;
            }
        }
        if kept > 0 {
            *position += 1;
        }
    };
    for (at, c) in text.char_indices() {
        if stands_alone(c) {
            if let Some(from) = start.take() {
                flush(from, at, &mut out, &mut position);
            }
            flush(at, at + c.len_utf8(), &mut out, &mut position);
        } else if c.is_alphanumeric() {
            if start.is_none() {
                start = Some(at);
            }
        } else if let Some(from) = start.take() {
            flush(from, at, &mut out, &mut position);
        }
    }
    if let Some(from) = start {
        flush(from, text.len(), &mut out, &mut position);
    }
    out
}

/// The words one TYPED word asks for: folded, cut at punctuation (and around
/// a standalone ideograph), never at case or digits.
#[must_use]
pub fn typed(word: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chunk = String::new();
    let push = |chunk: &mut String, out: &mut Vec<String>| {
        if !chunk.is_empty() {
            let word = folded(chunk);
            if word.len() <= WORD_BYTES {
                out.push(word);
            }
            chunk.clear();
        }
    };
    for c in word.chars() {
        if stands_alone(c) {
            push(&mut chunk, &mut out);
            chunk.push(c);
            push(&mut chunk, &mut out);
        } else if c.is_alphanumeric() {
            chunk.push(c);
        } else {
            push(&mut chunk, &mut out);
        }
    }
    push(&mut chunk, &mut out);
    out
}

/// The folder names above a path, as its `path_terms` (the root `/` has
/// none). The file's own name is not one of them: it is the `name` field.
#[must_use]
pub fn folders_of(path: &std::path::Path) -> Vec<String> {
    path.parent()
        .map(|p| {
            p.components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(text: &str) -> Vec<String> {
        indexed(text).into_iter().map(|p| p.text).collect()
    }

    #[test]
    fn a_name_is_cut_at_punctuation_case_and_digits() {
        assert_eq!(texts("IMG_20230415.jpg"), ["img", "20230415", "jpg"]);
        assert_eq!(
            texts("reportFinal-v2.docx"),
            ["report", "final", "reportfinal", "v", "2", "v2", "docx"]
        );
        assert_eq!(
            texts("XMLParser.java"),
            ["xml", "parser", "xmlparser", "java"]
        );
        assert_eq!(texts("Cargo.toml"), ["cargo", "toml"]);
        assert_eq!(texts("report2026"), ["report", "2026", "report2026"]);
    }

    #[test]
    fn accents_fold_and_every_case_lowers() {
        assert_eq!(texts("Résumé Crème.odt"), ["resume", "creme", "odt"]);
        assert_eq!(texts("Ágil Ñandú"), ["agil", "nandu"]);
        // Folding lowers ASCII only; what it leaves is lowered after it.
        assert_eq!(texts("ПРИВЕТ мир"), ["привет", "мир"]);
    }

    #[test]
    fn a_whole_chunk_and_its_parts_share_a_position() {
        let pieces = indexed("myFile two");
        let at: Vec<(String, usize)> = pieces
            .iter()
            .map(|p| (p.text.clone(), p.position))
            .collect();
        assert_eq!(
            at,
            [
                ("my".to_owned(), 0),
                ("file".to_owned(), 0),
                ("myfile".to_owned(), 0),
                ("two".to_owned(), 1)
            ]
        );
        // Offsets point back into the text, parts and whole alike.
        let text = "myFile two";
        for p in &pieces {
            assert_eq!(folded(&text[p.offset.clone()]), p.text);
        }
    }

    #[test]
    fn ideographs_stand_alone_and_long_runs_are_dropped() {
        assert_eq!(texts("日本語.txt"), ["日", "本", "語", "txt"]);
        let hash = "a".repeat(WORD_BYTES + 1);
        assert_eq!(texts(&format!("{hash} kept")), ["kept"]);
        assert!(texts("").is_empty());
        assert!(texts("--- ... ___").is_empty());
    }

    #[test]
    fn a_typed_word_folds_and_cuts_at_punctuation_only() {
        assert_eq!(typed("Résumé"), ["resume"]);
        assert_eq!(typed("report2026"), ["report2026"]);
        assert_eq!(typed("reportFinal"), ["reportfinal"]);
        assert_eq!(typed("my-file.txt"), ["my", "file", "txt"]);
        assert_eq!(typed("日本"), ["日", "本"]);
        assert!(typed("..").is_empty());
    }

    #[test]
    fn the_folders_are_the_path_above_the_name() {
        assert_eq!(
            folders_of(std::path::Path::new("/home/ana/Documents/note.txt")),
            ["home", "ana", "Documents"]
        );
        assert!(folders_of(std::path::Path::new("/")).is_empty());
    }
}
