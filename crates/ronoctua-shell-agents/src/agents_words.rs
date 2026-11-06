//! The LLM accounts' words: which text key each closed set of
//! `ronoctua-llm` is drawn with — the refusals, the agents, the status line's
//! three states, what a running agent is doing — and the few
//! numbers the view writes in the language's own marks (a per cent, a count
//! of tokens).
//!
//! # Why a file of its own
//!
//! Because both doors draw these (`agents.rs` builds the rows either door
//! shows, and both say the refusals on their own line), and every set here is
//! CLOSED in the crate: a `match` over it is a compile error when the crate
//! grows a case, and the census test below reads every case in every
//! language — a variant with no sentence would draw its raw key.

use crate::agents::Doing;
use crate::key;
use ronoctua_i18n::Arg;
use ronoctua_llm::statusline::Wrapping;
use ronoctua_llm::{Kind, Why};

/// The sentence of a refusal of the accounts' crate.
#[must_use]
pub const fn refusal_key(why: Why) -> &'static str {
    match why {
        Why::NotLet => key::AGENTS_REFUSED_NOT_LET,
        Why::Expired => key::AGENTS_REFUSED_EXPIRED,
        Why::NoCredentials => key::AGENTS_REFUSED_NO_CREDENTIALS,
        Why::Unreadable => key::AGENTS_REFUSED_UNREADABLE,
        Why::Offline => key::AGENTS_REFUSED_OFFLINE,
        Why::Unknown => key::AGENTS_REFUSED_UNKNOWN,
        Why::Invalid => key::AGENTS_REFUSED_INVALID,
        Why::NotInstalled => key::AGENTS_REFUSED_NOT_INSTALLED,
        Why::Arriving => key::AGENTS_REFUSED_ARRIVING,
        Why::Failed => key::AGENTS_REFUSED_FAILED,
    }
}

/// The sentence of a refusal, said.
#[must_use]
pub fn refused(why: Why, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(refusal_key(why))
}

/// The same refusal as a CLAUSE, for after a sentence's colon ("The usage
/// endpoint was not read: the login expired; …").
///
/// WRITTEN IN EACH LANGUAGE, and not the sentence with its first letter
/// lowered here: whether a clause after a colon takes a small letter is the
/// language's to say, and a word that starts one may be a name that keeps
/// its capital in any position.
#[must_use]
pub const fn clause_key(why: Why) -> &'static str {
    match why {
        Why::NotLet => key::AGENTS_CLAUSE_NOT_LET,
        Why::Expired => key::AGENTS_CLAUSE_EXPIRED,
        Why::NoCredentials => key::AGENTS_CLAUSE_NO_CREDENTIALS,
        Why::Unreadable => key::AGENTS_CLAUSE_UNREADABLE,
        Why::Offline => key::AGENTS_CLAUSE_OFFLINE,
        Why::Unknown => key::AGENTS_CLAUSE_UNKNOWN,
        Why::Invalid => key::AGENTS_CLAUSE_INVALID,
        Why::NotInstalled => key::AGENTS_CLAUSE_NOT_INSTALLED,
        Why::Arriving => key::AGENTS_CLAUSE_ARRIVING,
        Why::Failed => key::AGENTS_CLAUSE_FAILED,
    }
}

/// A refusal said after a colon ([`clause_key`]).
#[must_use]
pub fn clause(why: Why, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(clause_key(why))
}

/// An agent, by its name.
#[must_use]
pub const fn agent_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Claude => key::AGENTS_KIND_CLAUDE,
        Kind::ClaudeApi => key::AGENTS_KIND_CLAUDE_API,
        Kind::Codex => key::AGENTS_KIND_CODEX,
    }
}

/// The row that adds an account of an agent.
#[must_use]
pub const fn adding_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Claude => key::AGENTS_ADD_CLAUDE,
        Kind::ClaudeApi => key::AGENTS_ADD_CLAUDE_API,
        Kind::Codex => key::AGENTS_ADD_CODEX,
    }
}

/// The status line's state, as the row's value — what `RET` on it does is in
/// the sentence.
#[must_use]
pub const fn wrapping_key(w: Wrapping) -> &'static str {
    match w {
        Wrapping::Ours => key::AGENTS_LINE_OURS,
        Wrapping::Replaced => key::AGENTS_LINE_REPLACED,
        Wrapping::Not => key::AGENTS_LINE_NOT,
    }
}

/// What a running agent is doing, as its row's value says it.
#[must_use]
pub const fn doing_key(doing: Doing) -> &'static str {
    match doing {
        Doing::Working => key::AGENTS_DOING_WORKING,
        Doing::Waiting => key::AGENTS_DOING_WAITING,
        Doing::Done => key::AGENTS_DOING_DONE,
        Doing::Failed => key::AGENTS_DOING_FAILED,
    }
}

/// A per cent, whole, in the language's way of writing one.
#[must_use]
pub fn percent_text(p: f32, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_args(key::AGENTS_PERCENT, &[("percent", Arg::Number(whole(p)))])
}

/// A per cent as a whole number, bounded — what a bar and a sentence say.
#[must_use]
pub fn whole(p: f32) -> i64 {
    // A PER CENT, bounded to ten times a window: the cast cannot truncate.
    #[allow(clippy::cast_possible_truncation)]
    let n = if p.is_finite() {
        p.clamp(0.0, 1000.0).round() as i64
    } else {
        0
    };
    n
}

/// A count of tokens, short: `950`, `12.3k`, `4.1M`, `2.0bn` — the decimal
/// mark the language's, the word after it the language's too.
#[must_use]
pub fn tokens_text(n: u64, texts: &ronoctua_i18n::Texts) -> String {
    const K: u64 = 1_000;
    const M: u64 = K * K;
    const G: u64 = M * K;
    let (unit, key) = match n {
        0..K => {
            return texts.t_args(
                key::AGENTS_TOKENS_FEW,
                &[("n", Arg::Number(i64::try_from(n).unwrap_or(i64::MAX)))],
            );
        }
        K..M => (K, key::AGENTS_TOKENS_THOUSANDS),
        M..G => (M, key::AGENTS_TOKENS_MILLIONS),
        _ => (G, key::AGENTS_TOKENS_BILLIONS),
    };
    // ONE DECIMAL, rounded half up: 12 345 is 12.3k, and 999 950 rounds to
    // 1 000.0k rather than jumping a unit the number has not reached.
    let tenths = (n.saturating_mul(10) + unit / 2) / unit;
    let value = format!("{}{}{}", tenths / 10, texts.decimal_mark(), tenths % 10);
    texts.t_with(key, &[("value", &value)])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a case of a closed set has no sentence in a language — it
    /// would draw its raw key.
    #[test]
    fn every_case_has_its_words_in_every_language() {
        let sets: Vec<&str> = Why::ALL
            .into_iter()
            .map(refusal_key)
            .chain(Why::ALL.into_iter().map(clause_key))
            .chain(Kind::ALL.into_iter().map(agent_key))
            .chain(Kind::ALL.into_iter().map(adding_key))
            .chain(
                [Wrapping::Ours, Wrapping::Replaced, Wrapping::Not]
                    .into_iter()
                    .map(wrapping_key),
            )
            .chain(Doing::ALL.into_iter().map(doing_key))
            .collect();
        for language in ronoctua_i18n::LANGUAGES {
            let texts = ronoctua_i18n::Texts::fresh(language).expect("the language");
            for k in &sets {
                let said = texts.t(k);
                assert!(!said.is_empty() && said != *k, "{language}: {k}");
            }
        }
    }

    /// FAILS IF: a refusal said after a colon starts with a capital in a
    /// language of this system — `não foi lido: O login venceu` — where both
    /// write a clause there with a small letter.
    #[test]
    fn a_refusal_after_a_colon_starts_small() {
        for language in ronoctua_i18n::LANGUAGES {
            let texts = ronoctua_i18n::Texts::fresh(language).expect("the language");
            for why in Why::ALL {
                let said = clause(why, &texts);
                assert!(
                    said.chars().next().is_some_and(char::is_lowercase),
                    "{language}: {why:?} reads `{said}` after a colon"
                );
                let framed = texts.t_with(key::AGENTS_ENDPOINT_REFUSED, &[("why", &said)]);
                assert!(framed.ends_with(&said), "{language}: {framed}");
            }
        }
    }

    /// FAILS IF: a count reads in the wrong unit, with the wrong mark, or a
    /// per cent is not a whole number.
    #[test]
    fn numbers_read_in_the_language() {
        let en = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let pt = ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR");
        assert_eq!(tokens_text(950, &en), "950");
        assert_eq!(tokens_text(12_345, &en), "12.3k");
        assert_eq!(tokens_text(4_150_000, &en), "4.2M");
        assert_eq!(tokens_text(2_000_000_000, &en), "2.0bn");
        assert_eq!(tokens_text(12_345, &pt), "12,3 mil");
        assert_eq!(percent_text(84.6, &en), "85%");
        assert_eq!(whole(f32::NAN), 0);
        assert_eq!(whole(-3.0), 0);
    }
}
