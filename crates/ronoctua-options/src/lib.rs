//! What a declared setting is, and the arithmetic of turning one.
//!
//! # Why this is a crate
//!
//! `nix/scripts/gen-options.py` writes four tables — Syncthing's, Tor's,
//! ClipCascade's and Atuin's — and each of them needs the same three answers:
//! which word a ring turns to next, what a number's arrow step is over its
//! range, and how a value reads on a screen. Written per domain that is the
//! same fifty lines four times, which is the defect this repository keeps
//! naming.
//!
//! # What is NOT here, and why
//!
//! **`Setting` itself.** The generated table writes `scope: Scope::Bridges`,
//! and `Scope` is the one thing that genuinely differs per domain — so each
//! crate declares its own eight-field struct and this one owns [`Kind`]. A
//! generic `Setting<S>` would have made the generated `&[Setting]` not
//! compile, and a `Box<dyn>` would have made a static table an allocation.
//!
//! **The marshalling.** Tor writes torrc lines, Atuin writes TOML, ClipCascade
//! writes JSON. A value is a `&str` here and each domain converts at its own
//! edge, which is where it already has to know the spelling of `true`.
//!
//! **Which kinds turn and which are typed.** A domain decides: Syncthing's
//! amount is `{ value, unit }` and its arrows move the number
//! (`ronoctua_syncthing::options::Setting::turns`), where the other three
//! type a size ([`Kind::is_typed`]). Syncthing reads everything else from
//! here — the kind, the ring's turn, the number's nudge and a word's key — and
//! its table had its own copy of all four until the migration.

/// How `true` and `false` are spelled in one domain's configuration file.
///
/// A parameter and not a constant because the three domains disagree: a torrc
/// switch is `1`, a TOML one is `true`, and reading either as the other is a
/// setting that silently does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spelling {
    /// What on looks like.
    pub yes: &'static str,
    /// What off looks like.
    pub no: &'static str,
}

impl Spelling {
    /// `1` and `0` — torrc, and anything else that grew out of C.
    pub const DIGITS: Self = Self { yes: "1", no: "0" };
    /// `true` and `false` — TOML, JSON, and most things since.
    pub const WORDS: Self = Self {
        yes: "true",
        no: "false",
    };

    /// Whether a value means yes.
    ///
    /// GENEROUS ON THE WAY IN, exact on the way out. A file a person edited by
    /// hand may say `1` where the domain writes `true`, and refusing to read
    /// it would draw a switch that is off beside a service that is on.
    #[must_use]
    pub fn reads_yes(self, value: &str) -> bool {
        let value = value.trim();
        value.eq_ignore_ascii_case(self.yes)
            || value.eq_ignore_ascii_case("1")
            || value.eq_ignore_ascii_case("true")
            || value.eq_ignore_ascii_case("yes")
            || value.eq_ignore_ascii_case("on")
    }

    /// The text for a boolean.
    #[must_use]
    pub const fn of(self, on: bool) -> &'static str {
        if on { self.yes } else { self.no }
    }
}

/// What kind of thing a setting is, which is what decides how a key turns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// On or off.
    Switch,
    /// A whole number in a range, with a unit to say what of.
    Number {
        /// The smallest the program accepts.
        least: i64,
        /// The largest worth offering.
        most: i64,
        /// The text key naming what the number counts.
        unit: Option<&'static str>,
    },
    /// A line of text, typed in the minibuffer.
    Text,
    /// One of these words, in the order the ring turns.
    Ring(&'static [&'static str]),
    /// Several lines of text — one config line each.
    ///
    /// `Bridge` is the case that made this exist: bridges arrive two or three
    /// at a time, and torrc takes one line each. A `Text` would have made "one
    /// bridge" the only expressible number.
    List,
    /// A size in bytes, written the way a person says it.
    Amount,
}

impl Kind {
    /// The id, for a message that has to name the kind.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Switch => "switch",
            Self::Number { .. } => "number",
            Self::Text => "text",
            Self::Ring(_) => "ring",
            Self::List => "list",
            Self::Amount => "amount",
        }
    }

    /// Can this be typed into, rather than turned?
    ///
    /// What a door asks before deciding whether `RET` turns a ring or opens
    /// the minibuffer. The one place the two would be confused is the place a
    /// person would press a key and watch nothing happen.
    #[must_use]
    pub const fn is_typed(self) -> bool {
        matches!(self, Self::Text | Self::List | Self::Amount)
    }
}

/// The step an arrow takes over a range.
///
/// A hundred steps across the whole range, rounded to something a person would
/// have chosen — 1, 5, 10, 50, 100 and so on — and never zero. A single-unit
/// step over a range of six hundred thousand is a key nobody can reach the end
/// of; a step that divides the range evenly is a number that never lands on a
/// round one.
#[must_use]
pub fn step_of(least: i64, most: i64) -> i64 {
    let span = most.saturating_sub(least).max(1);
    let rough = (span / 100).max(1);
    // THE NEXT ROUND NUMBER AT OR ABOVE `rough`, out of the 1-2-5 ladder every
    // scale in this tree uses.
    let mut scale = 1i64;
    loop {
        for factor in [1, 2, 5] {
            let candidate = scale.saturating_mul(factor);
            if candidate >= rough {
                return candidate;
            }
        }
        let Some(next) = scale.checked_mul(10) else {
            return rough;
        };
        scale = next;
    }
}

/// The text key for one word of a ring.
///
/// It MUST agree with `word_of` in `nix/scripts/gen-options.py`, which is what
/// writes the keys both languages owe. The two are compared by
/// `option-words-agree`: a Rust that asked for a key the generator never
/// emitted would draw the key itself, in brackets, and nothing would say why.
#[must_use]
pub fn word_key(value: &str) -> String {
    if value.is_empty() {
        return "none".to_owned();
    }
    let mut out = String::with_capacity(value.len() + 4);
    for (i, c) in value.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_whitespace() || c == '_' {
            out.push('-');
        } else {
            out.push(c);
        }
    }
    // COLLAPSE AND TRIM, so `a  b` and `a_ b` do not become two different keys
    // for one word. The generator's regex does the same thing.
    let mut collapsed = String::with_capacity(out.len());
    let mut last_dash = false;
    for c in out.chars() {
        if c == '-' {
            if !last_dash {
                collapsed.push(c);
            }
            last_dash = true;
        } else {
            collapsed.push(c);
            last_dash = false;
        }
    }
    collapsed.trim_matches('-').to_owned()
}

/// The next word of a ring, turning forwards or back.
///
/// A value that is not in the ring lands on the FIRST word, and that is not a
/// fallback: a configuration file edited by hand can hold anything, and the
/// alternative is a key that appears to do nothing because the cursor is on a
/// word the ring does not contain.
#[must_use]
pub fn turn<'a>(ring: &'a [&'a str], now: &str, forwards: bool) -> &'a str {
    if ring.is_empty() {
        return "";
    }
    let at = ring.iter().position(|w| *w == now);
    let Some(at) = at else { return ring[0] };
    let last = ring.len() - 1;
    let next = if forwards {
        if at == last { 0 } else { at + 1 }
    } else if at == 0 {
        last
    } else {
        at - 1
    };
    ring[next]
}

/// A number moved by one arrow press, kept inside its range.
///
/// It CLAMPS rather than wrapping. A wrapped number is the arrow key that
/// turned a ten-second timeout into a week because somebody held it one press
/// too long, and there is no reading of "more" that means "the smallest".
#[must_use]
pub fn nudge(now: i64, least: i64, most: i64, forwards: bool) -> i64 {
    let step = step_of(least, most);
    let moved = if forwards {
        now.saturating_add(step)
    } else {
        now.saturating_sub(step)
    };
    moved.clamp(least, most)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ring_turns_both_ways_and_closes() {
        let ring: &[&str] = &["auto", "1", "0"];
        assert_eq!(turn(ring, "auto", true), "1");
        assert_eq!(turn(ring, "0", true), "auto", "the ring did not close");
        assert_eq!(turn(ring, "auto", false), "0", "it did not close backwards");
        // A VALUE THE RING DOES NOT HOLD lands on the first word. A file
        // edited by hand can say anything, and the alternative is a key that
        // appears to do nothing.
        assert_eq!(turn(ring, "nonsense", true), "auto");
        assert_eq!(turn(&[], "x", true), "");
    }

    #[test]
    fn a_number_clamps_and_never_wraps() {
        // THE DEFECT THIS PREVENTS: a wrapped number is the arrow key that
        // turned a ten-second timeout into a week because somebody held it one
        // press too long.
        assert_eq!(nudge(10, 10, 600, false), 10);
        assert_eq!(nudge(600, 10, 600, true), 600);
        let step = step_of(10, 600);
        assert_eq!(nudge(100, 10, 600, true), 100 + step);
        assert_eq!(nudge(100, 10, 600, false), 100 - step);
    }

    #[test]
    fn the_step_is_a_number_a_person_would_have_chosen() {
        // A single unit over six hundred thousand is an arrow nobody can reach
        // the end of; a step that divides the range exactly is a value that
        // never lands on a round number.
        assert_eq!(step_of(0, 100), 1);
        assert_eq!(step_of(0, 1000), 10);
        assert_eq!(step_of(10, 86400), 1000);
        assert_eq!(step_of(0, 604800), 10000);
        // AND NEVER ZERO, whatever the range is.
        for (a, b) in [(0, 1), (5, 5), (-3, 0), (0, 3)] {
            assert!(step_of(a, b) >= 1, "step_of({a}, {b}) was not positive");
        }
    }

    #[test]
    fn a_word_key_is_a_key() {
        // IT MUST AGREE WITH `word_of` IN THE GENERATOR, which is what writes
        // the keys both languages owe. A Rust that asked for a key the
        // generator never emitted would draw the key itself, in brackets.
        assert_eq!(word_key(""), "none");
        assert_eq!(word_key("sendreceive"), "sendreceive");
        assert_eq!(word_key("smallestFirst"), "smallest-first");
        assert_eq!(word_key("copy_file_range"), "copy-file-range");
        assert_eq!(word_key("notice syslog"), "notice-syslog");
        assert_eq!(word_key("meek_lite"), "meek-lite");
        // AND WHAT COMES OUT IS ALWAYS SOMETHING A `.ftl` COULD CARRY.
        for word in ["a  b", "_x_", "A B", "auto"] {
            let key = word_key(word);
            assert!(!key.is_empty(), "`{word}` gave an empty key");
            assert!(
                key.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "`{word}` gave `{key}`, which is not a text key"
            );
            assert!(!key.starts_with('-') && !key.ends_with('-'), "{key}");
            assert!(!key.contains("--"), "{key}");
        }
    }

    #[test]
    fn a_switch_is_read_generously_and_written_exactly() {
        // A FILE A PERSON EDITED BY HAND may say `1` where the domain writes
        // `true`, and refusing to read it would draw a switch that is off
        // beside a service that is on.
        for said in ["1", "true", "TRUE", "yes", "on"] {
            assert!(Spelling::DIGITS.reads_yes(said), "{said}");
            assert!(Spelling::WORDS.reads_yes(said), "{said}");
        }
        for said in ["0", "false", "no", "off", ""] {
            assert!(!Spelling::DIGITS.reads_yes(said), "{said}");
        }
        // ON THE WAY OUT, each domain's own spelling and no other.
        assert_eq!(Spelling::DIGITS.of(true), "1");
        assert_eq!(Spelling::WORDS.of(true), "true");
        assert_eq!(Spelling::DIGITS.of(false), "0");
    }

    #[test]
    fn what_can_be_typed_into_is_not_what_can_be_turned() {
        // The one place these would be confused is the place a person presses
        // a key and watches nothing happen.
        assert!(Kind::Text.is_typed());
        assert!(Kind::List.is_typed());
        assert!(Kind::Amount.is_typed());
        assert!(!Kind::Switch.is_typed());
        assert!(!Kind::Ring(&["a", "b"]).is_typed());
        assert!(
            !Kind::Number {
                least: 0,
                most: 1,
                unit: None
            }
            .is_typed()
        );
    }
}
