//! Syntax highlighting of the copied content.
//!
//! # Why an OWN highlighter
//!
//! The owner's decision, and it rests on two numbers. `syntect` brings a
//! regular expression engine and ~1 MB of syntax definitions into
//! the process that has to draw in one frame (§5.2); `tree-sitter` asks for
//! a compiled grammar — that is, a Nix dependency — **per language**.
//!
//! What gets pasted onto the clipboard is a handful of things: a shell
//! command, a snippet of `.nix`, a piece of JSON, a line of Rust. For
//! that, a tokeniser of some two hundred lines gets enough of it right and costs
//! zero dependencies.
//!
//! # What it is NOT
//!
//! It is not a parser. It knows no grammar, validates nothing and cannot
//! say whether the code is correct. It recognises **four things** —
//! comment, quoted text, number and reserved word — because those are the
//! four that stop a block of code from being a grey wall.
//!
//! Getting it wrong here is cheap: the worst case is a word coloured as if it were
//! something else, in a preview that exists so one can recognise what one copied.
//!
//! # Why it returns RANGES, and not pixels
//!
//! The renderer already draws runs with colour. Highlighting is only deciding where each colour
//! begins and ends — no new machinery, only the tokeniser.

/// The role of a piece of text, which becomes a colour in the host.
///
/// An `enum`, and not a colour token: the clipboard buffer does **not** receive
/// a token value (it is the same boundary as invariant 11), and what translates
/// role into colour is whatever draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Ordinary text.
    Normal,
    /// A comment.
    Comment,
    /// Quoted text.
    Text,
    /// A number.
    Number,
    /// A reserved word of the language.
    Word,
}

/// A contiguous piece with the same role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range {
    /// Where it starts, in bytes.
    pub start: usize,
    /// Where it ends, in bytes (exclusive).
    pub end: usize,
    /// What it is.
    pub role: Role,
}

/// The languages the highlighter knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// Shell, and whatever looks like it.
    Shell,
    /// Nix.
    Nix,
    /// Rust.
    Rust,
    /// JSON.
    Json,
    /// TOML and the like.
    Toml,
}

impl Language {
    /// The reserved words.
    const fn words(self) -> &'static [&'static str] {
        match self {
            Self::Shell => &[
                "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
                "function", "return", "export", "local", "echo", "cd", "set", "sudo",
            ],
            Self::Nix => &[
                "let", "in", "with", "rec", "inherit", "import", "if", "then", "else", "assert",
                "or", "true", "false", "null",
            ],
            Self::Rust => &[
                "fn", "let", "mut", "pub", "use", "mod", "impl", "struct", "enum", "trait",
                "match", "if", "else", "for", "while", "loop", "return", "const", "static", "self",
                "crate", "async", "await", "move", "ref", "dyn", "where", "true", "false",
            ],
            Self::Json => &["true", "false", "null"],
            Self::Toml => &["true", "false"],
        }
    }

    /// How a comment starts in this language.
    const fn comment(self) -> &'static str {
        match self {
            Self::Shell | Self::Nix | Self::Toml => "#",
            Self::Rust => "//",
            // JSON has no comment. A marker that never occurs avoids an
            // `Option` every caller would have to unwrap.
            Self::Json => "\0",
        }
    }
}

/// Guesses the language from what the text looks like.
///
/// `None` when nothing points at one of them — and then there is **no highlighting**, which is the
/// right thing: colouring prose as if it were code is worse than not colouring.
#[must_use]
pub fn guess(text: &str) -> Option<Language> {
    let t = text.trim_start();
    if t.is_empty() {
        return None;
    }
    // JSON is the cheapest to recognise, and the most common thing to paste from an
    // API response.
    if (t.starts_with('{') || t.starts_with('[')) && t.contains('"') && t.contains(':') {
        return Some(Language::Json);
    }
    if t.starts_with("#!") {
        return Some(Language::Shell);
    }
    // Then, by a STRONG MARKER — something that almost only appears in that language.
    // Counting common words would have "if" deciding between four candidates.
    for (mark, l) in [
        ("fn ", Language::Rust),
        ("impl ", Language::Rust),
        ("let mut ", Language::Rust),
        ("pub fn", Language::Rust),
        ("::", Language::Rust),
        ("nixpkgs", Language::Nix),
        ("mkDerivation", Language::Nix),
        ("buildInputs", Language::Nix),
        ("with pkgs", Language::Nix),
        ("nix-", Language::Nix),
        ("sudo ", Language::Shell),
        ("$(", Language::Shell),
        ("&&", Language::Shell),
        ("| grep", Language::Shell),
    ] {
        if text.contains(mark) {
            return Some(l);
        }
    }
    // TOML last: `key = value` at the start of a line is weak, and that is why
    // it only decides when nothing else has.
    if text
        .lines()
        .any(|l| l.starts_with('[') && l.trim_end().ends_with(']'))
    {
        return Some(Language::Toml);
    }
    None
}

/// The coloured ranges of a text.
///
/// It returns **only** what is not ordinary: the rest of the text is ordinary by omission,
/// and emitting a range per common word would double the list without saying anything.
#[must_use]
pub fn ranges(text: &str, language: Language) -> Vec<Range> {
    let bytes = text.as_bytes();
    let comment = language.comment().as_bytes();
    let mut out = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i];

        // A COMMENT runs to the end of the line. It beats everything: `# "quotes"` is
        // a comment in its entirety, and not a comment plus text.
        if bytes[i..].starts_with(comment) {
            let end = text[i..].find('\n').map_or(bytes.len(), |n| i + n);
            out.push(Range {
                start: i,
                end,
                role: Role::Comment,
            });
            i = end;
            continue;
        }

        // QUOTED TEXT, respecting the backslash.
        if c == b'"' || c == b'\'' {
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if bytes[j] == c {
                    j += 1;
                    break;
                }
                j += 1;
            }
            let end = j.min(bytes.len());
            out.push(Range {
                start: i,
                end,
                role: Role::Text,
            });
            i = end;
            continue;
        }

        // A NUMBER, only when it starts a word: the `2` in `utf8` is not a number.
        if c.is_ascii_digit() && (i == 0 || !is_word_char(bytes[i - 1])) {
            let mut j = i;
            while j < bytes.len()
                && (bytes[j].is_ascii_hexdigit() || matches!(bytes[j], b'.' | b'x' | b'_'))
            {
                j += 1;
            }
            out.push(Range {
                start: i,
                end: j,
                role: Role::Number,
            });
            i = j;
            continue;
        }

        // A WORD: it takes the whole thing and compares against the list. Comparing by prefix
        // would make `iffy` come out as a half-coloured `if`.
        if is_word_char(c) {
            let mut j = i;
            while j < bytes.len() && is_word_char(bytes[j]) {
                j += 1;
            }
            if language.words().contains(&&text[i..j]) {
                out.push(Range {
                    start: i,
                    end: j,
                    role: Role::Word,
                });
            }
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

/// Is it part of a word?
const fn is_word_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

#[cfg(test)]
mod tests {
    use super::{Language, Role, guess, ranges};

    fn roles(text: &str, l: Language) -> Vec<(&str, Role)> {
        ranges(text, l)
            .into_iter()
            .map(|f| (&text[f.start..f.end], f.role))
            .collect()
    }

    #[test]
    fn recognises_the_four_things() {
        let t = r#"let x = 42; // nota"#;
        let p = roles(t, Language::Rust);
        assert!(p.contains(&("let", Role::Word)), "{p:?}");
        assert!(p.contains(&("42", Role::Number)), "{p:?}");
        assert!(p.contains(&("// nota", Role::Comment)), "{p:?}");
    }

    #[test]
    fn the_comment_beats_the_quotes() {
        // `# "this"` is a comment IN ITS ENTIRETY. Without this order, the highlighting would open
        // a string that never closes and would paint the rest of the block a single colour.
        let t = "echo ok  # \"I am not text\"";
        let p = roles(t, Language::Shell);
        assert!(
            p.iter()
                .any(|(s, pa)| *pa == Role::Comment && s.contains("I am not text")),
            "{p:?}"
        );
        assert!(!p.iter().any(|(_, pa)| *pa == Role::Text), "{p:?}");
    }

    #[test]
    fn the_backslash_does_not_close_the_text() {
        let t = r#"x = "a\"b" ;"#;
        let p = roles(t, Language::Rust);
        assert!(p.contains(&(r#""a\"b""#, Role::Text)), "{p:?}");
    }

    #[test]
    fn a_similar_word_is_not_reserved() {
        // Comparing by PREFIX would make `iffy` come out as a half-coloured `if` —
        // and the highlighting would point at a keyword that is not there.
        let p = roles("iffy letargia", Language::Rust);
        assert!(p.is_empty(), "{p:?}");
    }

    #[test]
    fn a_number_inside_a_word_is_not_a_number() {
        let p = roles("utf8 is a name", Language::Rust);
        assert!(!p.iter().any(|(_, pa)| *pa == Role::Number), "{p:?}");
    }

    #[test]
    fn the_ranges_do_not_overlap_and_are_in_order() {
        // The host draws in order and cuts by byte: ranges out of order or
        // overlapping would give text written twice in the same place.
        let t = "let a = \"x\"; # fim\nfn b() {}";
        for l in [Language::Rust, Language::Shell, Language::Nix] {
            let fs = ranges(t, l);
            for pair in fs.windows(2) {
                assert!(pair[0].end <= pair[1].start, "{l:?}: {fs:?}");
            }
            for f in &fs {
                assert!(f.start < f.end && f.end <= t.len(), "{l:?}: {f:?}");
                // AND EVERY RANGE LANDS ON A CHARACTER BOUNDARY: cutting UTF-8
                // in half would become a panic at slicing time.
                assert!(t.is_char_boundary(f.start) && t.is_char_boundary(f.end));
            }
        }
    }

    #[test]
    fn guesses_what_can_be_guessed() {
        assert_eq!(guess(r#"{"a": 1}"#), Some(Language::Json));
        assert_eq!(guess("#!/bin/sh\nls"), Some(Language::Shell));
        assert_eq!(guess("pub fn x() {}"), Some(Language::Rust));
        assert_eq!(guess("with pkgs; [ hello ]"), Some(Language::Nix));
        assert_eq!(guess("[section]\nkey = 1"), Some(Language::Toml));
    }

    #[test]
    fn prose_gets_no_highlight() {
        // COLOURING PROSE IS WORSE THAN NOT COLOURING: the reader starts looking for
        // meaning in a colour that means nothing.
        assert_eq!(guess("Good morning, how are you doing?"), None);
        assert_eq!(guess("https://example.org/pagina"), None);
        assert_eq!(guess(""), None);
    }

    #[test]
    fn unclosed_text_does_not_blow_up() {
        // A snippet pasted half-finished is the normal case, not the exception.
        let t = "echo \"sem fim";
        let fs = ranges(t, Language::Shell);
        assert!(fs.iter().all(|f| f.end <= t.len()));
    }
}
