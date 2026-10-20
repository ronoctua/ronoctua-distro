//! The scanner for languages read as code, and for markup.
//!
//! # It takes a `&Grammar` and never a `Language`
//!
//! Which is what makes it testable with no generated table at all: a test
//! writes a `Grammar` as a Rust literal and exercises nesting, raw strings and
//! escapes in `cargo test`, in seconds, with no Nix and no environment
//! variable. What is left for the generator and the checks is whether the
//! declared table is RIGHT — a different question, asked in a different place.
//!
//! # The order of the tests is the design
//!
//! A character literal is tried first, in a grammar that has one: Common
//! Lisp's `#\;` is the semicolon and `#\"` the quote, and read as anything
//! else they would open a comment or a string.
//!
//! Then a line comment, which returns immediately. That is what makes "the
//! comment beats the quotes" true rather than hoped for: without it,
//! `# "this"` would open a string that never closes and paint the rest of the
//! block one colour.
//!
//! Then: a block comment, then a string opener (longest first, so `"""` beats
//! `"`), then an attribute prefix, then a numeral at a word boundary, then a
//! whole word. Never a prefix of a word — by prefix, `iffy` would be a
//! half-coloured `if`. What a word is made of is the grammar's (`word_chars`):
//! in Lisp `when-let` is one word, and does not light `when`.

use alloc::vec::Vec;

use crate::{Grammar, Open, Range, Role, State, in_word, is_word_char, push};

/// One line of a language read as code.
pub(crate) fn code(text: &str, grammar: &Grammar, state: State, out: &mut Vec<Range>) -> State {
    let bytes = text.as_bytes();
    let mut at = 0usize;

    // WHAT THE LINE BEFORE LEFT OPEN, discharged first.
    match state.outer {
        Open::Comment { depth } => match close_comment(text, grammar, depth, 0) {
            Walked::Open(depth) => {
                push(out, 0, text.len(), Role::Comment);
                return with_outer(state, Open::Comment { depth });
            }
            Walked::Closed(end) => {
                push(out, 0, end, Role::Comment);
                at = end;
            }
        },
        Open::Text { delimiter, hashes } => {
            let d = grammar.strings.get(usize::from(delimiter));
            let Some(d) = d else {
                return with_outer(state, Open::Nothing);
            };
            match close_text(text, d, hashes, 0) {
                Carried::Open => {
                    quoted(text, grammar, 0, text.len(), out);
                    return state;
                }
                // A DELIMITER WITH A CEILING NEVER SPANS LINES, so this cannot
                // be reached from a carried state; treating it as a close is
                // what keeps the line drawable if it ever is.
                Carried::NotText | Carried::Closed(_) => {
                    let end = match close_text(text, d, hashes, 0) {
                        Carried::Closed(end) => end,
                        _ => text.len(),
                    };
                    quoted(text, grammar, 0, end, out);
                    at = end;
                }
            }
        }
        Open::Fence { .. } | Open::Embedded { .. } | Open::Nothing => {}
    }

    while at < bytes.len() {
        // 0. A CHARACTER LITERAL, before the comment and the quotes it may
        //    hold: `#\;` is the semicolon and `#\"` the quote.
        if let Some(prefix) = grammar.char_prefix
            && starts(bytes, at, prefix)
        {
            let end = char_literal_end(bytes, at + prefix.len(), grammar);
            push(out, at, end, Role::Constant);
            at = end;
            continue;
        }

        // 1. A COMMENT TO THE END OF THE LINE, and it RETURNS. That return is
        //    what makes "comment beats quotes" a property rather than an order
        //    somebody has to remember.
        if let Some(marker) = grammar.line_comments.iter().find(|m| starts(bytes, at, m)) {
            let _ = marker;
            push(out, at, text.len(), Role::Comment);
            return with_outer(state, Open::Nothing);
        }

        // 2. A COMMENT ACROSS LINES.
        if let Some(block) = grammar.block_comment
            && starts(bytes, at, block.open)
        {
            let from = at;
            match close_comment(text, grammar, 1, at + block.open.len()) {
                Walked::Open(depth) => {
                    push(out, from, text.len(), Role::Comment);
                    return with_outer(state, Open::Comment { depth });
                }
                Walked::Closed(end) => {
                    push(out, from, end, Role::Comment);
                    at = end;
                    continue;
                }
            }
        }

        // 3. QUOTED TEXT, longest opener first — the generator sorts them, so
        //    `"""` is tried before `"` and a docstring is not read as two
        //    empty strings.
        if let Some((n, d, hashes, body)) = grammar
            .strings
            .iter()
            .enumerate()
            .find_map(|(n, d)| opens(bytes, at, d).map(|(h, b)| (n, d, h, b)))
        {
            let from = at;
            match close_text(text, d, hashes, body) {
                Carried::Closed(end) => {
                    quoted(text, grammar, from, end, out);
                    at = end;
                    continue;
                }
                Carried::NotText => {
                    // NOT AN OPENER AFTER ALL. Advance past its first
                    // character and go on scanning: a lifetime is punctuation
                    // and a name, not quoted text.
                    at += char_len(bytes[at]);
                    continue;
                }
                Carried::Open => {
                    quoted(text, grammar, from, text.len(), out);
                    return if d.spans_lines {
                        with_outer(
                            state,
                            Open::Text {
                                delimiter: u8::try_from(n).unwrap_or(0),
                                hashes,
                            },
                        )
                    } else {
                        // IT DID NOT SPAN, so it does not carry. A stray `"`
                        // colours its own line and nothing beyond it.
                        with_outer(state, Open::Nothing)
                    };
                }
            }
        }

        // 4. A MARKER ABOUT THE THING.
        if let Some(a) = grammar.attribute
            && starts(bytes, at, a.prefix)
        {
            let from = at;
            let end = match a.close {
                Some(close) => {
                    find(bytes, at + a.prefix.len(), close).map_or(text.len(), |n| n + close.len())
                }
                None => word_end(bytes, at + a.prefix.len(), grammar),
            };
            if end > from {
                push(out, from, end, Role::Attribute);
                at = end;
                continue;
            }
        }

        let c = bytes[at];

        // 5. A NUMERAL, and only at a word boundary — inside `utf8` the `8` is
        //    part of the word, not a number.
        if !matches!(grammar.numbers, crate::Numbers::None)
            && at_boundary(bytes, at, grammar)
            && let Some(end) = numeral(bytes, at, grammar)
        {
            push(out, at, end, Role::Number);
            at = end;
            continue;
        }

        // 6. A WHOLE WORD, never a prefix of one.
        if in_word(c, grammar) && at_boundary(bytes, at, grammar) {
            let end = word_end(bytes, at, grammar);
            let word = &text[at..end];
            if let Some(role) = classify(word, bytes, end, grammar) {
                push(out, at, end, role);
            }
            at = end;
            continue;
        }

        // 7. ANYTHING ELSE, and it advances by the CHARACTER and never by one
        //    byte: stepping into the middle of a multi-byte character makes
        //    every offset after it land off a boundary, and whoever draws
        //    slices by these offsets.
        at += char_len(c);
    }

    with_outer(state, Open::Nothing)
}

/// One line of markup: tags, attributes and entities.
pub(crate) fn markup(text: &str, grammar: &Grammar, state: State, out: &mut Vec<Range>) -> State {
    let bytes = text.as_bytes();
    let mut at = 0usize;

    // THE REST OF A `<script>` OR A `<style>` the line before opened, drawn
    // in its own language up to the element's closing tag.
    if let Open::Embedded { element, inner } = state.outer {
        let close = EMBEDDED
            .get(usize::from(element))
            .and_then(|(name, _)| closing_tag(bytes, 0, name));
        let stop = close.unwrap_or(text.len());
        let carried = embedded(text, 0, stop, inner, state.inner, out);
        let Some(stop) = close else {
            return State {
                outer: state.outer,
                inner: carried,
            };
        };
        at = stop;
    }
    let state = if matches!(state.outer, Open::Embedded { .. }) {
        State::fresh()
    } else {
        state
    };

    if let Open::Comment { depth } = state.outer {
        match close_comment(text, grammar, depth, 0) {
            Walked::Open(depth) => {
                push(out, 0, text.len(), Role::Comment);
                return with_outer(state, Open::Comment { depth });
            }
            Walked::Closed(end) => {
                push(out, 0, end, Role::Comment);
                at = end;
            }
        }
    }

    while at < bytes.len() {
        if let Some(block) = grammar.block_comment
            && starts(bytes, at, block.open)
        {
            let from = at;
            match close_comment(text, grammar, 1, at + block.open.len()) {
                Walked::Open(depth) => {
                    push(out, from, text.len(), Role::Comment);
                    return with_outer(state, Open::Comment { depth });
                }
                Walked::Closed(end) => {
                    push(out, from, end, Role::Comment);
                    at = end;
                    continue;
                }
            }
        }

        // A DECLARATION — `<!DOCTYPE`, `<?xml … ?>` — is a marker about the
        // document rather than an element of it.
        if starts(bytes, at, "<!") || starts(bytes, at, "<?") {
            let end = find(bytes, at, ">").map_or(text.len(), |n| n + 1);
            push(out, at, end, Role::Attribute);
            at = end;
            continue;
        }

        if c_at(bytes, at) == Some(b'<') {
            let closed = find(bytes, at, ">");
            let end = closed.map_or(text.len(), |n| n + 1);
            tag(text, at, end, grammar, out);
            // AN ELEMENT WHOSE CONTENT IS ANOTHER LANGUAGE hands the text
            // after its opening tag to that language, up to its closing tag —
            // on this line, or on a later one through the state.
            if closed.is_some()
                && let Some((element, inner)) = embeds(bytes, at, end, grammar)
            {
                let close = EMBEDDED
                    .get(usize::from(element))
                    .and_then(|(name, _)| closing_tag(bytes, end, name));
                let stop = close.unwrap_or(text.len());
                let carried = embedded(text, end, stop, inner, Open::Nothing, out);
                match close {
                    Some(stop) => {
                        at = stop;
                        continue;
                    }
                    None => {
                        return State {
                            outer: Open::Embedded { element, inner },
                            inner: carried,
                        };
                    }
                }
            }
            at = end;
            continue;
        }

        // AN ENTITY is the markup family's escape: `&amp;` is not the text it
        // sits in, and without a role it reads as inert letters.
        if c_at(bytes, at) == Some(b'&')
            && let Some(semi) = find(bytes, at, ";")
            && semi - at <= 12
        {
            push(out, at, semi + 1, Role::Escape);
            at = semi + 1;
            continue;
        }

        // EVERYTHING BETWEEN TAGS IS PROSE, and prose is not coloured.
        at += char_len(bytes[at]);
    }

    with_outer(state, Open::Nothing)
}

/// The elements whose content is another language, and the fence tag that
/// names it (`crate::of_tag`) — the one place markup learns that a `<script>`
/// holds a program and a `<style>` a stylesheet.
///
/// BY TAG, AND NOT BY A `Language` VARIANT: the variants are generated, and a
/// table row renamed upstream must leave the content plain rather than fail
/// to compile here. A tag nothing answers to draws the content as prose,
/// which is what it was before this existed.
const EMBEDDED: [(&str, &str); 2] = [("script", "javascript"), ("style", "css")];

/// Whether the tag in `from..to` OPENS one of [`EMBEDDED`] — not its closing
/// tag, and not a self-closed `<script src=… />`, which holds nothing — and
/// which, with the language its content is drawn in.
fn embeds(
    bytes: &[u8],
    from: usize,
    to: usize,
    grammar: &Grammar,
) -> Option<(u8, crate::Language)> {
    let name_at = from + 1;
    if c_at(bytes, name_at) == Some(b'/') || to < 2 || c_at(bytes, to - 2) == Some(b'/') {
        return None;
    }
    let name_end = word_end(bytes, name_at, grammar);
    let name = bytes.get(name_at..name_end)?;
    EMBEDDED
        .iter()
        .enumerate()
        .find(|(_, (element, _))| name.eq_ignore_ascii_case(element.as_bytes()))
        .and_then(|(n, (_, tag))| Some((u8::try_from(n).ok()?, crate::of_tag(tag)?)))
}

/// Where `</name` begins at or after `at`, any case — the end of an
/// embedded element's content. A `</scripts` is not it: the name has to end
/// there.
fn closing_tag(bytes: &[u8], at: usize, name: &str) -> Option<usize> {
    let mut from = at;
    while let Some(n) = find(bytes, from, "</") {
        let name_at = n + 2;
        let end = name_at + name.len();
        if bytes
            .get(name_at..end)
            .is_some_and(|b| b.eq_ignore_ascii_case(name.as_bytes()))
            && !bytes.get(end).copied().is_some_and(is_word_char)
        {
            return Some(n);
        }
        from = name_at;
    }
    None
}

/// Draws `from..to` of a markup line in the embedded language, its ranges
/// placed on the LINE's offsets, and answers what that language leaves open.
fn embedded(
    text: &str,
    from: usize,
    to: usize,
    inner: crate::Language,
    open: Open,
    out: &mut Vec<Range>,
) -> Open {
    let Some(piece) = text.get(from..to) else {
        return Open::Nothing;
    };
    let before = out.len();
    let carried = code(
        piece,
        inner.grammar(),
        State {
            outer: open,
            inner: Open::Nothing,
        },
        out,
    );
    for r in &mut out[before..] {
        r.start += from;
        r.end += from;
    }
    carried.outer
}

/// The pieces of one `<…>`: the element's name, then its attributes.
fn tag(text: &str, from: usize, to: usize, grammar: &Grammar, out: &mut Vec<Range>) {
    let bytes = text.as_bytes();
    let mut at = from + 1;
    if c_at(bytes, at) == Some(b'/') {
        at += 1;
    }
    let name = word_end(bytes, at, grammar);
    push(out, at, name, Role::Tag);
    at = name;

    while at < to {
        let c = bytes[at];
        if c == b'"' || c == b'\'' {
            let end = find(bytes, at + 1, if c == b'"' { "\"" } else { "'" })
                .map_or(to, |n| n + 1)
                .min(to);
            push(out, at, end, Role::String);
            at = end;
            continue;
        }
        if in_word(c, grammar) && at_boundary(bytes, at, grammar) {
            let end = word_end(bytes, at, grammar);
            // AN ATTRIBUTE'S NAME IS THE KEY SIDE OF A PAIR, which is what the
            // role means — the same role a JSON key gets, and deliberately so.
            if c_at(bytes, end) == Some(b'=') {
                push(out, at, end, Role::Property);
            }
            at = end;
            continue;
        }
        at += char_len(c);
    }
}

/// What a word is, if it is anything.
fn classify(word: &str, bytes: &[u8], end: usize, grammar: &Grammar) -> Option<Role> {
    // THE WHOLE WORD, compared with `binary_search` against sorted lists — the
    // generator sorts them and a test asserts every member is findable, which
    // is what stops a lookup silently missing.
    if grammar.keywords.binary_search(&word).is_ok() {
        return Some(Role::Keyword);
    }
    if grammar.types.binary_search(&word).is_ok() {
        return Some(Role::Type);
    }
    if grammar.constants.binary_search(&word).is_ok() {
        return Some(Role::Constant);
    }
    // A LISP KEYWORD, `:test`: a word that starts with the colon. A symbol
    // of a package, `cl:car`, has its colon inside, and is a name.
    if grammar.colon_is_constant && word.len() > 1 && word.starts_with(':') {
        return Some(Role::Constant);
    }
    if grammar.screaming_is_constant && is_screaming(word) {
        return Some(Role::Constant);
    }
    if grammar.capitalised_is_type && is_capitalised(word) {
        return Some(Role::Type);
    }
    if grammar.call && next_visible(bytes, end) == Some(b'(') {
        return Some(Role::Function);
    }
    match grammar.property {
        crate::Property::Colon if next_visible(bytes, end) == Some(b':') => Some(Role::Property),
        crate::Property::Equals if next_visible(bytes, end) == Some(b'=') => Some(Role::Property),
        _ => None,
    }
}

/// `HTTP_TIMEOUT` yes, `HTTP` yes, `Http` no, `X` no.
///
/// The single letter is excluded on purpose: a type parameter `T` and a
/// generic `E` are not constants, and there are more of them in a line of
/// Rust than there are constants in a file.
fn is_screaming(word: &str) -> bool {
    word.len() > 1
        && word.chars().any(|c| c.is_ascii_uppercase())
        && word
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// `String` yes, `STRING` no (that is a constant), `string` no.
fn is_capitalised(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase()) && chars.any(|c| c.is_ascii_lowercase())
}

/// Emits quoted text, split around whatever inside it is NOT text.
///
/// An escape and an interpolation both stop being literal, and drawing them in
/// the string's own colour is what makes `"${HOME}/x"` read as inert.
fn quoted(text: &str, grammar: &Grammar, from: usize, to: usize, out: &mut Vec<Range>) {
    let bytes = text.as_bytes();
    let mut cut = from;
    let mut at = from;
    let escape = grammar
        .strings
        .iter()
        .map(|d| d.escape)
        .find(|e| !e.is_empty())
        .unwrap_or("");

    while at < to {
        if !escape.is_empty() && starts(bytes, at, escape) && at + escape.len() < to {
            let end = (at + escape.len() + char_len(bytes[at + escape.len()])).min(to);
            push(out, cut, at, Role::String);
            push(out, at, end, Role::Escape);
            cut = end;
            at = end;
            continue;
        }
        if let Some(marker) = grammar.interpolation.iter().find(|m| starts(bytes, at, m)) {
            let end = if marker.ends_with('{') {
                find(bytes, at, "}").map_or(to, |n| n + 1).min(to)
            } else if marker.ends_with('(') {
                find(bytes, at, ")").map_or(to, |n| n + 1).min(to)
            } else {
                word_end(bytes, at + marker.len(), grammar).min(to)
            };
            if end > at {
                push(out, cut, at, Role::String);
                push(out, at, end, Role::Escape);
                cut = end;
                at = end;
                continue;
            }
        }
        at += char_len(bytes[at]);
    }
    push(out, cut, to, Role::String);
}

/// What a walk through a block comment ended in.
///
/// A SEPARATE ENUM FROM [`Carried`], and not tidiness: a comment cannot come
/// back "this was never a comment", and sharing one type would put an
/// unreachable arm at every comment site — an arm nobody can test and everyone
/// has to read.
enum Walked {
    /// Still open at the end of the line, at this depth.
    Open(u8),
    /// It closed, here.
    Closed(usize),
}

/// What a walk through quoted text ended in.
enum Carried {
    /// It is still open at the end of the line.
    ///
    /// It carries nothing: WHICH delimiter opened it is already known to the
    /// caller, which is the one that matched it.
    Open,
    /// It closed, here.
    Closed(usize),
    /// IT WAS NEVER TEXT. Only a delimiter with a `max` can say this, and it
    /// is what tells Rust's `&\'a str` from a character literal: past the
    /// ceiling the opener is not an opener, so it is skipped as ordinary
    /// punctuation instead of being closed early and painted.
    ///
    /// Closing it early was the first attempt, and the test caught it: the
    /// lifetime came out as a string reaching to the next quote, which
    /// swallowed the type after it.
    NotText,
}

/// Walks a block comment from `at`, honouring nesting.
fn close_comment(text: &str, grammar: &Grammar, depth: u8, at: usize) -> Walked {
    let Some(block) = grammar.block_comment else {
        return Walked::Closed(at);
    };
    let bytes = text.as_bytes();
    let mut depth = depth;
    let mut at = at;
    while at < bytes.len() {
        if starts(bytes, at, block.close) {
            depth = depth.saturating_sub(1);
            at += block.close.len();
            if depth == 0 {
                return Walked::Closed(at);
            }
            continue;
        }
        // NESTING IS A COUNT AND NOT A FLAG, and Haskell is what proves it:
        // `{- {- -} -}` is one comment still open after the first `-}`.
        if block.nests && starts(bytes, at, block.open) {
            depth = depth.saturating_add(1);
            at += block.open.len();
            continue;
        }
        at += char_len(bytes[at]);
    }
    Walked::Open(depth)
}

/// Walks quoted text from `at`, honouring the escape and the raw form.
fn close_text(text: &str, d: &crate::Delimiter, hashes: u8, at: usize) -> Carried {
    let bytes = text.as_bytes();
    let mut at = at;
    let mut taken = 0usize;
    while at < bytes.len() {
        // THE CEILING, and it is what stops a lifetime opening a string.
        // Without it `&'a str` reads as a `'` that never closes, and the rest
        // of the file comes out one colour.
        if let Some(max) = d.max
            && taken > usize::from(max)
        {
            return Carried::NotText;
        }
        if !d.escape.is_empty() && starts(bytes, at, d.escape) {
            at += d.escape.len();
            if at < bytes.len() {
                at += char_len(bytes[at]);
            }
            taken += 1;
            continue;
        }
        if starts(bytes, at, d.close) && trailing_hashes(bytes, at + d.close.len()) >= hashes {
            return Carried::Closed(at + d.close.len() + usize::from(hashes));
        }
        at += char_len(bytes[at]);
        taken += 1;
    }
    Carried::Open
}

/// Does a delimiter open at `at`, and where does its body start?
///
/// # The hashes sit BETWEEN the marker and the quote
///
/// Rust's raw string is `r#"…"#`, not `r"#…`. Counting them after the whole
/// opener was the first attempt, and the test caught it: `r#"a"b"#` closed at
/// the inner quote and the rest of the line came out as a second string.
///
/// So for a delimiter with `hashes`, `open` is the marker BEFORE them (`r`),
/// the close quote has to follow them, and the body starts after that.
fn opens(bytes: &[u8], at: usize, d: &crate::Delimiter) -> Option<(u8, usize)> {
    if !starts(bytes, at, d.open) {
        return None;
    }
    if !d.hashes {
        return Some((0, at + d.open.len()));
    }
    // A bare `r` is a name, so the marker only opens when a quote follows the
    // hashes — with none of them, `r"…"` is still a raw string.
    let hashes = count_hashes(bytes, at + d.open.len());
    let quote = at + d.open.len() + usize::from(hashes);
    if !starts(bytes, quote, d.close) {
        return None;
    }
    Some((hashes, quote + d.close.len()))
}

/// How many `#` follow, for Rust's raw form.
fn count_hashes(bytes: &[u8], at: usize) -> u8 {
    let mut n = 0u8;
    let mut at = at;
    while c_at(bytes, at) == Some(b'#') && n < u8::MAX {
        n += 1;
        at += 1;
    }
    n
}

/// The same, read after a closing quote.
fn trailing_hashes(bytes: &[u8], at: usize) -> u8 {
    count_hashes(bytes, at)
}

/// Does `needle` start at `at`?
fn starts(bytes: &[u8], at: usize, needle: &str) -> bool {
    !needle.is_empty()
        && bytes.len() >= at + needle.len()
        && &bytes[at..at + needle.len()] == needle.as_bytes()
}

/// Where `needle` next appears at or after `at`.
fn find(bytes: &[u8], at: usize, needle: &str) -> Option<usize> {
    (at..bytes.len().saturating_sub(needle.len().saturating_sub(1)))
        .find(|&n| starts(bytes, n, needle))
}

/// The byte at `at`, if there is one.
const fn c_at(bytes: &[u8], at: usize) -> Option<u8> {
    if at < bytes.len() {
        Some(bytes[at])
    } else {
        None
    }
}

/// The next byte that is not a space or a tab.
fn next_visible(bytes: &[u8], at: usize) -> Option<u8> {
    let mut at = at;
    while let Some(c) = c_at(bytes, at) {
        if c != b' ' && c != b'\t' {
            return Some(c);
        }
        at += 1;
    }
    None
}

/// Is `at` the first byte of a word, by this grammar's word characters?
fn at_boundary(bytes: &[u8], at: usize, grammar: &Grammar) -> bool {
    at == 0 || !in_word(bytes[at - 1], grammar)
}

/// Where the word starting at `at` ends.
fn word_end(bytes: &[u8], at: usize, grammar: &Grammar) -> usize {
    let mut end = at;
    while end < bytes.len() && in_word(bytes[end], grammar) {
        end += 1;
    }
    end
}

/// Where the numeral starting at `at` ends, if one starts there.
///
/// A digit starts one. So does a SIGN BEFORE A DIGIT where the grammar reads
/// the sign as part of a word — Lisp's `-1` is one token, and its `-` is no
/// operator; in a grammar where `-` is punctuation the sign stays out and the
/// digits are the numeral, as they always were. Lisp's ratio `1/2` is one
/// numeral too, for the same reason.
///
/// AND A NUMERAL THAT RUNS INTO A CHARACTER ONLY THIS GRAMMAR CALLS PART OF A
/// WORD IS NOT ONE: Lisp's `1+` is a function and `1-2-3` a symbol. The
/// default word characters never count here, so a grammar that declares none
/// reads every numeral as it did before rows could declare them.
fn numeral(bytes: &[u8], at: usize, grammar: &Grammar) -> Option<usize> {
    let c = bytes[at];
    let digits = if c.is_ascii_digit() {
        at
    } else if (c == b'-' || c == b'+')
        && in_word(c, grammar)
        && c_at(bytes, at + 1).is_some_and(|n| n.is_ascii_digit())
    {
        at + 1
    } else {
        return None;
    };
    let c_like = matches!(grammar.numbers, crate::Numbers::C);
    let mut end = number_end(bytes, digits, c_like);
    if c_at(bytes, end) == Some(b'/')
        && in_word(b'/', grammar)
        && c_at(bytes, end + 1).is_some_and(|n| n.is_ascii_digit())
    {
        end = number_end(bytes, end + 1, c_like);
    }
    if c_at(bytes, end).is_some_and(|n| in_word(n, grammar) && !is_word_char(n)) {
        return None;
    }
    Some(end)
}

/// Where a character literal ends, its prefix already read: one character —
/// whatever it is, a quote, a semicolon, an accented letter — and, when that
/// one is part of a word, the word characters after it (`#\Space`,
/// `#\Newline`). At the end of the line the literal is the prefix alone: the
/// character it names is the line break.
fn char_literal_end(bytes: &[u8], body: usize, grammar: &Grammar) -> usize {
    let Some(first) = c_at(bytes, body) else {
        return bytes.len().min(body);
    };
    let mut end = (body + char_len(first)).min(bytes.len());
    if in_word(first, grammar) {
        while end < bytes.len() && in_word(bytes[end], grammar) {
            end += 1;
        }
    }
    end
}

/// Where the numeral starting at `at` ends.
fn number_end(bytes: &[u8], at: usize, c_like: bool) -> usize {
    let mut end = at;
    while end < bytes.len() {
        let c = bytes[end];
        let ok = c.is_ascii_alphanumeric()
            || c == b'_'
            || (c == b'.' && c_at(bytes, end + 1).is_some_and(|n| n.is_ascii_digit()))
            || (c_like
                && (c == b'+' || c == b'-')
                && matches!(c_at(bytes, end.wrapping_sub(1)), Some(b'e' | b'E')));
        if !ok {
            break;
        }
        end += 1;
    }
    end
}

/// How many bytes the character starting with this byte takes.
///
/// Advancing by ONE byte through a multi-byte character puts every offset
/// after it off a `char` boundary, and whoever draws slices by these offsets —
/// so the failure is a panic on screen, not a wrong colour.
const fn char_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        // A continuation byte here means the caller was already off a
        // boundary. One byte gets back in step rather than skipping further.
        _ => 1,
    }
}

/// The same state with a different `outer`.
const fn with_outer(state: State, outer: Open) -> State {
    State {
        outer,
        inner: state.inner,
    }
}
