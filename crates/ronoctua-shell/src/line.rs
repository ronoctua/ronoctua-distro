//! A QUESTION ON ONE LINE — what is typed, in either door.
//!
//! # Why it is its own piece
//!
//! Because both doors type. The footer asked on its minibuffer's line and a
//! window had nothing to ask on, so every window that needed a name, an
//! address, a path or a password sent the person to the footer — and a
//! window's own draft could not be answered anywhere. The owner's rule is that
//! the two doors do the same things; this is the half of the minibuffer that
//! asks, taken out of it so a window holds the same one (`door::Ui::ask`) and
//! the footer's minibuffer holds it too. One line, one set of keys, one mask.
//!
//! # What it is not
//!
//! It is not the `M-x` finder: no list, no filter, no candidate. It holds what
//! is typed, and answers with it. There is no cursor inside the text, as there
//! never was in the minibuffer: what is typed goes at the end.

use ronoctua_keymap::{Key, Sym};

/// The keys a question teaches on a window's header, clickable
/// (invariant 19): `Esc` lets it go, `RET` answers.
///
/// The words are the ids read as text keys (`line-cancel`, `line-answer`),
/// and the sentence under the pointer is `about-<id>` — the rule every hint
/// table keeps (`every-hint-key-explains-itself`).
pub const OF_THE_LINE: &[(&str, &str)] = &[
    // `Esc` first, as every line of the system teaches its way back first.
    ("ESC", "line-cancel"),
    ("RET", "line-answer"),
];

/// A question being typed.
///
/// `Debug` is written by hand: a SECRET never reaches a log through it.
#[derive(Clone, PartialEq, Eq)]
pub struct Line {
    /// The command that asks — and that answers: the question is its
    /// `ask-<id>` key, and the answer goes back under this id.
    pub id: &'static str,
    /// Whether what is typed is hidden — a password, a PIN, a key.
    pub secret: bool,
    /// What has been typed.
    pub text: String,
}

impl std::fmt::Debug for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Line")
            .field("id", &self.id)
            .field("secret", &self.secret)
            .field("text", &self.shown())
            .finish()
    }
}

/// What a key did to the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Nothing changed; no frame is owed.
    Nothing,
    /// The text changed.
    Changed,
    /// `C-v` or `C-y`: the door reads the clipboard and hands it to
    /// [`Line::paste`].
    WantsPaste,
    /// `RET`: the answer, under the id that asked.
    Answer(&'static str, String),
    /// `Esc` or `C-g`: the question is let go, unanswered.
    Cancel,
}

impl Line {
    /// A question for `id`, the line already holding `initial` — taken as ONE
    /// line (see [`one_line`]).
    ///
    /// Changing a name or an address starts from what it IS: a web app called
    /// "Proton Mail" renamed to "Mail" is four `DEL`s, and asking with an
    /// empty line would make the person type the address back from memory to
    /// fix one letter of it.
    #[must_use]
    pub fn new(id: &'static str, secret: bool, initial: Option<&str>) -> Self {
        Self {
            id,
            secret,
            text: initial.map(one_line).unwrap_or_default(),
        }
    }

    /// What is typed, as the screen shows it: the text, or ONE DOT PER CELL
    /// of it when it is a secret.
    ///
    /// One per cell, and not a fixed number: the length that appears is what
    /// was typed, otherwise one cannot see that the key took. By cluster,
    /// because the dots stand in for the typing and an accented letter is one
    /// thing typed and one cell wide.
    #[must_use]
    pub fn shown(&self) -> String {
        if self.secret {
            "•".repeat(usize::from(ronoctua_render::layout::width(&self.text)))
        } else {
            self.text.clone()
        }
    }

    /// The question, in the person's own language.
    ///
    /// THE COMMAND'S ID IS NOT A SENTENCE: it is `ask-<id>` when there is one
    /// (a real question, "Password for this network"), and the command's own
    /// label otherwise, which every command has in both languages by
    /// construction.
    #[must_use]
    pub fn question(&self, texts: &ronoctua_i18n::Texts) -> String {
        let said = |key: &str| {
            let text = texts.t(key);
            // A KEY NOBODY WROTE resolves to `[key]` (`Texts::t_marked`), and
            // that is how this asks whether it exists at all.
            (!text.starts_with('[')).then_some(text)
        };
        said(&format!("ask-{}", self.id))
            .or_else(|| said(&format!("cmd-{}", self.id)))
            .unwrap_or_else(|| self.id.to_owned())
    }

    /// The whole line as it is drawn: the question, and the text or its mask.
    #[must_use]
    pub fn drawn(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("{}: {}", self.question(texts), self.shown())
    }

    /// The same, with no localiser — the command's id for the question. The
    /// shape the tests read.
    #[must_use]
    pub fn prompt(&self) -> String {
        format!("{}: {}", self.id, self.shown())
    }

    /// One key.
    ///
    /// `C-v`/`C-y` paste (Emacs's yank, which is the notation this line is
    /// written in); `Esc` and `C-g` cancel; `RET` answers; `DEL` takes one
    /// cluster; a letter and the space are typed. Every other key does
    /// nothing — a question is not a list, and an arrow has nowhere to go.
    pub fn key(&mut self, key: Key) -> Step {
        if key.mods.ctrl {
            return match key.sym {
                Sym::Char('v' | 'y') => Step::WantsPaste,
                Sym::Char('g') => Step::Cancel,
                _ => Step::Nothing,
            };
        }
        match key.sym {
            Sym::Esc => Step::Cancel,
            Sym::Enter => Step::Answer(self.id, self.text.clone()),
            Sym::Backspace => {
                if pop_cluster(&mut self.text) {
                    Step::Changed
                } else {
                    Step::Nothing
                }
            }
            Sym::Space => {
                self.text.push(' ');
                Step::Changed
            }
            // A CONTROL CHARACTER IS NOT TYPED. `Delete` arrives as U+007F
            // from `key_char`, and typed it is a character nobody can see —
            // in a path, a file that does not exist.
            Sym::Char(c) if c.is_control() => Step::Nothing,
            Sym::Char(c) => {
                self.text.push(c);
                Step::Changed
            }
            Sym::Tab
            | Sym::Left
            | Sym::Right
            | Sym::Up
            | Sym::Down
            | Sym::PageUp
            | Sym::PageDown
            | Sym::Home
            | Sym::End => Step::Nothing,
        }
    }

    /// What was copied, typed in at the end of the line — as ONE line.
    pub fn paste(&mut self, text: &str) -> Step {
        let line = one_line(text);
        if line.is_empty() {
            return Step::Nothing;
        }
        self.text.push_str(&line);
        Step::Changed
    }
}

/// Text as ONE line takes it: the ends trimmed, and every control character —
/// a line break above all — a space, so nothing typed in is a character
/// nobody can see. A path copied with its trailing newline would otherwise
/// name a file that does not exist.
#[must_use]
pub fn one_line(text: &str) -> String {
    text.trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Takes the last CLUSTER off `text`, and says whether there was one.
///
/// One cluster, not one `char`: `String::pop` takes a code point, and a
/// cluster is up to six of them — backspacing an emoji left the halves of it
/// behind, and a secret's mask (which counts cells) stopped agreeing with the
/// text behind it.
pub fn pop_cluster(text: &mut String) -> bool {
    let Some(last) = ronoctua_render::layout::cells_of(text).last() else {
        return false;
    };
    let keep = text.len() - last.len();
    text.truncate(keep);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_keymap::Mods;

    fn plain(sym: Sym) -> Key {
        Key {
            mods: Mods::default(),
            sym,
        }
    }

    fn ctrl(c: char) -> Key {
        Key {
            mods: Mods {
                ctrl: true,
                ..Mods::default()
            },
            sym: Sym::Char(c),
        }
    }

    fn typed(line: &mut Line, s: &str) {
        for c in s.chars() {
            line.key(plain(if c == ' ' { Sym::Space } else { Sym::Char(c) }));
        }
    }

    #[test]
    fn a_line_answers_what_was_typed_under_the_id_that_asked() {
        let mut l = Line::new("software-web-new", false, None);
        typed(&mut l, "example.org/a b");
        assert_eq!(l.text, "example.org/a b");
        assert_eq!(
            l.key(plain(Sym::Enter)),
            Step::Answer("software-web-new", "example.org/a b".into())
        );
    }

    #[test]
    fn a_line_starts_from_what_is_there_as_one_line() {
        let mut l = Line::new("software-web-name", false, Some("  Proton\nMail\n"));
        assert_eq!(l.text, "Proton Mail");
        for _ in 0.."Mail".len() {
            assert_eq!(l.key(plain(Sym::Backspace)), Step::Changed);
        }
        typed(&mut l, "Box");
        assert_eq!(
            l.key(plain(Sym::Enter)),
            Step::Answer("software-web-name", "Proton Box".into())
        );
    }

    #[test]
    fn esc_and_c_g_cancel_and_c_v_c_y_ask_for_the_clipboard() {
        let mut l = Line::new("x", false, None);
        assert_eq!(l.key(plain(Sym::Esc)), Step::Cancel);
        assert_eq!(l.key(ctrl('g')), Step::Cancel);
        assert_eq!(l.key(ctrl('v')), Step::WantsPaste);
        assert_eq!(l.key(ctrl('y')), Step::WantsPaste);
        // AND NO OTHER CHORD IS TYPED: `C-a` is not an `a`.
        assert_eq!(l.key(ctrl('a')), Step::Nothing);
        assert_eq!(l.text, "");
    }

    #[test]
    fn a_paste_is_one_line_and_an_empty_one_is_nothing() {
        let mut l = Line::new("x", false, Some("/music/"));
        assert_eq!(l.paste("a b.flac\n"), Step::Changed);
        assert_eq!(l.text, "/music/a b.flac");
        assert_eq!(l.paste("  \n\t"), Step::Nothing);
        assert_eq!(l.paste("one\ntwo"), Step::Changed);
        assert_eq!(l.text, "/music/a b.flacone two");
    }

    /// THE DEFECT: `Delete` typed U+007F into a search, a character nobody
    /// can see. A control character is never typed.
    #[test]
    fn a_control_character_is_not_typed() {
        let mut l = Line::new("x", false, None);
        assert_eq!(l.key(plain(Sym::Char('\u{7f}'))), Step::Nothing);
        assert_eq!(l.key(plain(Sym::Char('\u{1b}'))), Step::Nothing);
        assert_eq!(l.text, "");
    }

    #[test]
    fn backspace_takes_a_whole_cluster() {
        let mut l = Line::new("x", false, None);
        typed(
            &mut l,
            "a\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F468}",
        );
        assert_eq!(l.key(plain(Sym::Backspace)), Step::Changed);
        assert_eq!(l.text, "a");
        assert_eq!(l.key(plain(Sym::Backspace)), Step::Changed);
        assert_eq!(l.key(plain(Sym::Backspace)), Step::Nothing);
    }

    /// A SECRET IS MASKED on the screen and in the log, one dot per cell,
    /// and its answer is still the text.
    #[test]
    fn a_secret_is_masked_everywhere_but_in_the_answer() {
        let mut l = Line::new("network-connect", true, None);
        typed(&mut l, "s3nh4-ç");
        assert_eq!(l.shown(), "•".repeat(7));
        assert!(!l.prompt().contains("s3nh4"), "{}", l.prompt());
        let logged = format!("{l:?}");
        assert!(
            !logged.contains("s3nh4"),
            "the secret reached Debug: {logged}"
        );
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        assert!(!l.drawn(&texts).contains("s3nh4"));
        assert_eq!(
            l.key(plain(Sym::Enter)),
            Step::Answer("network-connect", "s3nh4-ç".into())
        );
    }

    /// THE QUESTION IS A SENTENCE: `ask-<id>` when it exists, the command's
    /// label otherwise — never the id, in any language.
    #[test]
    fn the_question_is_the_commands_own_words() {
        for language in ronoctua_i18n::LANGUAGES {
            let texts = ronoctua_i18n::Texts::fresh(language).expect(language);
            let l = Line::new("software-web-new", false, None);
            assert_eq!(l.question(&texts), texts.t("ask-software-web-new"));
            assert!(l.drawn(&texts).starts_with(&l.question(&texts)));
        }
    }
}
